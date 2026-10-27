# std.async

Timers and ways to wait for several tasks at once. Tasks themselves, `async
func`, and `await` are part of the language: see [async](async.md).

```mars
takepkg std.async;

async func fetch(string name, float seconds){
    await async.sleep(seconds);
    ret name.upper();
}

func m{
    pages = await async.all(arr(fetch("a", 0.02), fetch("b", 0.01)));
    out(pages);                                             // ["A", "B"]

    fastest = await async.race(arr(fetch("slow", 1), fetch("quick", 0.01)));
    out(fastest);                                           // QUICK

    run {
        await async.timeout(fetch("late", 1), 0.05);
    } handle(async.TimeoutError e){
        out(e.message);                                     // task fetch did not finish within 0.05 seconds
    }
}
```

The whole program takes about 0.1 seconds: the waits overlap.

| Function | Result |
| --- | --- |
| `sleep(seconds)` | A task that finishes after `seconds`, a number or a duration |
| `all(tasks)` | A task whose result is the tasks' results, in order |
| `race(tasks)` | A task whose result is that of the first of the tasks to finish |
| `timeout(t, seconds)` | A task whose result is `t`'s, if it finishes within `seconds` |
| `settled(tasks)` | A task whose result is `arr(true, result)` or `arr(false, error)` for each task |
| `spawn(f)` | A task that calls `f`, a function of no arguments |
| `yield_now()` | A task that finishes once the tasks ready before it have had a turn |
| `work(seconds)` | A task that waits `seconds` on a worker thread |
| `TimeoutError` | Raised by `timeout` when time runs out |

Every function here that waits returns a task to `await`, so they combine:
`async.race(arr(async.all(downloads), async.sleep(5)))` gives up after five
seconds. Each checks its arguments when it is called, so a negative number of
seconds or an array holding something other than tasks raises at the call.

## Waiting for several tasks

- `all` waits for every task. When one fails, `all` cancels the others, waits
  for them to finish, and raises the first error, as that task's own `await`
  would, with its trace.
- `race` gives the result of the first task to finish, after cancelling the
  others and waiting for them to finish. When the first to finish failed, its
  error is raised. It needs at least one task.
- `timeout` gives the task's result, or raises its error, when it finishes in
  time. When time runs out it cancels the task, waits for it to finish, and
  raises `TimeoutError`.
- `settled` waits for every task and raises nothing for their errors:

```mars
takepkg std.async;

async func check(int n){
    if (n < 0){ err(RangeError, f"{n} is negative"); }
    ret n * 10;
}

func m{
    for (outcome, await async.settled(arr(check(1), check(-2)))){
        out(outcome[0], outcome[1]);    // true 10, then false RangeError: -2 is negative
    }
}
```

The errors that `all`, `race`, and `timeout` leave out, of the tasks they
cancel, do not count as errors nothing awaited at the end of the program.
Cancelling the task one of them returned cancels the tasks it waits for, and
it finishes once they have.

## Sleeping and taking turns

`sleep` waits on a timer: only the task that awaits it waits, and the others
run meanwhile. It takes a number of seconds of any numeric kind, or a
`duration` such as `time.milliseconds(250)`. `time.sleep`, from
[std.time](std-time.md), does the same without an `await`: inside a task it
pauses only that task, and outside tasks it stops the program.

```mars
takepkg std.async;

async func count(string name){
    for (i = 1, i <= 2, i++){
        out(name, i);                   // a 1, b 1, a 2, b 2
        await async.yield_now();
    }
}

func m{
    await async.all(arr(count("a"), count("b")));
}
```

`yield_now` lets the tasks that are ready run before the one that awaits it
goes on; a long computation in a task should await it now and then.

`spawn(f)` runs a function that is not async as a task: `async.spawn(func() =>
total(numbers))`. When `f` returns a task, as an async function does, the
spawned task's result is that task's.

`work(seconds)` waits on a worker thread instead of a timer. It shows the way
native packages wait for slow work, such as a download, without holding up
other tasks: ten `work(0.2)` tasks awaited together take 0.2 seconds.

## Errors

| Error | When |
| --- | --- |
| `TypeError` | An argument of the wrong type, or an array that holds something other than tasks |
| `RangeError` | A negative or NaN number of seconds, or `race` with no tasks |
| `async.TimeoutError` | `timeout` ran out of time |
| `CancelledError` | The task awaiting one of these functions was cancelled, or a task it waited for was |

`TimeoutError` inherits from `Error`; its message names the task and the time,
as in `task fetch did not finish within 0.05 seconds`.

## How it works

The waiting is native ([std/rs/async.rs](../../std/rs/async.rs)) on the
interpreter's event loop ([src/runtime.rs](../../src/runtime.rs)): a timer,
waiting for the first of several tasks to finish, and a wait on a worker
thread. `all`, `race`, `timeout`, `settled`, `spawn`, and `yield_now` are
Marslang ([std/async.mars](../../std/async.mars)) built from those. A timer
costs nothing while it waits; while no task is ready, the event loop sleeps
until the next timer is due or a worker thread finishes.
