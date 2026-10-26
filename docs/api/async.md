# Async: tasks, async func, and await

Tasks let one program make progress on several things while each waits: two
timers, a download and a countdown, a hundred requests. `async func` declares
a function whose calls run as tasks, and `await` waits for a task to finish.

```mars
takepkg std.async;

async func brew(string drink, int minutes){
    // A tenth of a second for each minute, while the other drink brews too.
    await async.sleep(minutes / 10);
    ret drink + " is ready";
}

func m{
    tea = brew("tea", 3);
    coffee = brew("coffee", 1);
    out(await coffee);                  // coffee is ready
    out(await tea);                     // tea is ready
}
```

Both drinks brew at once, so the program takes 0.3 seconds, not 0.4.
[std.async](std-async.md) has the waiting functions: `sleep`, `all`, `race`,
`timeout`, `settled`, `spawn`, and `yield_now`.

| Form | Meaning |
| --- | --- |
| `async func name(params){ ... }` | A function whose calls start tasks |
| `async func name(params) => expression;` | The same, with a body of one expression |
| `async func(type x) => expression` | An anonymous async function |
| `async func name(...)` in a family | An async method; `init` cannot be async |
| `await t` | Wait for the task `t` to finish: its result, or its error raised here |
| `t.done()` | Whether the task has finished |
| `t.result()` | Its result, or its error raised here; an `Error` while it has not finished |
| `t.cancel()` | Ask it to stop: `CancelledError` is raised in it at its next wait |
| `task t` | A parameter, variable, or `match` pattern of type task |

## Tasks

Calling an async function checks its arguments, as any call does, then
starts a task and gives it back at once. The body does not run yet: the task
waits its turn until the code running now waits or ends, after the tasks that
were started before it.

```mars
async func greet(string name) => "hello " + name;

func m{
    t = greet("ada");
    out(t, t.done());                   // <task greet> false
    out(await t);                       // hello ada
    out(t.done(), t.result());          // true hello ada
}
```

Tasks take turns on one thread. A task runs until it waits, at an `await` of
an unfinished task or in a wait such as `async.sleep`, and then the next task
that is ready runs; tasks become ready in the order their waits end. Only one
runs at a time, so two tasks never change a value at the same moment, and no
locks are needed. A task that computes for a long time without waiting holds
the others up; `await async.yield_now();` in its loop lets them run.

A task is a value: it prints as `<task NAME>`, compares by identity, and is
of type `task`, as in `func watch(task t)` or the pattern `task t =>`. Awaiting a
finished task again gives the same result. Functions declared with `async`
are listed by `marslang symbols` with `"async":true`.

## await

`await t` waits until the task `t` finishes. Its value is the task's result;
when the task raised an error, the same error is raised by the `await`, to be
handled where the await is.

- **It binds tightly.** `await` applies to the call, member, or index right
  after it, before any operator: `await a() + await b()` adds two results, and
  `await t ** 2` squares one.
- **Only tasks.** `await` of anything else, `await 5` or `await f()` where `f`
  is not async, raises `TypeError: await needs a task, got an int`.
- **Anywhere.** `await` works in every function, async or not, in `m`, and at
  the top level. Inside a task, only that task waits. Outside any task, in `m`
  or the top level, the other tasks run until the awaited one has finished.

```mars
takepkg std.async;

async func double(int x){
    await async.sleep(0.01);
    ret x * 2;
}

func total(array numbers){
    sum = 0;
    for (n, numbers){ sum += await double(n); }   // not async, yet it awaits
    ret sum;
}

func m{
    out(total(arr(1, 2, 3)));                         // 12
    out(await double(5) + await double(10));          // 30
}
```

### How it differs from JavaScript and Python

- **No colouring.** In JavaScript and Python only an async function may
  await, so a function that needs to wait must become async, and so must its
  callers. In Marslang any function may await, and a function does not change
  for its callers when it starts to.
- **Queued, not eager.** A JavaScript async function runs at once until its
  first `await`; a Marslang one does not start until the code that called it
  waits or ends.
- **Started by the call.** A Python coroutine does nothing until it is
  awaited or made a task; a Marslang task runs whether or not anything awaits it.
- **Tasks only.** JavaScript lets `await` take any value; Marslang raises
  `TypeError`, since awaiting a value that is not a task is usually a missing
  `async`.
- **At the end.** When `m` returns, tasks still running finish, as in Node.js.
  Python's `asyncio.run` cancels them instead.

## When m ends

Tasks still running when `m` returns run to their end before the program
does. Then, if a task raised an error that nothing awaited, and nothing read
with `result()`, the program fails with that error, so it is never lost:

```text
error: Error: lost
    at fails (jobs.mars:2)
    in task fails, which nothing awaited
```

A task that was cancelled does not count. When `m` itself fails, the tasks
still running are cancelled first, so their `then` blocks run, and the program
then reports `m`'s error. `std.cli.exit(code)` stops every task where it is,
inside a task as anywhere else. All of this holds for each test that
`marslang test` runs, and for top-level code without an `m`; an `async func m`
is awaited.

## Errors and traces

An error in a task is reported with the calls the task made, then the call
that awaited it:

```mars
family ValueError(Error){}

async func parse(string text){
    if (text == ""){ err(ValueError, "nothing to parse"); }
    ret text.len();
}

async func load() => await parse("");

func m{
    run {
        await load();
    } handle(ValueError e){
        out(e.message);                 // nothing to parse
    }
}
```

Without the `run` block, with `m` only `await load();` on line 11 of
`load.mars`, the error ends the program with this trace. `awaiting parse`
marks where the frames of the task `parse` end and the code that awaited it
begins:

```text
error: ValueError: nothing to parse
    at parse (load.mars:4)
    at load (load.mars:8), awaiting parse
    at m (load.mars:11), awaiting load
```

## Cancelling

`t.cancel()` asks a task to stop. The task raises `CancelledError` at its next
wait, or at once if it is waiting now, so its `handle` and `then` blocks run as
for any error. Awaiting a cancelled task raises `CancelledError` too.
`cancel()` returns true, or false when the task had already finished. A task
cancelled before it started never runs.

```mars
takepkg std.async;

async func download(){
    run {
        await async.sleep(60);
    } then {
        out("download closed");
    }
}

func m{
    t = download();
    await async.sleep(0.01);
    out(t.cancel());                    // true
    run {
        await t;                        // download closed
    } handle(CancelledError e){
        out(e.message);                 // task download was cancelled
    }
    out(t.done(), t.cancel());          // true false
}
```

A task cancelled while it awaits another task cancels that one too, unless
something else is waiting for it: `await async.sleep(60)` waits for a task of
its own, which stops with the task that awaits it, rather than running on for
a minute.

`CancelledError` is a built-in error family, inheriting from `Error` like
`TypeError`, so `handle(Error e)` catches it too. A task that catches its
cancellation may clean up, even await, and carry on; usually it raises it
again with `err(e)` once it is done. `async.all`, `race`, `timeout`, and
`settled` cancel the tasks they wait for when they are themselves cancelled.

## Waits that can never end

A task that awaits itself raises `Error: task NAME cannot await itself`.
When every unfinished task is waiting for another and no timer or worker
thread is pending, nothing can ever finish: the `await` in `m` that waits for
one of them raises `Error: deadlock: this wait can never end`, and tasks still
stuck when the program ends fail it with `Error: deadlock: 2 tasks can never
finish, since each is waiting for another task: first, second`.

## Limits

- Tasks take turns on one thread, so they never run in parallel; work that
  only computes is no faster in tasks.
- Calls that wait for the outside world, such as `std.http`, `time.sleep`,
  and `in()`, still block every task until they return. In a task, use
  `async.sleep` to wait.
- Each task that has started has a stack of its own (see below). The pages it
  touches stay in memory until it finishes: from about 10 KB on Windows to
  25 KB on Linux for a task waiting in a short call, so a hundred thousand
  waiting tasks need one to three gigabytes. An async function that calls
  itself starts a task, with its own stack, for each level. On Linux, the
  kernel's limit on memory mappings (`vm.max_map_count`, often 65,530) allows
  about 30,000 started tasks at once; a task beyond that cannot start, and
  awaiting it raises `Error`.

## How it works

Each task runs on a stack of its own, a stackful coroutine from the
[corosensei](https://docs.rs/corosensei) crate, which switches stacks in a
few nanoseconds. That is why `await` can pause a task anywhere, deep in
expressions and calls and in ordinary functions, without the language needing
async colouring. Every task stack reserves 512 MB, the size of the
interpreter's own, so a task can recurse as deeply as `m`, to the 10,000-call
limit; only the pages it touches use memory. A stack is set up when a task
first runs, and reused after it finishes.

Every task runs on the interpreter's one thread, so values are never shared
between threads. The event loop runs whenever the code outside tasks waits,
and once `m` returns: it runs ready tasks in turn, and when none is ready
sleeps until the next timer is due or a worker thread finishes, using no
processor time while it waits. Native code waits for slow work by running it
on a worker thread, which suspends only the task that called it. The cycle
collector counts what a waiting task's stack holds as in use, so collecting
while tasks wait is safe.

The runtime is Rust ([src/runtime.rs](../../src/runtime.rs)); `async func` and
`await` are in [src/parser.rs](../../src/parser.rs),
[src/expression.rs](../../src/expression.rs), and
[src/interp.rs](../../src/interp.rs); and std.async is Marslang
([std/async.mars](../../std/async.mars)) over a few native waits
([std/rs/async.rs](../../std/rs/async.rs)).
