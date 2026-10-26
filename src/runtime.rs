//! Tasks and the event loop: what `async func`, `await`, and std.async run on.
//! All Rust; std.async builds its functions on these in Marslang.
//!
//! Each task runs on a stack of its own, a stackful coroutine (corosensei),
//! so a task can pause anywhere: deep inside expressions and calls, and inside
//! native code. Every task runs on the interpreter's one thread, one at a time,
//! so values (`Rc`) never cross threads.
//!
//! - Code outside any task (`m` and top-level statements) is the root. When
//!   the root waits, it runs the event loop until what it waits for is done.
//! - A task that waits leaves a `Waker` with whatever will end the wait (a
//!   task finishing, a timer, a worker thread, a `Signal`) and suspends,
//!   which switches back to the root's event loop.
//! - The event loop runs ready tasks in the order they became ready. When none
//!   is ready it sleeps until the next timer is due or a worker thread
//!   finishes, so waiting costs no processor time.
//!
//! Native code waits with `block_on_worker`, `sleep`, `wait_any`, and `Signal`.
//! Inside a task they suspend only that task; at the root, `block_on_worker`
//! and `sleep` simply block, while waiting for tasks runs them. Each returns
//! `Err(CancelledError)` when the waiting task is cancelled, which a native
//! passes on with `?`. Other tasks run while one waits, so a native must not
//! hold a `RefCell` borrow across a wait: of its own thread-local state, which
//! another task's call would then find borrowed, or of a value's contents,
//! which postpones the cycle collector until it is released. Holding values
//! themselves is fine.
//!
//! A task stack is as large as the interpreter thread's, so a task recurses as
//! deeply as `m` can, but it is only reserved: memory is used only for the
//! pages a task touches. Stacks are allocated when a task first runs, and
//! kept for reuse when it finishes.

use std::cell::{Cell, RefCell};
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::panic::AssertUnwindSafe;
use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use corosensei::stack::DefaultStack;
use corosensei::{Coroutine, CoroutineResult, Yielder};

use crate::interp::{Interp, TaskLocal};
use crate::value::*;

/// Bytes of address space each task stack reserves: the interpreter thread's
/// size, so `interp::MAX_CALL_DEPTH` is reached before the stack runs out.
const TASK_STACK: usize = crate::INTERPRETER_STACK;
/// Stacks of finished tasks kept for new ones.
const POOLED_STACKS: usize = 16;

type TaskCoroutine = Coroutine<(), (), Outcome, DefaultStack>;

/// A call of an async function: queued, running, waiting, or finished.
pub struct Task {
    id: u64,
    /// The async function's name, for messages and `<task NAME>`.
    pub(crate) name: String,
    /// The body, until the task starts.
    start: RefCell<Option<Box<dyn FnOnce() -> Outcome>>>,
    /// Its stack and state, from when it starts until it finishes. Taken out
    /// while it runs.
    coroutine: RefCell<Option<TaskCoroutine>>,
    /// How it finished; `None` until then.
    outcome: RefCell<Option<Outcome>>,
    /// When it finished, counting finished tasks from 1; 0 until then.
    finished: Cell<u64>,
    /// `cancel()` was called, and the task has not been told yet.
    cancelling: Cell<bool>,
    /// The wait it is in, matching the token of the `Waker` that ends it; 0
    /// when it is not waiting.
    token: Cell<u64>,
    /// Its coroutine's yielder, set when it starts.
    yielder: Cell<*const Yielder<(), ()>>,
    /// Who waits for it to finish.
    waiters: RefCell<Vec<Waker>>,
    /// The task it is awaiting, while it does.
    awaiting: RefCell<Option<Rc<Task>>>,
    /// Whether its result or error has been read, by await or result().
    pub(crate) observed: Cell<bool>,
}

/// How a task ended.
#[derive(Clone)]
pub enum Outcome {
    Value(Value),
    /// The error, and the value a handler sees for it; `None` for an error
    /// raised outside the interpreter, whose value is made when needed.
    Error(RuntimeError, Option<Value>),
}

impl Task {
    pub fn is_done(&self) -> bool { self.outcome.borrow().is_some() }

    pub fn outcome(&self) -> Option<Outcome> { self.outcome.borrow().clone() }

    /// The values a finished task holds, for the cycle collector; `None` while
    /// its outcome is borrowed.
    pub(crate) fn held(&self) -> Option<Vec<Value>> {
        let outcome = self.outcome.try_borrow().ok()?;
        Some(match &*outcome {
            Some(Outcome::Value(value)) | Some(Outcome::Error(_, Some(value))) => vec![value.clone()],
            _ => Vec::new(),
        })
    }

    /// Drop the outcome of a task the cycle collector found unreachable.
    pub(crate) fn release(&self) -> Option<Outcome> { self.outcome.borrow_mut().take() }
}

/// What ends one wait of one task (or of the root, task 0).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Waker { task: u64, token: u64 }

/// A timer: wake `waker` at `deadline`. Ordered so the heap gives the earliest.
struct Timer { deadline: Instant, sequence: u64, waker: Waker }

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool { (self.deadline, self.sequence) == (other.deadline, other.sequence) }
}
impl Eq for Timer {}
impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}
impl Ord for Timer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (other.deadline, other.sequence).cmp(&(self.deadline, self.sequence))
    }
}

struct Runtime {
    /// Tasks ready to run, in the order they became ready.
    ready: VecDeque<Rc<Task>>,
    /// Every unfinished task: queued, running, ready, or waiting.
    live: HashMap<u64, Rc<Task>>,
    timers: BinaryHeap<Timer>,
    /// Worker jobs that have not reported back, and who waits for each.
    jobs: HashMap<u64, Waker>,
    /// Workers send their job's number here when they finish.
    done_sender: mpsc::Sender<u64>,
    done_receiver: mpsc::Receiver<u64>,
    /// The wait the root is in; 0 once it is woken, or when it is not waiting.
    root_token: u64,
    /// Counters for task ids, wait tokens, timers, and jobs.
    next: u64,
    finished: u64,
    stacks: Vec<DefaultStack>,
    /// Tasks that ended with an error other than a cancellation.
    failed: Vec<Rc<Task>>,
    /// A task asked the program to stop (std.cli.exit): how.
    exit: Option<RuntimeError>,
}

impl Runtime {
    fn new() -> Runtime {
        let (done_sender, done_receiver) = mpsc::channel();
        Runtime { ready: VecDeque::new(), live: HashMap::new(), timers: BinaryHeap::new(), jobs: HashMap::new(),
            done_sender, done_receiver, root_token: 0, next: 0, finished: 0, stacks: Vec::new(), failed: Vec::new(), exit: None }
    }

    fn number(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    /// Whether `waker` would still end a wait.
    fn waiting(&self, waker: Waker) -> bool {
        if waker.task == 0 { return self.root_token == waker.token; }
        self.live.get(&waker.task).is_some_and(|task| task.token.get() == waker.token)
    }
}

thread_local! {
    /// Each program runs on its own thread, so each has its own tasks.
    static RUNTIME: RefCell<Runtime> = RefCell::new(Runtime::new());
    /// The task running now; `None` at the root.
    static CURRENT: RefCell<Option<Rc<Task>>> = const { RefCell::new(None) };
    /// The interpreter running the program, which keeps each task's call
    /// depth and caller; null outside a run.
    static HOST: Cell<*const Interp<'static>> = const { Cell::new(std::ptr::null()) };
    /// Suspended tasks are being unwound, as a run ends.
    static UNWINDING: Cell<bool> = const { Cell::new(false) };
}

/// Begin a run on this thread with `host` as its interpreter.
pub(crate) fn enter(host: &Interp) {
    HOST.with(|cell| cell.set(host as *const Interp as *const Interp<'static>));
}

/// Swap the interpreter's per-task state for `local`, returning what it had.
fn swap_local(local: TaskLocal) -> TaskLocal {
    let host = HOST.with(Cell::get);
    if host.is_null() { return local; }
    // SAFETY: the host is set for the length of a run, and every task is
    // discarded (`shutdown`) before that run's interpreter is dropped.
    unsafe { (*host).swap_local(local) }
}

fn current() -> Option<Rc<Task>> {
    CURRENT.with(|current| current.borrow().clone())
}

/// Whether code is running inside a task, rather than at the root.
pub fn in_task() -> bool {
    CURRENT.with(|current| current.borrow().is_some())
}

/// Whether suspended tasks are being unwound because the run is ending. A
/// native call's panic guard lets this unwinding through.
pub(crate) fn unwinding() -> bool { UNWINDING.with(Cell::get) }

/// Start a task that runs `body` when the event loop reaches it: after the
/// tasks already ready, once the code running now waits or ends.
pub(crate) fn spawn(name: String, body: Box<dyn FnOnce() -> Outcome>) -> Rc<Task> {
    RUNTIME.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        let task = Rc::new(Task {
            id: runtime.number(), name, start: RefCell::new(Some(body)), coroutine: RefCell::new(None),
            outcome: RefCell::new(None), finished: Cell::new(0), cancelling: Cell::new(false), token: Cell::new(0),
            yielder: Cell::new(std::ptr::null()), waiters: RefCell::new(Vec::new()), awaiting: RefCell::new(None),
            observed: Cell::new(false),
        });
        runtime.live.insert(task.id, task.clone());
        runtime.ready.push_back(task.clone());
        task
    })
}

/// End a wait: make its task ready, or tell the root. Whether it ended one;
/// a waker whose wait has already ended does nothing.
fn wake(waker: Waker) -> bool {
    RUNTIME.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        if !runtime.waiting(waker) { return false; }
        if waker.task == 0 {
            runtime.root_token = 0;
        } else {
            let task = runtime.live[&waker.task].clone();
            task.token.set(0);
            runtime.ready.push_back(task);
        }
        true
    })
}

/// The task's cancellation, if one is waiting to be raised in it.
fn take_cancel(task: &Task) -> RResult<()> {
    if task.cancelling.replace(false) {
        return err(ErrorKind::CancelledError, format!("task {} was cancelled", task.name));
    }
    Ok(())
}

/// Raise the running task's pending cancellation, if any. Every wait does
/// this first; at the root there is nothing to cancel.
pub fn check_cancel() -> RResult<()> {
    match current() {
        Some(task) => take_cancel(&task),
        None => Ok(()),
    }
}

/// A new wait for the running task, or the root: register the waker it
/// returns with what will end the wait, then `park`.
fn prepare() -> Waker {
    RUNTIME.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        let token = runtime.number();
        match current() {
            Some(task) => {
                task.token.set(token);
                Waker { task: task.id, token }
            }
            None => {
                runtime.root_token = token;
                Waker { task: 0, token }
            }
        }
    })
}

/// Wait until `waker` is woken. A task suspends, and raises CancelledError if
/// it was cancelled meanwhile; the root runs other tasks until then, and
/// fails if none of them can end its wait.
fn park(waker: Waker) -> RResult<()> {
    match current() {
        Some(task) => {
            suspend(&task);
            task.token.set(0);
            take_cancel(&task)
        }
        None => {
            let result = run_until(&|runtime| runtime.root_token != waker.token);
            RUNTIME.with(|runtime| {
                let mut runtime = runtime.borrow_mut();
                if runtime.root_token == waker.token { runtime.root_token = 0; }
            });
            match result {
                Ok(()) => Ok(()),
                Err(Stop::Exit(error)) => Err(error),
                Err(Stop::Deadlock) => err(ErrorKind::Error,
                    "deadlock: this wait can never end, since every unfinished task is waiting too, \
                     and no timer or worker thread is pending"),
            }
        }
    }
}

/// Switch from the running task back to the event loop until it is resumed.
fn suspend(task: &Task) {
    let yielder = task.yielder.get();
    let mine = swap_local(TaskLocal::default());
    // SAFETY: the yielder belongs to this task's coroutine, whose stack we are
    // running on; it lives as long as the coroutine.
    unsafe { (*yielder).suspend(()) };
    swap_local(mine);
}

/// Why the event loop stopped before what it was run for was done.
enum Stop {
    /// Nothing is ready, and no timer or worker thread can change that.
    Deadlock,
    /// A task asked the program to stop.
    Exit(RuntimeError),
}

enum Next { Done, Run(Rc<Task>), Wait, Exit(RuntimeError) }

/// The event loop, at the root: run ready tasks, and sleep until a timer or
/// a worker thread makes one ready, until `done` holds.
fn run_until(done: &dyn Fn(&Runtime) -> bool) -> Result<(), Stop> {
    loop {
        let next = RUNTIME.with(|runtime| {
            let mut runtime = runtime.borrow_mut();
            if let Some(exit) = &runtime.exit { return Next::Exit(exit.clone()); }
            if done(&runtime) { return Next::Done; }
            match runtime.ready.pop_front() {
                Some(task) => Next::Run(task),
                None => Next::Wait,
            }
        });
        match next {
            Next::Done => return Ok(()),
            Next::Exit(error) => return Err(Stop::Exit(error)),
            Next::Run(task) => {
                step(task);
                // Tasks that never wait for long must not keep timers and
                // workers from being noticed.
                events(false);
            }
            Next::Wait => if !events(true) { return Err(Stop::Deadlock) },
        }
    }
}

/// Wake whoever waits for timers that are due and worker threads that have
/// finished. With `block`, first sleep until there is one. False when there
/// is nothing to wait for.
fn events(block: bool) -> bool {
    let woken = RUNTIME.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        // Timers of waits that already ended, by cancellation, are dropped
        // when they come up, or all at once when they pile up, as when many
        // timeouts end early.
        if runtime.timers.len() > 64 && runtime.timers.len() > 2 * runtime.live.len() {
            let timers = std::mem::take(&mut runtime.timers);
            runtime.timers = timers.into_iter().filter(|timer| runtime.waiting(timer.waker)).collect();
        }
        while let Some(timer) = runtime.timers.peek() {
            if runtime.waiting(timer.waker) { break; }
            runtime.timers.pop();
        }
        let deadline = runtime.timers.peek().map(|timer| timer.deadline);
        if deadline.is_none() && runtime.jobs.is_empty() { return None; }
        let mut jobs = Vec::new();
        if block {
            match deadline {
                Some(deadline) => {
                    let now = Instant::now();
                    if deadline > now {
                        if let Ok(job) = runtime.done_receiver.recv_timeout(deadline - now) { jobs.push(job); }
                    }
                }
                // The runtime keeps a sender, so this returns when a worker finishes.
                None => if let Ok(job) = runtime.done_receiver.recv() { jobs.push(job); },
            }
        }
        if !runtime.jobs.is_empty() {
            while let Ok(job) = runtime.done_receiver.try_recv() { jobs.push(job); }
        }
        let mut woken: Vec<Waker> = jobs.iter().filter_map(|job| runtime.jobs.remove(job)).collect();
        if deadline.is_some() {
            let now = Instant::now();
            while runtime.timers.peek().is_some_and(|timer| timer.deadline <= now) {
                woken.push(runtime.timers.pop().unwrap().waker);
            }
        }
        Some(woken)
    });
    let Some(woken) = woken else { return false };
    for waker in woken { wake(waker); }
    true
}

/// Restores the root's state when a task stops running, even by panicking.
struct Running { root: Option<TaskLocal> }

impl Drop for Running {
    fn drop(&mut self) {
        CURRENT.with(|current| current.borrow_mut().take());
        if let Some(root) = self.root.take() { swap_local(root); }
    }
}

/// Run `task` until it waits or finishes. Only the root's event loop calls this.
fn step(task: Rc<Task>) {
    if task.is_done() { return; }
    let mut coroutine = match task.coroutine.take() {
        Some(coroutine) => coroutine,
        None => {
            let Some(body) = task.start.take() else { return };
            let stack = RUNTIME.with(|runtime| runtime.borrow_mut().stacks.pop());
            let stack = match stack.map(Ok).unwrap_or_else(|| DefaultStack::new(TASK_STACK)) {
                Ok(stack) => stack,
                Err(error) => {
                    let error = RuntimeError::new(ErrorKind::Error, format!("task {} could not start: no memory for its stack ({error})", task.name));
                    finish(&task, Outcome::Error(error, None));
                    return;
                }
            };
            TaskCoroutine::with_stack(stack, move |yielder: &Yielder<(), ()>, ()| {
                if let Some(task) = current() { task.yielder.set(yielder); }
                body()
            })
        }
    };
    CURRENT.with(|current| *current.borrow_mut() = Some(task.clone()));
    let running = Running { root: Some(swap_local(TaskLocal::default())) };
    let result = coroutine.resume(());
    drop(running);
    match result {
        CoroutineResult::Yield(()) => { task.coroutine.replace(Some(coroutine)); }
        CoroutineResult::Return(outcome) => {
            let stack = coroutine.into_stack();
            RUNTIME.with(|runtime| {
                let mut runtime = runtime.borrow_mut();
                if runtime.stacks.len() < POOLED_STACKS { runtime.stacks.push(stack); }
            });
            finish(&task, outcome);
        }
    }
}

/// Record how `task` ended and wake whoever waits for it.
fn finish(task: &Rc<Task>, outcome: Outcome) {
    let waiters = RUNTIME.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        if let Outcome::Error(error, _) = &outcome {
            if error.exit.is_some() { runtime.exit.get_or_insert_with(|| error.clone()); }
            else if error.kind != ErrorKind::CancelledError { runtime.failed.push(task.clone()); }
        }
        *task.outcome.borrow_mut() = Some(outcome);
        runtime.finished += 1;
        task.finished.set(runtime.finished);
        runtime.live.remove(&task.id);
        task.waiters.take()
    });
    for waiter in waiters { wake(waiter); }
}

/// Ask `task` to stop: it raises CancelledError at its next wait, and at once
/// if it is waiting now. A task that has not started ends without running.
/// A task awaiting another cancels that one too, unless something else is
/// waiting for it, as `await async.sleep(60)` waits for a task of its own.
/// False when it has already finished.
pub fn cancel(task: &Rc<Task>) -> bool {
    if task.is_done() { return false; }
    if task.start.borrow_mut().take().is_some() {
        let error = RuntimeError::new(ErrorKind::CancelledError, format!("task {} was cancelled", task.name));
        finish(task, Outcome::Error(error, None));
        return true;
    }
    task.cancelling.set(true);
    let token = task.token.get();
    if token != 0 { wake(Waker { task: task.id, token }); }
    let awaited = task.awaiting.borrow_mut().take();
    if let Some(awaited) = awaited {
        awaited.waiters.borrow_mut().retain(|waker| waker.task != task.id);
        let waited_for = RUNTIME.with(|runtime| {
            let runtime = runtime.borrow();
            awaited.waiters.borrow().iter().any(|waker| runtime.waiting(*waker))
        });
        if !waited_for { cancel(&awaited); }
    }
    true
}

/// Wait until `task` finishes; its outcome is then `task.outcome()`.
pub fn wait_for(task: &Rc<Task>) -> RResult<()> {
    check_cancel()?;
    if task.is_done() { return Ok(()); }
    let running = current();
    if running.as_ref().is_some_and(|running| Rc::ptr_eq(running, task)) {
        return err(ErrorKind::Error, format!("task {} cannot await itself: it would wait forever", task.name));
    }
    while !task.is_done() {
        let waker = prepare();
        task.waiters.borrow_mut().push(waker);
        if let Some(running) = &running { *running.awaiting.borrow_mut() = Some(task.clone()); }
        let result = park(waker);
        if let Some(running) = &running { running.awaiting.borrow_mut().take(); }
        if result.is_err() { task.waiters.borrow_mut().retain(|w| *w != waker); }
        result?;
    }
    Ok(())
}

/// Wait until one of `tasks` finishes: the position of the one that finished
/// first. Returns at once when one already has.
pub fn wait_any(tasks: &[Rc<Task>]) -> RResult<usize> {
    check_cancel()?;
    if tasks.is_empty() { return err(ErrorKind::RangeError, "waiting for one of no tasks would wait forever"); }
    if let Some(running) = current() {
        if tasks.iter().any(|task| Rc::ptr_eq(task, &running)) {
            return err(ErrorKind::Error, format!("task {} cannot wait for itself: it would wait forever", running.name));
        }
    }
    loop {
        let first = tasks.iter().enumerate()
            .filter(|(_, task)| task.finished.get() > 0)
            .min_by_key(|(_, task)| task.finished.get());
        if let Some((index, _)) = first { return Ok(index); }
        let waker = prepare();
        for task in tasks { task.waiters.borrow_mut().push(waker); }
        let result = park(waker);
        for task in tasks { task.waiters.borrow_mut().retain(|w| *w != waker); }
        result?;
    }
}

/// Wait for `length`. Inside a task only the task waits, on a timer; at the
/// root this blocks.
pub fn sleep(length: Duration) -> RResult<()> {
    check_cancel()?;
    if !in_task() {
        std::thread::sleep(length);
        return Ok(());
    }
    // Far enough to never come, for a length beyond what Instant can add.
    let deadline = Instant::now().checked_add(length).unwrap_or_else(|| Instant::now() + Duration::from_secs(86_400 * 365 * 100));
    let waker = prepare();
    RUNTIME.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        let sequence = runtime.number();
        runtime.timers.push(Timer { deadline, sequence, waker });
    });
    park(waker)
}

/// Run `work` on a worker thread and return what it returns. Inside a task
/// only the task waits: the others run meanwhile. At the root it simply runs
/// here, blocking. If the task is cancelled while waiting it raises
/// CancelledError at once; the worker runs on, and its result is dropped. A
/// panic in `work` becomes an Error.
pub fn block_on_worker<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> RResult<T> {
    let failed = |panic: Box<dyn std::any::Any + Send>| {
        let detail = panic.downcast_ref::<&str>().map(|s| s.to_string())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".into());
        RuntimeError::new(ErrorKind::Error, format!("internal error in a worker thread: {detail}"))
    };
    check_cancel()?;
    if !in_task() { return std::panic::catch_unwind(AssertUnwindSafe(work)).map_err(failed); }
    let slot: Arc<Mutex<Option<std::thread::Result<T>>>> = Arc::new(Mutex::new(None));
    let (job, sender) = RUNTIME.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        (runtime.number(), runtime.done_sender.clone())
    });
    let filled = slot.clone();
    std::thread::Builder::new().name("marslang worker".into()).spawn(move || {
        let result = std::panic::catch_unwind(AssertUnwindSafe(work));
        *filled.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(result);
        // The run may have ended, and its receiver with it.
        let _ = sender.send(job);
    }).map_err(|error| RuntimeError::new(ErrorKind::Error, format!("could not start a worker thread: {error}")))?;
    let waker = prepare();
    RUNTIME.with(|runtime| runtime.borrow_mut().jobs.insert(job, waker));
    park(waker)?;
    let result = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take();
    result.expect("a worker reports back after it fills its slot").map_err(failed)
}

/// Something tasks wait for until other code notifies it: the building block
/// for channels, locks, and events in native packages.
#[derive(Default)]
pub struct Signal { waiters: RefCell<Vec<Waker>> }

impl Signal {
    pub fn new() -> Signal { Signal::default() }

    /// Wait until `notify_one` or `notify_all`. Inside a task this suspends
    /// only the task, and raises CancelledError if it is cancelled meanwhile.
    /// At the root it runs the other tasks until then, and fails when none of
    /// them can notify it.
    pub fn wait(&self) -> RResult<()> {
        check_cancel()?;
        let waker = prepare();
        self.waiters.borrow_mut().push(waker);
        let result = park(waker);
        if result.is_err() { self.waiters.borrow_mut().retain(|w| *w != waker); }
        result
    }

    /// Wake every waiter; how many there were.
    pub fn notify_all(&self) -> usize {
        let waiters = self.waiters.take();
        waiters.into_iter().filter(|waker| wake(*waker)).count()
    }

    /// Wake the waiter that has waited longest; false when none waits.
    pub fn notify_one(&self) -> bool {
        loop {
            let next = {
                let mut waiters = self.waiters.borrow_mut();
                if waiters.is_empty() { return false; }
                waiters.remove(0)
            };
            if wake(next) { return true; }
        }
    }
}

/// Run every task to its end, as a run does once the program's own code is
/// done. Tasks that can never finish are discarded, with an error naming them.
pub(crate) fn run_all() -> RResult<()> {
    match run_until(&|runtime| runtime.live.is_empty()) {
        Ok(()) => Ok(()),
        Err(Stop::Exit(error)) => Err(error),
        Err(Stop::Deadlock) => {
            let mut names: Vec<String> = RUNTIME.with(|runtime| runtime.borrow().live.values().map(|task| task.name.clone()).collect());
            names.sort();
            discard();
            let count = names.len();
            err(ErrorKind::Error, format!("deadlock: {count} task{} can never finish, since each is waiting for another task: {}",
                if count == 1 { "" } else { "s" }, names.join(", ")))
        }
    }
}

/// Cancel every unfinished task, as a run does when its code fails.
pub(crate) fn cancel_all() {
    let tasks: Vec<Rc<Task>> = RUNTIME.with(|runtime| runtime.borrow().live.values().cloned().collect());
    for task in &tasks { cancel(task); }
}

/// The error of a task that failed with nothing awaiting it, if any: the
/// first, with a note saying so. Forgets the failed tasks.
pub(crate) fn unobserved_failure() -> Option<RuntimeError> {
    let failed = RUNTIME.with(|runtime| std::mem::take(&mut runtime.borrow_mut().failed));
    let unseen: Vec<&Rc<Task>> = failed.iter().filter(|task| !task.observed.get()).collect();
    let first = unseen.first()?;
    let Some(Outcome::Error(mut error, _)) = first.outcome() else { return None };
    let others = match unseen.len() - 1 {
        0 => String::new(),
        1 => " (1 more task failed with nothing awaiting it)".to_string(),
        n => format!(" ({n} more tasks failed with nothing awaiting them)"),
    };
    error.trace.get_or_insert_with(Default::default).footer = Some(format!("in task {}, which nothing awaited{others}", first.name));
    Some(error)
}

/// Drop every unfinished task, and forget the failed ones and any exit
/// request, so the event loop starts afresh. Suspended tasks are unwound,
/// which frees what their stacks hold; their `then` blocks do not run.
pub(crate) fn discard() {
    let tasks: Vec<Rc<Task>> = RUNTIME.with(|runtime| {
        let mut runtime = runtime.borrow_mut();
        runtime.ready.clear();
        runtime.timers.clear();
        // Workers still running report back to no one.
        runtime.jobs.clear();
        runtime.root_token = 0;
        runtime.failed.clear();
        runtime.exit = None;
        runtime.live.drain().map(|(_, task)| task).collect()
    });
    UNWINDING.with(|unwinding| unwinding.set(true));
    for task in &tasks {
        task.start.take();
        drop(task.coroutine.take());
        // Tasks awaiting each other would otherwise keep each other alive.
        task.awaiting.take();
    }
    UNWINDING.with(|unwinding| unwinding.set(false));
}

/// End a run: discard its tasks, and forget its interpreter.
pub(crate) fn shutdown() {
    discard();
    HOST.with(|host| host.set(std::ptr::null()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_task(name: &str, body: impl FnOnce() -> RResult<Value> + 'static) -> Rc<Task> {
        spawn(name.into(), Box::new(move || match body() {
            Ok(value) => Outcome::Value(value),
            Err(error) => Outcome::Error(error, None),
        }))
    }

    fn result(task: &Task) -> Value {
        match task.outcome() { Some(Outcome::Value(value)) => value, _ => panic!("task {} did not succeed", task.name) }
    }

    /// The runtime is per thread, and each test has its own.
    fn on_own_thread(test: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(test).unwrap().join().unwrap();
    }

    #[test]
    fn ten_tasks_waiting_on_worker_threads_wait_together() {
        on_own_thread(|| {
            let started = Instant::now();
            let tasks: Vec<Rc<Task>> = (0..10).map(|i| value_task("work", move || {
                let n = block_on_worker(move || { std::thread::sleep(Duration::from_millis(200)); i * 2 })?;
                Ok(Value::Int(n))
            })).collect();
            run_all().unwrap();
            let elapsed = started.elapsed();
            let values: Vec<i64> = tasks.iter().map(|task| match result(task) { Value::Int(n) => n, _ => -1 }).collect();
            assert_eq!(values, (0..10).map(|i| i * 2).collect::<Vec<i64>>());
            assert!(elapsed < Duration::from_millis(1200), "took {elapsed:?}");
            shutdown();
        });
    }

    #[test]
    fn timers_wake_tasks_in_deadline_order() {
        on_own_thread(|| {
            let order = Rc::new(RefCell::new(Vec::new()));
            for (name, ms) in [("slow", 60), ("fast", 10), ("middle", 30)] {
                let order = order.clone();
                value_task(name, move || { sleep(Duration::from_millis(ms))?; order.borrow_mut().push(name); Ok(Value::Null) });
            }
            run_all().unwrap();
            assert_eq!(*order.borrow(), ["fast", "middle", "slow"]);
            shutdown();
        });
    }

    #[test]
    fn signals_wake_waiting_tasks_and_cancellation_interrupts_waits() {
        on_own_thread(|| {
            let signal = Rc::new(Signal::new());
            let s = signal.clone();
            let waiter = value_task("waiter", move || { s.wait()?; Ok(Value::Int(1)) });
            let s = signal.clone();
            let sleeper = value_task("sleeper", move || { sleep(Duration::from_secs(60))?; Ok(Value::Null) });
            let target = sleeper.clone();
            value_task("notifier", move || {
                s.notify_all();
                cancel(&target);
                Ok(Value::Null)
            });
            let started = Instant::now();
            run_all().unwrap();
            assert!(started.elapsed() < Duration::from_secs(5));
            assert!(matches!(result(&waiter), Value::Int(1)));
            assert!(matches!(sleeper.outcome(), Some(Outcome::Error(e, _)) if e.kind == ErrorKind::CancelledError));
            shutdown();
        });
    }

    #[test]
    fn collection_waits_while_a_suspended_task_holds_a_container_borrowed() {
        on_own_thread(|| {
            crate::gc::collect();
            let cycle = Value::array(Vec::new());
            if let Value::Array(a) = &cycle { a.items.borrow_mut().push(cycle.clone()); }
            drop(cycle);
            let held = Value::array(Vec::new());
            let collected = Rc::new(RefCell::new(Vec::new()));
            value_task("holder", move || {
                let Value::Array(array) = &held else { unreachable!() };
                let guard = array.items.borrow_mut();
                // Suspended with the borrow held, as a native might be.
                sleep(Duration::from_millis(30))?;
                drop(guard);
                Ok(Value::Null)
            });
            let during = collected.clone();
            value_task("collector", move || {
                sleep(Duration::from_millis(5))?;
                during.borrow_mut().push(crate::gc::collect());
                Ok(Value::Null)
            });
            run_all().unwrap();
            // Postponed while the borrow was held; the cycle is freed after.
            collected.borrow_mut().push(crate::gc::collect());
            assert_eq!(*collected.borrow(), [0, 1]);
            shutdown();
        });
    }

    #[test]
    fn a_wait_nothing_can_end_is_a_deadlock() {
        on_own_thread(|| {
            let signal = Rc::new(Signal::new());
            let s = signal.clone();
            value_task("stuck", move || { s.wait()?; Ok(Value::Null) });
            let error = run_all().unwrap_err();
            assert!(error.message.contains("deadlock: 1 task can never finish"), "{error}");
            // At the root, a wait that cannot end fails rather than hanging.
            let error = signal.wait().unwrap_err();
            assert!(error.message.contains("deadlock"), "{error}");
            shutdown();
        });
    }
}
