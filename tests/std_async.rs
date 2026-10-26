//! async func, await, tasks, and std.async: tasks that wait together.

use std::time::{Duration, Instant};

fn executes(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("Marslang compilation failed");
    let (stdout, result) = marslang::run_captured(compiled, "");
    if let Err(error) = result { panic!("runtime error: {}\noutput so far:\n{stdout}", error.report()); }
    assert_eq!(stdout, expected);
}

fn runtime_error(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("compile failed");
    let (_, result) = marslang::run_captured(compiled, "");
    let error = result.expect_err(&format!("expected runtime error: {source}")).to_string();
    assert!(error.contains(expected), "expected {expected}, got {error}");
}

/// The error a program fails with, with its trace, and what it printed first.
fn failure(source: &str) -> (String, String) {
    let compiled = marslang::compile(source).expect("compile failed");
    let (stdout, result) = marslang::run_captured(compiled, "");
    (result.expect_err(&format!("expected runtime error: {source}")).report(), stdout)
}

fn compile_error(source: &str, expected: &str) {
    let error = marslang::compile(source).expect_err(&format!("expected a compile error: {source}"));
    assert!(error.contains(expected), "expected {expected}, got {error}");
}

fn timed(source: &str, expected: &str) -> Duration {
    let started = Instant::now();
    executes(source, expected);
    started.elapsed()
}

// ----- the language: async func, await, and tasks -----

#[test]
fn calling_an_async_function_starts_a_task_that_runs_later() {
    executes(r#"
async func add(int a, int b){
    out("adding", a, b);
    ret a + b;
}
func m{
    t = add(1, 2);
    out("started", t, t.done());
    out(await t, t.done(), t.result());
    out(await t);
}"#, "started <task add> false\nadding 1 2\n3 true 3\n3\n");
}

#[test]
fn async_expression_functions_lambdas_and_methods() {
    executes(r#"
async func twice(int x) => x * 2;
family Counter{
    func init(){ me.n = 0; }
    async func bump(int by){ me.n = me.n + by; ret me.n; }
}
func m{
    c = Counter();
    out(await c.bump(2), await c.bump(3), c.n);
    triple = async func(int x) => x * 3;
    out(await triple(4) + await twice(5));
    out(f"{await twice(21)}");
    bump = c.bump;
    out(await bump(10));
}"#, "2 5 5\n22\n42\n15\n");
}

#[test]
fn await_binds_tighter_than_binary_operators() {
    executes(r#"
async func value(int x) => x;
func m{
    out(await value(2) + await value(3) * await value(4));
    out(await value(2) ** 3, -await value(5), not await value(0));
    out(await value(7) == 7);
}"#, "14\n8 -5 true\ntrue\n");
}

#[test]
fn await_works_in_m_and_in_functions_that_are_not_async() {
    executes(r#"
takepkg std.async;
async func slow(int x){
    await async.sleep(0.01);
    ret x;
}
func helper(int x){
    // Not async, yet it may await: inside a task it pauses that task.
    ret await slow(x) + 1;
}
async func outer() => helper(10) * 2;
func m{
    out(helper(1));
    out(await outer());
}"#, "2\n22\n");
}

#[test]
fn tasks_take_turns_in_the_order_they_become_ready() {
    executes(r#"
takepkg std.async;
async func worker(string name, float delay){
    out(name, "starts");
    await async.sleep(delay);
    out(name, "ends");
    ret name;
}
func m{
    a = worker("a", 0.2);
    b = worker("b", 0.1);
    c = worker("c", 0.15);
    out("m waits");
    out(await a, await b, await c);
}"#, "m waits\na starts\nb starts\nc starts\nb ends\nc ends\na ends\na b c\n");
}

#[test]
fn yield_now_lets_ready_tasks_run() {
    executes(r#"
takepkg std.async;
async func count(string name){
    for (i = 0, i < 3, i++){
        out(name, i);
        await async.yield_now();
    }
}
func m{
    a = count("a");
    b = count("b");
    await async.all(arr(a, b));
}"#, "a 0\nb 0\na 1\nb 1\na 2\nb 2\n");
}

#[test]
fn each_task_keeps_its_own_method_access_and_last_error() {
    // b resumes right after a has paused inside a method of Vault: a's access
    // to Vault's private methods must not carry over to b, nor b's lasterr to a.
    executes(r#"
takepkg std.async;
takepkg std.Decorator;
family Vault{
    func init(){ me.secret = 42; }
    @Decorator.private
    func _peek() => me.secret;
    async func open(){
        me.leak = me._peek;
        run { err(Error, "a's own"); } handle(Error e){}
        await async.yield_now();
        out("a sees", lasterr().message);
        ret me.leak();
    }
}
async func thief(Vault v){
    await async.yield_now();
    out("b sees", lasterr());
    run { ret v.leak(); } handle(TypeError e){ ret e.message; }
}
func m{
    v = Vault();
    b = thief(v);
    a = v.open();
    out(await b);
    out(await a);
}"#, "b sees null\n_peek is private to Vault\na sees a's own\n42\n");
}

#[test]
fn tasks_still_pending_when_m_returns_run_to_their_end() {
    executes(r#"
takepkg std.async;
async func later(string what){
    await async.sleep(0.02);
    out(what);
}
func m{
    later("second");
    t = later("third");
    out("m is done");
}"#, "m is done\nsecond\nthird\n");
}

#[test]
fn an_async_m_and_top_level_await() {
    executes(r#"
takepkg std.async;
async func value(int x) => x;
first = await value(1);
async func m{
    await async.sleep(0.01);
    out(first, await value(2));
}"#, "1 2\n");
}

#[test]
fn tasks_are_values_with_a_type_and_a_kind() {
    executes(r#"
async func one() => 1;
func describe(task t) => f"{t} done={t.done()}";
func m{
    t = one();
    out(describe(t));
    out(match (t){ task x => "a task"; _ => "something else"; });
    out(t == t, t == one());
    await t;
    out(describe(t), string(t));
}"#, "<task one> done=false\na task\ntrue false\n<task one> done=true <task one>\n");
    runtime_error("func f(task t) => 1;\nfunc m{ f(5); }", "expected task, got int");
}

#[test]
fn awaiting_something_that_is_not_a_task_is_a_type_error() {
    runtime_error("func m{ x = await 5; }", "await needs a task, got an int: only tasks can be awaited");
    runtime_error("func load() => 1;\nfunc m{ await load(); }", "await needs a task, got an int");
    runtime_error("func m{ await arr(); }", "await needs a task, got an array");
}

#[test]
fn argument_errors_raise_at_the_call_not_in_the_task() {
    let (report, output) = failure("async func f(int x) => x;\nfunc m{\n out(\"before\");\n f(\"no\");\n out(\"after\");\n}");
    assert_eq!(output, "before\n");
    assert!(report.starts_with("TypeError: expected int"), "{report}");
    assert!(report.contains("at m (line 4)"), "{report}");
}

#[test]
fn result_needs_a_finished_task() {
    runtime_error("async func f() => 1;\nfunc m{ f().result(); }", "task f has not finished: await it, or check done() first");
    runtime_error("async func f() => 1;\nfunc m{ f().wait(); }", "task has no method 'wait'");
}

// ----- errors and traces -----

#[test]
fn errors_propagate_through_await_with_the_tasks_frames() {
    let source = "family ValueError(Error){}
async func inner(int x){
    err(ValueError, f\"bad {x}\");
}
async func outer(int x){
    ret await inner(x);
}
func m{
    await outer(3);
}";
    let (report, _) = failure(source);
    assert_eq!(report, "ValueError: bad 3\n    at inner (line 3)\n    at outer (line 6), awaiting inner\n    at m (line 9), awaiting outer");
    executes(&source.replace("    await outer(3);", "    run { await outer(4); } handle(ValueError e){ out(\"caught\", e.message); }"),
        "caught bad 4\n");
}

#[test]
fn a_task_error_is_raised_again_by_every_await_and_by_result() {
    executes(r#"
async func broken() => err(RangeError, "out of range");
func m{
    t = broken();
    run { await t; } handle(RangeError e){ out("await:", e.message); }
    run { await t; } handle(RangeError e){ out("again:", e.message); }
    run { t.result(); } handle(RangeError e){ out("result:", e.message); }
}"#, "await: out of range\nagain: out of range\nresult: out of range\n");
    let (report, _) = failure("async func broken() => err(RangeError, \"no\");\nfunc m{\n t = broken();\n await t;\n t.result();\n}");
    assert!(report.contains("at m (line 4), awaiting broken"), "{report}");
}

#[test]
fn a_failed_task_that_nothing_awaits_fails_the_program() {
    let source = "async func fails(){\n    err(Error, \"lost\");\n}\nfunc m{\n    fails();\n    out(\"m ends\");\n}";
    let (report, output) = failure(source);
    assert_eq!(output, "m ends\n");
    assert_eq!(report, "Error: lost\n    at fails (line 2)\n    in task fails, which nothing awaited");
    let (report, _) = failure("async func fails() => err(Error, \"lost\");\nfunc m{ fails(); fails(); }");
    assert!(report.ends_with("in task fails, which nothing awaited (1 more task failed with nothing awaiting it)"), "{report}");
    // Awaiting it, or reading its result, deals with it; a cancelled task's error never counts.
    executes("async func fails() => err(Error, \"seen\");
func m{
    t = fails();
    run { await t; } handle(Error e){ out(e.message); }
    c = fails();
    c.cancel();
}", "seen\n");
}

#[test]
fn when_m_fails_its_tasks_are_cancelled_and_clean_up() {
    let source = r#"
takepkg std.async;
async func background(){
    run {
        await async.sleep(10);
        out("never");
    } then {
        out("cleaned up");
    }
}
func m{
    background();
    await async.sleep(0.01);
    err(Error, "m failed");
}"#;
    let started = Instant::now();
    let (report, output) = failure(source);
    assert!(report.starts_with("Error: m failed"), "{report}");
    assert_eq!(output, "cleaned up\n");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn exit_in_a_task_stops_the_program() {
    let compiled = marslang::compile(r#"
takepkg std.async;
takepkg std.cli;
async func quit(){
    out("quitting");
    cli.exit(3);
}
async func other(){
    await async.sleep(10);
    out("never");
}
func m{
    other();
    await quit();
    out("never either");
}"#).unwrap();
    let started = Instant::now();
    let (output, result) = marslang::run_captured(compiled, "");
    assert_eq!(output, "quitting\n");
    assert_eq!(result.unwrap_err().exit, Some(3));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn exit_stops_a_task_waiting_inside_native_code() {
    // The waiting task is unwound through the native call it waits in.
    let compiled = marslang::compile(r#"
takepkg std.async;
takepkg std.cli;
async func busy(){
    run { await async.work(3); } then { out("never: exit stops tasks where they are"); }
}
func m{
    busy();
    await async.sleep(0.05);
    cli.exit(4);
}"#).unwrap();
    let started = Instant::now();
    let (output, result) = marslang::run_captured(compiled, "");
    assert_eq!(output, "");
    assert_eq!(result.unwrap_err().exit, Some(4));
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn waits_that_can_never_end_are_errors() {
    runtime_error(r#"
async func a(task other) => await other;
func m{
    holder = arr();
    t = (async func() => await holder[0])();
    holder.add(t);
    await t;
}"#, "cannot await itself");
    runtime_error(r#"
takepkg std.async;
func m{
    gate = arr();
    first = async.spawn(func() => await gate[0]);
    second = async.spawn(func() => await first);
    gate.add(second);
    await first;
}"#, "deadlock: this wait can never end");
    runtime_error(r#"
takepkg std.async;
func m{
    gate = arr();
    first = async.spawn(func() => await gate[0]);
    gate.add(async.spawn(func() => await first));
}"#, "deadlock: 2 tasks can never finish, since each is waiting for another task: async.spawn, async.spawn");
}

// ----- cancellation -----

#[test]
fn cancelling_raises_cancelled_error_at_the_next_await_and_then_blocks_run() {
    executes(r#"
takepkg std.async;
async func job(){
    run {
        out("job waits");
        await async.sleep(10);
        out("never");
    } handle(CancelledError e){
        out("job:", e.message);
        await async.sleep(0.01);
        out("cleanup can await");
        err(e);
    } then {
        out("then runs");
    }
}
func m{
    t = job();
    await async.sleep(0.01);
    out(t.cancel(), t.done());
    run { await t; } handle(CancelledError e){ out("m:", e.message); }
    out(t.done(), t.cancel());
}"#, "job waits\ntrue false\njob: task job was cancelled\ncleanup can await\nthen runs\nm: task job was cancelled\ntrue false\n");
}

#[test]
fn cancelling_a_task_cancels_the_task_it_awaits_unless_another_waits_for_it() {
    let elapsed = timed(r#"
takepkg std.async;
async func inner(string name){
    run { await async.sleep(60); } handle(CancelledError e){ out(name, "cancelled"); err(e); }
}
async func outer(task t) => await t;
async func later(){
    await async.sleep(0.02);
    ret "shared value";
}
func m{
    a = outer(inner("alone"));
    await async.sleep(0.01);
    a.cancel();
    run { await a; } handle(CancelledError e){ out("outer cancelled"); }
    await async.sleep(0.01);
    s = later();
    b = outer(s);
    c = outer(s);
    await async.sleep(0.005);
    b.cancel();
    out(await c, s.result());
}"#, "outer cancelled\nalone cancelled\nshared value shared value\n");
    // Neither sleep(60) is left running for the program to wait for.
    assert!(elapsed < Duration::from_secs(10), "{elapsed:?}");
}

#[test]
fn a_task_cancelled_before_it_starts_never_runs() {
    executes(r#"
async func job(){ out("never"); }
func m{
    t = job();
    out(t.cancel(), t.done());
    run { await t; } handle(CancelledError e){ out(e.message); }
    run { t.result(); } handle(CancelledError e){ out("result raises it too"); }
}"#, "true true\ntask job was cancelled\nresult raises it too\n");
}

#[test]
fn a_task_that_catches_its_cancellation_may_go_on() {
    executes(r#"
takepkg std.async;
async func stubborn(){
    run { await async.sleep(10); } handle(CancelledError e){ out("ignored"); }
    ret "finished anyway";
}
func m{
    t = stubborn();
    await async.yield_now();
    t.cancel();
    out(await t);
}"#, "ignored\nfinished anyway\n");
}

// ----- std.async -----

#[test]
fn ten_tasks_sleeping_200_ms_take_200_ms_in_all() {
    let elapsed = timed(r#"
takepkg std.async;
async func nap(int i){
    await async.sleep(0.2);
    ret i;
}
func m{
    tasks = arr();
    for (i = 0, i < 10, i++){ tasks.add(nap(i)); }
    out(await async.all(tasks));
}"#, "[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]\n");
    assert!(elapsed >= Duration::from_millis(200), "{elapsed:?}");
    assert!(elapsed < Duration::from_millis(1000), "ten 200 ms sleeps took {elapsed:?}");
}

#[test]
fn ten_tasks_waiting_200_ms_on_worker_threads_take_200_ms_in_all() {
    let elapsed = timed(r#"
takepkg std.async;
func m{
    tasks = arr();
    repeat 10 { tasks.add(async.work(0.2)); }
    out(await async.all(tasks));
}"#, "[null, null, null, null, null, null, null, null, null, null]\n");
    assert!(elapsed >= Duration::from_millis(200), "{elapsed:?}");
    assert!(elapsed < Duration::from_millis(1000), "ten 200 ms waits on workers took {elapsed:?}");
}

#[test]
fn sleep_takes_numbers_and_durations_and_checks_them_when_called() {
    executes(r#"
takepkg std.async;
takepkg std.time;
func m{
    await async.sleep(0);
    await async.sleep(longint(0));
    await async.sleep(time.milliseconds(5));
    out("slept");
}"#, "slept\n");
    runtime_error("takepkg std.async;\nfunc m{ t = async.sleep(-1); }", "async.sleep needs a nonnegative number of seconds, got -1");
    runtime_error("takepkg std.async;\nfunc m{ t = async.sleep(\"1\"); }", "value does not match [int,longint,float,duration]");
}

#[test]
fn all_gives_results_in_order_and_raises_the_first_error() {
    executes(r#"
takepkg std.async;
async func after(float seconds, any value){
    await async.sleep(seconds);
    ret value;
}
async func fails_after(float seconds, string why){
    await async.sleep(seconds);
    err(Error, why);
}
async func survivor(){
    run { await async.sleep(1); } handle(CancelledError e){ out("survivor cancelled"); err(e); }
}
func m{
    out(await async.all(arr(after(0.03, "slow"), after(0.01, "fast"), after(0, 3))));
    out(await async.all(arr()));
    run {
        await async.all(arr(fails_after(0.05, "second"), fails_after(0.01, "first"), survivor()));
    } handle(Error e){ out("raised:", e.message); }
}"#, "[\"slow\", \"fast\", 3]\n[]\nsurvivor cancelled\nraised: first\n");
    runtime_error("takepkg std.async;\nfunc m{ async.all(arr(1)); }", "async.all needs an array of tasks, but it holds int 1");
}

#[test]
fn race_gives_the_first_to_finish_and_cancels_the_rest() {
    executes(r#"
takepkg std.async;
async func after(float seconds, string name){
    run { await async.sleep(seconds); } handle(CancelledError e){ out(name, "cancelled"); err(e); }
    ret name;
}
async func fails_after(float seconds){
    await async.sleep(seconds);
    err(RangeError, "lost the race badly");
}
func m{
    out(await async.race(arr(after(0.2, "slow"), after(0.01, "quick"))));
    run { await async.race(arr(after(0.2, "slower"), fails_after(0.01))); } handle(RangeError e){ out(e.message); }
}"#, "slow cancelled\nquick\nslower cancelled\nlost the race badly\n");
    runtime_error("takepkg std.async;\nfunc m{ async.race(arr()); }", "async.race needs at least one task");
}

#[test]
fn timeout_gives_the_result_in_time_or_raises_and_cancels() {
    executes(r#"
takepkg std.async;
async func after(float seconds, string name){
    run { await async.sleep(seconds); } handle(CancelledError e){ out(name, "cancelled"); err(e); }
    ret name;
}
func m{
    out(await async.timeout(after(0.01, "fast"), 1));
    slow = after(5, "slow");
    run { await async.timeout(slow, 0.05); } handle(async.TimeoutError e){ out(e.message); }
    out(slow.done());
}"#, "fast\nslow cancelled\ntask after did not finish within 0.05 seconds\ntrue\n");
}

#[test]
fn settled_gives_each_outcome_without_raising() {
    executes(r#"
takepkg std.async;
async func page(string name){
    await async.sleep(0.01);
    if (name == "bad"){ err(Error, "no page bad"); }
    ret name.upper();
}
func m{
    gone = page("gone");
    gone.cancel();
    for (outcome, await async.settled(arr(page("a"), page("bad"), gone))){
        out(outcome[0], outcome[1]);
    }
}"#, "true A\nfalse Error: no page bad\nfalse CancelledError: task page was cancelled\n");
}

#[test]
fn cancelling_a_combinator_cancels_what_it_waits_for() {
    executes(r#"
takepkg std.async;
async func child(string name){
    run { await async.sleep(5); } handle(CancelledError e){ out(name, "cancelled"); err(e); }
}
func m{
    group = async.all(arr(child("a"), child("b")));
    await async.sleep(0.01);
    group.cancel();
    run { await group; } handle(CancelledError e){ out("group cancelled"); }
}"#, "a cancelled\nb cancelled\ngroup cancelled\n");
}

#[test]
fn spawn_runs_a_plain_function_as_a_task() {
    executes(r#"
takepkg std.async;
async func later() => "from an async func";
func m{
    t = async.spawn(func() => 6 * 7);
    out(t, await t);
    out(await async.spawn(func() => later()));
}"#, "<task async.spawn> 42\nfrom an async func\n");
}

// ----- the runtime under load -----

#[test]
fn deep_recursion_inside_a_task_reaches_the_call_depth_limit_cleanly() {
    executes(r#"
func depth(int n){
    if (n == 0){ ret 0; }
    ret depth(n - 1) + 1;
}
async func deep(int n) => depth(n);
func endless(int n) => endless(n + 1);
async func runaway() => endless(0);
func m{
    out(await deep(9000));
    run { await runaway(); } handle(RangeError e){ out(e.message); }
    out(await deep(5000));
}"#, "9000\nmaximum call depth exceeded\n5000\n");
}

#[test]
fn a_thousand_tasks_run_at_once() {
    executes(r#"
takepkg std.async;
async func square(int i){
    await async.sleep(0.05);
    ret i * i;
}
func m{
    tasks = arr();
    for (i = 0, i < 1000, i++){ tasks.add(square(i)); }
    total = 0;
    for (r, await async.all(tasks)){ total += r; }
    out(total);
}"#, "332833500\n");
}

#[test]
fn the_collector_runs_safely_while_tasks_are_suspended() {
    executes(r#"
takepkg std.async;
takepkg std.memory;
async func holder(int i){
    kept = arr(i);
    kept.add(kept);
    data = arr(i, arr(i * 10));
    await async.sleep(0.05);
    // What the suspended task held is intact, cycle included.
    ret kept[0] + data[1][0] + int(memory.same(kept[1], kept));
}
func garbage(){
    repeat 200 { x = arr(); x.add(x); }
}
func m{
    tasks = arr();
    for (i = 0, i < 50, i++){ tasks.add(holder(i)); }
    await async.sleep(0.01);
    garbage();
    out(memory.collect() >= 200);
    total = 0;
    for (r, await async.all(tasks)){ total += r; }
    out(total);
}"#, "true\n13525\n");
}

#[test]
fn a_finished_task_in_a_cycle_is_collected() {
    executes(r#"
takepkg std.memory;
async func keep(array a) => a;
func m{
    repeat 300 {
        a = arr();
        t = keep(a);
        a.add(t);
        await t;
    }
    out(memory.collect() >= 300);
}"#, "true\n");
}

// ----- keywords -----

#[test]
fn async_and_await_are_keywords() {
    compile_error("func m{\n    x = 1;\n    async = 2;\n}", "line 3: 'async' is a keyword and cannot name a variable");
    compile_error("func m{\n    await = 2;\n}", "line 2: 'await' is a keyword and cannot name a variable");
    compile_error("func await(){}", "line 1: 'await' is a keyword and cannot name a function");
    compile_error("\nfunc f(int async) => 1;", "line 2: 'async' is a keyword and cannot name a parameter");
    compile_error("family async{}", "'async' is a keyword and cannot name a family");
    compile_error("func m{\n    for (await, arr(1)){ }\n}", "line 2: 'await' is a keyword and cannot name a loop variable");
    compile_error("func m{\n    x = async;\n}", "line 2: 'async' is a keyword");
    compile_error("func m{\n    x = await;\n}", "line 2: await needs a task to wait for");
    compile_error("family Box{\n    async func init(){ }\n}", "line 2: init cannot be async");
    compile_error("takepkg std.math = await;", "'await' is a keyword and cannot name an imported package");
}

#[test]
fn async_and_await_are_still_member_names() {
    executes("family B{ func init(){ me.await = 1; me.async = 2; } }\nfunc m{ b = B(); out(b.await + b.async); }", "3\n");
}

#[test]
fn std_async_is_imported_under_the_name_async() {
    executes("takepkg std.async;\nfunc m{ await async.sleep(0); out(async.TimeoutError); }", "<family TimeoutError>\n");
    executes("takepkg std.async = aio;\nfunc m{ await aio.sleep(0); out(\"ok\"); }", "ok\n");
}

#[test]
fn symbols_mark_async_functions() {
    let symbols = marslang::symbols("async func load(string url) => url;\nfunc plain() => 1;", std::path::Path::new("."))
        .expect("symbols");
    assert!(symbols.contains(r#""name":"load","family":null,"line":1,"end":1,"params":[{"name":"url","type":"string"}],"doc":null,"access":"public","async":true}"#), "{symbols}");
    assert!(symbols.contains(r#""name":"plain","family":null,"line":2,"end":2,"params":[],"doc":null,"access":"public"}"#), "{symbols}");
}

/// Every example in a page, in order, prints what its comments say: here
/// spelled out, since a task's output can come from a line without `out`.
fn documented_examples(doc: &str, expected: &[&str]) {
    let doc = doc.replace("\r\n", "\n");
    let blocks: Vec<&str> = doc.split("```mars\n").skip(1).map(|block| block.split("```").next().unwrap()).collect();
    assert_eq!(blocks.len(), expected.len(), "examples in the page");
    for (code, expected) in blocks.iter().zip(expected) { executes(code, expected); }
}

#[test]
fn the_async_page_examples_print_what_their_comments_say() {
    documented_examples(include_str!("../docs/api/async.md"), &[
        "coffee is ready\ntea is ready\n",
        "<task greet> false\nhello ada\ntrue hello ada\n",
        "12\n30\n",
        "nothing to parse\n",
        "true\ndownload closed\ntask download was cancelled\ntrue false\n",
    ]);
}

#[test]
fn the_language_page_example_prints_what_its_comment_says() {
    let doc = include_str!("../docs/api/language.md").replace("\r\n", "\n");
    let section = doc.split("\n## Tasks and await\n").nth(1).expect("a tasks section").split("\n## ").next().unwrap();
    documented_examples(section, &["first second\n"]);
}

#[test]
fn the_std_async_page_examples_print_what_their_comments_say() {
    documented_examples(include_str!("../docs/api/std-async.md"), &[
        "[\"A\", \"B\"]\nQUICK\ntask fetch did not finish within 0.05 seconds\n",
        "true 10\nfalse RangeError: -2 is negative\n",
        "a 1\nb 1\na 2\nb 2\n",
    ]);
}

#[test]
fn async_tests_run_with_marslang_test() {
    let compiled = marslang::compile(r#"
takepkg std.async;
async func test_waits(){
    await async.sleep(0.01);
    out("waited");
}
func test_leaves_a_failing_task(){
    async.spawn(func() => err(Error, "lost"));
}
func test_after(){ out("next test"); }
"#).unwrap();
    let outcomes = marslang::run_tests(compiled, "async_test.mars".into()).expect("tests ran");
    let summary: Vec<(String, bool, String)> = outcomes.iter()
        .map(|o| (o.name.clone(), o.error.is_none(), o.output.clone())).collect();
    assert_eq!(summary, vec![
        ("test_waits".to_string(), true, "waited\n".to_string()),
        ("test_leaves_a_failing_task".to_string(), false, String::new()),
        ("test_after".to_string(), true, "next test\n".to_string()),
    ]);
}
