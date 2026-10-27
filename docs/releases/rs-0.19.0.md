# rs-0.19.0 — async and await, programs, and broadcast channels

Programs can now do several things at once: [async and await](../api/async.md)
run tasks that wait without blocking each other, HTTP requests, sleeps, and
the new [std.process](../api/std-process.md) wait inside a task while the rest
of the program goes on, and [std.broadcast](../api/std-broadcast.md) sends
messages within a program, to the other Marslang programs on a computer, or
across the network.

## async and await

```mars
takepkg std.async;

async func slow(string name, float seconds){
    await async.sleep(seconds);
    ret name;
}

func m{
    out(await async.all(arr(slow("a", 0.2), slow("b", 0.2))));   // ["a", "b"]
}
```

Both tasks sleep at the same time, so this takes 0.2 seconds, not 0.4.

- **Calling and awaiting**
  - Calling an `async func` starts a task and returns it at once.
  - `await` waits for a task, giving its result or raising its error. It works anywhere, in `m` and in ordinary functions too, so async does not spread through a program the way it does in JavaScript.
- **The task kind**: `done()`, `result()`, and `cancel()`. Cancelling raises `CancelledError` at the task's next `await`, so its `then` blocks run.
- **[std.async](../api/std-async.md)**: `sleep`, `all`, `race`, `timeout`, `settled`, `spawn`, and `yield_now`.
- **End of the program and errors**
  - Tasks still running when `m` ends finish first.
  - An error that nothing awaited fails the program, rather than vanishing.
  - Traces say which task was waiting: `at m (app.mars:11), awaiting load`.
- **Cost**: each task runs on its own stack, about 12–25 KB of memory. Synchronous programs are no slower.

## Waiting without blocking

Inside a task, every `std.http` request, `time.sleep`, and std.process run
waits on its own while other tasks go on. Five slow requests take as long as the
slowest, not their sum. Outside tasks, nothing changes.

## std.process

```mars
takepkg std.process;

func m{
    result = process.run(arr("git", "--version"));
    out(result.ok);   // true
}
```

`run`, `run_in` (another folder), `run_with` (text for standard input),
`check` (raises unless the program succeeded), `which`, and `set_timeout`.
There is no shell: every array element is one argument, so text from a user can
never become a command.

## std.broadcast

```mars
takepkg std.async;
takepkg std.broadcast;

func m{
    news = broadcast.channel("scores", "local");
    func listen(){ out(await news.receive()); }
    t = async.spawn(listen);
    news.send(map().set("player", "Ada"));
    await t;   // {"player": "Ada"}
}
```

Channels carry any JSON value, and there are three scopes:
- `"local"` reaches the same program.
- `"machine"` reaches every Marslang program you run on this computer. It uses a folder only your user can open, and no background service, so other users cannot send or listen.
- `"network"` uses UDP broadcast on the local network. Only use it when you mean it, since anyone there can read and send. Messages are limited in size, and malformed ones are dropped.

Receiving is a task: `receive`, `receive_for`, `on`, and `close`.

## Smaller changes

- A block may open with its `{` on the next line, as well as on the header's line, for every block.
- `async` and `await` are now keywords. `takepkg std.async;` still gives `async.sleep` and the other functions.

## Validation

`cargo test` passes 501 tests on Windows, among them tasks under the collector,
1,000 concurrent tasks, and broadcasts between two running programs. Every push
runs them on Windows, macOS, and Linux on x86_64 and ARM64, and runs the static
Linux binary inside Alpine, Debian 11, Ubuntu 20.04, Rocky Linux 8, and
Amazon Linux 2.
