# std.broadcast

Named channels that programs send messages on and listen to: within one
program, between the programs of one user on one computer, or across the
local network. Receiving is a task to `await`, so a program waits for
messages without holding up its other tasks (see [async](async.md)).

```mars
takepkg std.broadcast;
takepkg std.async;

func m{
    news = broadcast.channel("scores", "local");
    func listen(){ out(await news.receive()); }
    t = async.spawn(listen);
    news.send(map().set("player", "Ada").set("points", 10));
    await t;                                   // {"player": "Ada", "points": 10}
}
```

A channel object is made for a name and a **scope**, and from that moment it
both sends and receives: every message sent on its name and scope reaches it,
its own included. A message is any value JSON can hold, and every receiver
gets a copy of its own.

| Scope | Who sends and receives | Carried by |
| --- | --- | --- |
| `"local"` | This program only | Memory |
| `"machine"` | Programs of the same user on this computer; no other user | Unix domain sockets in a directory only the user can open |
| `"network"` | **Anyone** on the local network | UDP broadcast |

| Function | Result |
| --- | --- |
| `channel(name, scope)` | A `Channel` for `name` in `scope` |
| `network_port(name)` | The UDP port of the network channel `name`, from 41000 to 48999 |
| `machine_directory()` | The directory where machine channels register |

| Member of a `Channel` | Result |
| --- | --- |
| `send(value)` | Sends `value` to every receiver: how many it reached for `"local"` and `"machine"`, `null` for `"network"` |
| `receive()` | A task giving the next message |
| `receive_for(seconds)` | A task giving the next message, or `null` if none comes within `seconds`, a number or a `duration` |
| `on(f)` | Calls `f(message)`, as a task, for each message; returns a `Subscription` |
| `close()` | Stops listening; true, or false when already closed |
| `name`, `scope` | What it was made with |
| `registration` | For `"machine"`, the socket file that registers it; otherwise `null` |
| `closed` | Whether `close()` was called |

A `Subscription` has `stop()`, which returns true, or false when it was
already stopped, and `active`.

Names are 1 to 100 letters, digits, `-`, `_`, and `.`, as in `build.jobs` or
`sensor-3`. The scope is always written out: no scope is assumed, and the
network one is never chosen for you.

## Receiving

`receive()` gives the oldest message waiting for that channel object, or
waits for the next one; only the task that awaits it waits. Messages that
arrive while nothing is receiving are **queued per channel object, up to
1000; beyond that the oldest is dropped**. Since a channel object listens
from the moment it is made, a message sent before anything awaits `receive()`
is waiting for it, as in the first example.

```mars
takepkg std.broadcast;
takepkg std.async;

async func later(any ch){
    await async.sleep(0.05);
    ch.send("ready");
}

func m{
    jobs = broadcast.channel("jobs", "local");
    workers = broadcast.channel("jobs", "local");
    out(jobs.send(1), jobs.send(2));                 // 2 2
    out(await workers.receive(), await workers.receive());   // 1 2

    out(await workers.receive_for(0.01));           // null
    later(jobs);
    out(await workers.receive_for(1));              // ready
}
```

Messages from one sender arrive in the order it sent them. `receive_for`
gives `null` when time runs out, so a message that is `null` itself looks the
same; await `receive()` with `async.timeout` to tell them apart. A message
that arrives just as the time runs out is given, not dropped.

What `receive` gives is the value itself for `"local"` and `"machine"`, whose
senders are this program or this user's. For `"network"` it is a map that
also says who sent it, since anyone may have:

```text
{"message": {"temperature": 21}, "from": "192.168.1.5"}
```

## Subscriptions

`on(f)` calls `f(message)` for every message from then on, each call a task
of its own, so a slow handler holds up nothing; `f` may be an async function.
While a channel object has a subscription, its messages go to its
subscriptions, not to `receive()`. An error in a handler fails the program
when it ends, as for any task nothing awaited.

```mars
takepkg std.broadcast;
takepkg std.async;

func m{
    alerts = broadcast.channel("alerts", "local");
    sub = alerts.on(func(any message) => out("alert:", message));
    alerts.send("disk full");                       // alert: disk full
    await async.yield_now();
    sub.stop();
    alerts.send("ignored");
    out(await alerts.receive());                    // ignored
}
```

**Does a subscription keep the program running?** A `"machine"` or
`"network"` subscription does, as an open server does in Node.js: a program
whose `m` ends with one active keeps receiving until it is stopped or its
channel is closed, which a handler may do. A `"local"` subscription does not:
only the program itself can send on a local channel, so once its code and
tasks are done no message can come, and the program ends.

```mars
takepkg std.broadcast;

func m{
    commands = broadcast.channel("commands", "machine");
    sub = null;
    func obey(any command){
        out("doing", command);                      // doing stop
        if (command == "stop"){ sub.stop(); }
    }
    sub = commands.on(obey);
    commands.send("stop");                          // another program could send this
}
```

## Machine channels

A `"machine"` channel reaches every program of the same user on this
computer that has a channel object for its name, and no program of any other
user: that is a hard rule, enforced by the operating system.

- Each channel object listens on a Unix domain socket of its own, a file in
  `machine_directory()`, named by a hash of the channel name, its process id,
  and a counter. The file is its registration.
- A sender connects to every registration of the channel in turn and writes
  the message. `send` returns how many took it, the sending object included.
- No daemon runs: there is nothing to start, and nothing that outlives the
  programs. A program that ends closes its channel objects, which removes
  their files. A program that is killed cannot; the next sender finds that its
  process is gone and removes the file.
- A message is at most 1 MB (1,048,576 bytes) of JSON.

**Only the current user.** On Linux the directory is
`$XDG_RUNTIME_DIR/marslang-broadcast-UID`, a directory of the user's own, or
else `/tmp/marslang-broadcast-UID` (macOS: in `$TMPDIR`). It is made with
permissions `0700`, and it is refused unless it is a real directory, not a
link, that the user owns; if its permissions are wider they are narrowed to
`0700`. Each socket file is `0600`. Connecting to a socket needs search
permission on its directory, so no other user can send, and no other user
can listen, since listening means making a file there. On Windows (10 and
later, which have AF_UNIX sockets) the directory is
`%TEMP%\marslang-broadcast`, in the user's profile. Each time a program first
uses it, it is checked to be owned by the user, and given an access list,
not inherited from its parent, with one entry: full control for the user,
which the socket files inherit. Administrators, and root on Unix, can open
anything, these files included; they are outside what any program can keep
out.

```mars
takepkg std.broadcast;

func m{
    builds = broadcast.channel("build.done", "machine");
    out(builds.send(map().set("ok", true)));        // 1, or more with other listeners
    out(await builds.receive());                    // {"ok": true}
    out(builds.registration != null);               // true
    builds.close();
}
```

## Network channels

**Anyone on the local network can send and read network messages.** They are
not encrypted or authenticated, and nothing stops a stranger sending one
that looks like yours, so do not use them for secrets or to decide anything
that matters without checking. Use them for things like finding other
programs, or announcing what is not private.

- Messages are UDP datagrams to the broadcast address `255.255.255.255`, on
  port `network_port(name)`, a number from 41000 to 48999 that comes from the
  name and is the same on every computer, so a firewall can be opened for it.
  Each channel object binds that port, shared, so several programs and
  objects on one computer all hear each message.
- A message is at most 65,507 bytes, the most one UDP datagram holds, with a
  short header naming the channel; a larger one raises `RangeError`.
- What arrives is untrusted: a datagram that is not a message of this
  channel, or not JSON within std.json's limits (such as how deeply it nests),
  is dropped. What `receive` gives is `{"message": value, "from": address}`.
- UDP may lose, duplicate, or reorder messages, and `send` cannot know who
  heard one, so it returns `null`.
- The environment variable `MARSLANG_BROADCAST_ADDRESS` replaces the address:
  a subnet's broadcast address such as `192.168.1.255`, which some networks
  need, or `127.0.0.1` to keep messages on this computer, binding only the
  loopback interface. Routers do not pass broadcasts on, so network channels
  reach one local network only.

```mars
takepkg std.broadcast;

func m{
    hello = broadcast.channel("hello", "network");
    out(broadcast.network_port("hello"));           // 46672
    out(hello.send("anyone there?"));               // null
    reply = await hello.receive_for(1);             // our own message comes back
    out(reply.get("message"));                      // anyone there?
    hello.close();
}
```

## Closing

`close()` stops a channel object listening: its queued messages are dropped,
its subscriptions stop, a `receive` waiting on it raises `ClosedError`, and a
machine registration is removed. Using a closed channel raises `ClosedError`.
Channel objects close by themselves when the program ends, but a program
that makes many should close the ones it is done with: each local one keeps
receiving copies, and each machine or network one a socket and a thread.

## Errors

| Error | When |
| --- | --- |
| `RangeError` | A name that is not 1 to 100 letters, digits, `-`, `_`, or `.`; a scope other than `"local"`, `"machine"`, or `"network"`; a message over the scope's limit; a negative number of seconds |
| `TypeError` | A value JSON cannot hold, such as a set, a pair, a function, or a map with keys that are not strings |
| `broadcast.ClosedError` | The channel object was closed; inherits from `Error` |
| `Error` | The machine directory is not safe to use, or a socket or thread could not be made |
| `CancelledError` | The task awaiting `receive` or `receive_for` was cancelled; no message is lost |

```mars
takepkg std.broadcast;

func m{
    run { broadcast.channel("two words", "local"); } handle(RangeError e){
        out(e.message);   // broadcast.channel needs a name of 1 to 100 letters, digits, '-', '_', or '.', got "two words"
    }
    run { broadcast.channel("news", "everywhere"); } handle(RangeError e){
        out(e.message);   // broadcast.channel needs the scope "local", "machine", or "network", got "everywhere"
    }
    run { broadcast.channel("news", "local").send(set(1)); } handle(TypeError e){
        out(e.message);   // send needs a value JSON can hold: JSON has no sets; write an array of the items instead
    }
}
```

## How it works

Channels, the local scope, the queues of local channel objects, copying
(each receiver parses the JSON text that `send` wrote once), `receive_for`,
and subscriptions are Marslang ([std/broadcast.mars](../../std/broadcast.mars)).
Native ([std/rs/broadcast.rs](../../std/rs/broadcast.rs)) are only what
Marslang cannot do:

- Waking a task that waits on a local channel: a runtime `Signal`.
- The sockets. Each machine or network channel object has a background
  thread that reads its socket and queues messages as text; no value crosses
  threads. A task that awaits `receive` waits for that queue on a worker
  thread, so only it waits, and it takes the message on the interpreter
  thread, so a cancelled receive loses nothing. Closing the channel, or the
  end of the program, stops the thread and removes the registration.
- On Windows, AF_UNIX sockets and the directory's access list, through
  windows-sys, since Rust's standard library has neither there.

A machine subscription keeps the program running because a task waits for
its messages on a worker thread, which the event loop waits for; a local one
waits for nothing, so none is left once the program is done.
