//! std.broadcast: local channels in one program, machine channels between
//! processes of this user (a second `marslang` process is started), and
//! network channels over UDP on 127.0.0.1 (MARSLANG_BROADCAST_ADDRESS), so
//! the tests need no network. A test of real broadcast is #[ignore]d: CI
//! machines often have no broadcast route, and on Windows binding a public
//! UDP port asks the firewall.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn executes(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("Marslang compilation failed");
    let (stdout, result) = marslang::run_captured(compiled, "");
    if let Err(error) = result { panic!("runtime error: {error}\noutput so far:\n{stdout}"); }
    assert_eq!(stdout, expected);
}

fn output(source: &str) -> String {
    let compiled = marslang::compile(source).expect("Marslang compilation failed");
    let (stdout, result) = marslang::run_captured(compiled, "");
    if let Err(error) = result { panic!("runtime error: {error}\noutput so far:\n{stdout}"); }
    stdout
}

fn runtime_error(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("compile failed");
    let (_, result) = marslang::run_captured(compiled, "");
    let error = result.expect_err(&format!("expected runtime error: {source}")).to_string();
    assert!(error.contains(expected), "expected {expected}, got {error}");
}

/// A channel name no other test or run uses.
fn unique(tag: &str) -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    format!("test-{tag}-{}-{nanos}", std::process::id())
}

/// A fresh directory for a child program and the files it writes.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(unique(tag));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Start `marslang PROGRAM` in `dir`, with `source` as the program.
fn spawn(dir: &Path, source: &str, env: &[(&str, &str)]) -> Child {
    std::fs::write(dir.join("child.mars"), source).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_marslang"));
    command.arg("child.mars").current_dir(dir).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (key, value) in env { command.env(key, value); }
    command.spawn().expect("start marslang")
}

/// Wait until the child writes `READY`, and give what it wrote.
fn ready(dir: &Path, child: &mut Child) -> String {
    let started = Instant::now();
    loop {
        if let Ok(text) = std::fs::read_to_string(dir.join("READY")) {
            if !text.is_empty() { return text; }
        }
        if let Ok(Some(status)) = child.try_wait() {
            let mut errors = String::new();
            use std::io::Read;
            child.stderr.take().unwrap().read_to_string(&mut errors).unwrap();
            panic!("the child ended before it was ready ({status}): {errors}");
        }
        assert!(started.elapsed() < Duration::from_secs(30), "the child was not ready within 30 seconds");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn finished(child: Child) -> String {
    let result = child.wait_with_output().unwrap();
    let (stdout, stderr) = (String::from_utf8_lossy(&result.stdout).into_owned(), String::from_utf8_lossy(&result.stderr).into_owned());
    assert!(result.status.success(), "the child failed: {stderr}\noutput:\n{stdout}");
    stdout.replace("\r\n", "\n")
}

// ----- local -----

#[test]
fn the_example_from_the_brief_receives_with_await() {
    executes(r#"
        takepkg std.broadcast;
        takepkg std.async;

        func m{
            news = broadcast.channel("scores", "local");
            func listen(){ out(await news.receive()); }
            t = async.spawn(listen);
            news.send(map().set("player", "Ada").set("points", 10));
            await t;
        }
    "#, "{\"player\": \"Ada\", \"points\": 10}\n");
}

#[test]
fn every_local_receiver_of_a_name_gets_each_message_and_no_other_name_does() {
    executes(r#"
        takepkg std.broadcast;

        func m{
            a = broadcast.channel("room", "local");
            b = broadcast.channel("room", "local");
            c = broadcast.channel("room", "local");
            other = broadcast.channel("hall", "local");
            out(a.send("hello"));
            out(await a.receive(), await b.receive(), await c.receive());
            out(await other.receive_for(0.01));
            out(other.send(1));
            b.close();
            out(a.send("again"));
        }
    "#, "3\nhello hello hello\nnull\n1\n2\n");
}

#[test]
fn local_receivers_get_copies_that_do_not_share_the_senders_value() {
    executes(r#"
        takepkg std.broadcast;

        func m{
            sender = broadcast.channel("copies", "local");
            first = broadcast.channel("copies", "local");
            second = broadcast.channel("copies", "local");
            original = map().set("items", arr(1, 2));
            sender.send(original);
            mine = await first.receive();
            mine.get("items").add(3);
            mine.set("extra", true);
            out(original, await second.receive());
            out(mine);
            out(mine == original);
        }
    "#, "{\"items\": [1, 2]} {\"items\": [1, 2]}\n{\"items\": [1, 2, 3], \"extra\": true}\nfalse\n");
}

#[test]
fn receive_for_gives_null_on_timeout_and_the_message_when_one_comes_in_time() {
    executes(r#"
        takepkg std.broadcast;
        takepkg std.async;
        takepkg std.time;

        async func later(any ch, float seconds, any value){
            await async.sleep(seconds);
            ch.send(value);
        }

        func m{
            ch = broadcast.channel("timing", "local");
            out(await ch.receive_for(0.02));
            later(ch, 0.02, "soon");
            out(await ch.receive_for(5));
            out(await ch.receive_for(time.milliseconds(10)));
            // A message that arrives after a timeout is kept for the next receive.
            later(ch, 0.05, "late");
            out(await ch.receive_for(0.01));
            out(await ch.receive());
        }
    "#, "null\nsoon\nnull\nnull\nlate\n");
}

#[test]
fn local_messages_arrive_in_order_and_the_queue_keeps_the_newest_thousand() {
    executes(r#"
        takepkg std.broadcast;

        func m{
            ch = broadcast.channel("order", "local");
            for (i = 1, i <= 5, i++){ ch.send(i); }
            got = arr();
            repeat 5 { got.add(await ch.receive()); }
            out(got);
            for (i = 0, i < 1005, i++){ ch.send(i); }
            out(await ch.receive());
            count = 1;
            while (await ch.receive_for(0) != null){ count += 1; }
            out(count);
        }
    "#, "[1, 2, 3, 4, 5]\n5\n1000\n");
}

#[test]
fn on_calls_the_function_for_each_message_until_stopped_and_does_not_keep_a_local_program_alive() {
    executes(r#"
        takepkg std.broadcast;
        takepkg std.async;

        async func slowly(any message){
            await async.sleep(0.01);
            out("async handler", message);
        }

        func m{
            ch = broadcast.channel("events", "local");
            other = broadcast.channel("events", "local");
            sub = ch.on(func(any message) => out("handler", message));
            other.send("one");
            other.send("two");
            await async.yield_now();
            out(sub.stop(), sub.stop());
            other.send("three");
            out("receive", await ch.receive());
            ch.on(slowly);
            other.send("four");
            // m ends here: the handler runs, then the program ends, as no
            // local subscription keeps it running.
        }
    "#, "handler one\nhandler two\ntrue false\nreceive three\nasync handler four\n");
}

#[test]
fn closing_stops_receivers_and_using_a_closed_channel_raises_closed_error() {
    executes(r#"
        takepkg std.broadcast;
        takepkg std.async;

        func waiting(any ch){
            run { await ch.receive(); } handle(broadcast.ClosedError e){ out("waiting:", e.message); }
        }

        func m{
            ch = broadcast.channel("closing", "local");
            t = async.spawn(func() => waiting(ch));
            await async.yield_now();
            out(ch.close(), ch.close());
            await t;
            run { ch.send(1); } handle(broadcast.ClosedError e){ out(e.message); }
            run { ch.on(func(any x) => x); } handle(broadcast.ClosedError e){ out(e.message); }
            run { await ch.receive_for(1); } handle(broadcast.ClosedError e){ out(e.message); }
            out(broadcast.channel("closing", "local").send(2));
        }
    "#, "true false\nwaiting: channel \"closing\" is closed, so it cannot receive\n\
         channel \"closing\" is closed, so it cannot send\n\
         channel \"closing\" is closed, so it cannot subscribe\n\
         channel \"closing\" is closed, so it cannot receive\n1\n");
}

// ----- errors -----

#[test]
fn channel_names_must_be_1_to_100_letters_digits_dashes_underscores_or_dots() {
    let message = "broadcast.channel needs a name of 1 to 100 letters, digits, '-', '_', or '.'";
    for name in ["", "two words", "slash/name", "é", &"x".repeat(101)] {
        runtime_error(&format!("takepkg std.broadcast;\nfunc m{{ broadcast.channel(\"{name}\", \"local\"); }}"), message);
    }
    executes(&format!(r#"
        takepkg std.broadcast;
        func m{{ out(broadcast.channel("{}", "local").name.len(), broadcast.channel("a-b_c.D9", "local").name); }}
    "#, "x".repeat(100)), "100 a-b_c.D9\n");
    runtime_error("takepkg std.broadcast;\nfunc m{ broadcast.channel(5, \"local\"); }", "TypeError");
}

#[test]
fn an_unknown_scope_raises_range_error_listing_the_three() {
    runtime_error(r#"
        takepkg std.broadcast;
        func m{ broadcast.channel("x", "planet"); }
    "#, r#"RangeError: broadcast.channel needs the scope "local", "machine", or "network", got "planet""#);
    // No scope is assumed.
    runtime_error(r#"
        takepkg std.broadcast;
        func m{ broadcast.channel("x", "Network"); }
    "#, "RangeError");
}

#[test]
fn values_json_cannot_hold_raise_when_sent() {
    for (value, expected) in [
        ("set(1, 2)", "TypeError: send needs a value JSON can hold: JSON has no sets"),
        ("func(any x) => x", "TypeError: send needs a value JSON can hold: a function"),
        ("pair(1, 2)", "TypeError: send needs a value JSON can hold"),
        ("map().set(1, 2)", "TypeError: send needs a value JSON can hold"),
        ("float(\"inf\")", "RangeError: send needs a value JSON can hold"),
    ] {
        runtime_error(&format!("takepkg std.broadcast;\nfunc m{{ broadcast.channel(\"j\", \"local\").send({value}); }}"), expected);
    }
    runtime_error(r#"
        takepkg std.broadcast;
        func m{ broadcast.channel("j", "local").receive_for(-1); }
    "#, "RangeError: receive_for needs a nonnegative number of seconds, got -1");
}

#[test]
fn machine_messages_over_a_megabyte_raise_range_error() {
    runtime_error(r#"
        takepkg std.broadcast;
        takepkg std.strings;
        func m{ broadcast.channel("big", "machine").send(strings.repeated("x", 1048576)); }
    "#, "RangeError: the message is 1048578 bytes as JSON; a machine channel carries at most 1048576");
}

// ----- machine -----

#[test]
fn machine_channels_deliver_between_two_processes_both_ways() {
    let name = unique("both-ways");
    let dir = scratch("both-ways");
    let mut child = spawn(&dir, &r#"
        takepkg std.broadcast;
        takepkg std.file;

        func m{
            inbox = broadcast.channel("NAME", "machine");
            reply = broadcast.channel("NAME-reply", "machine");
            file.write("READY", "ready");
            message = await inbox.receive_for(30);
            out("child got", message);
            out("reply reached", reply.send(map().set("echo", message)));
        }
    "#.replace("NAME", &name), &[]);
    ready(&dir, &mut child);
    let parent = output(&r#"
        takepkg std.broadcast;

        func m{
            reply = broadcast.channel("NAME-reply", "machine");
            // The child's channel object, and this one, which hears itself.
            out(broadcast.channel("NAME", "machine").send(map().set("n", 7)));
            out(await reply.receive_for(30));
        }
    "#.replace("NAME", &name));
    assert_eq!(parent, "2\n{\"echo\": {\"n\": 7}}\n");
    // The child's reply object, and the parent's.
    assert_eq!(finished(child), "child got {\"n\": 7}\nreply reached 2\n");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_machine_on_subscription_keeps_its_program_running_until_stopped() {
    let name = unique("on");
    let dir = scratch("on");
    let mut child = spawn(&dir, &r#"
        takepkg std.broadcast;
        takepkg std.file;

        func m{
            ch = broadcast.channel("NAME", "machine");
            sub = null;
            func handle(any message){
                out("got", message);
                if (message == "last"){ sub.stop(); }
            }
            sub = ch.on(handle);
            file.write("READY", "ready");
            // m ends here, but the subscription keeps the program running.
        }
    "#.replace("NAME", &name), &[]);
    ready(&dir, &mut child);
    let sent = output(&r#"
        takepkg std.broadcast;
        func m{
            ch = broadcast.channel("NAME", "machine");
            out(ch.send("first"), ch.send("last"));
        }
    "#.replace("NAME", &name));
    assert_eq!(sent, "2 2\n");
    assert_eq!(finished(child), "got first\ngot last\n");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_registration_left_by_a_program_that_died_is_removed_by_the_next_sender() {
    let name = unique("dead");
    let dir = scratch("dead");
    let mut child = spawn(&dir, &r#"
        takepkg std.broadcast;
        takepkg std.file;
        takepkg std.async;

        func m{
            ch = broadcast.channel("NAME", "machine");
            file.write("READY", ch.registration);
            await async.sleep(60);
        }
    "#.replace("NAME", &name), &[]);
    let registration = PathBuf::from(ready(&dir, &mut child));
    assert!(registration.exists(), "{}", registration.display());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&registration).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "socket file mode {mode:o}");
    }
    child.kill().unwrap();
    child.wait().unwrap();
    // Killed, the program could not remove its registration.
    assert!(registration.exists());
    // This program's own object is the only one that hears it.
    executes(&format!(r#"
        takepkg std.broadcast;
        func m{{ out(broadcast.channel("{name}", "machine").send("anyone?")); }}
    "#), "1\n");
    assert!(!registration.exists(), "the dead registration was not removed");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_machine_receive_that_times_out_or_is_cancelled_loses_no_message() {
    executes(&format!(r#"
        takepkg std.broadcast;
        takepkg std.async;

        func m{{
            ch = broadcast.channel("{}", "machine");
            out(await ch.receive_for(0.05));
            t = ch.receive();
            await async.sleep(0.05);
            out(t.cancel());
            out(ch.send("kept"), ch.send("next"));
            out(await ch.receive_for(5), await ch.receive());
        }}
    "#, unique("cancel")), "null\ntrue\n1 1\nkept next\n");
}

#[test]
fn closing_a_machine_channel_removes_its_registration() {
    executes(r#"
        takepkg std.broadcast;
        takepkg std.file;

        func m{
            ch = broadcast.channel("closing-machine", "machine");
            path = ch.registration;
            out(file.exists(path));
            ch.close();
            out(file.exists(path), ch.registration == path);
        }
    "#, "true\nfalse true\n");
}

/// Only the current user can open the directory machine channels register in:
/// on Unix it is 0700 and the user's own; on Windows its access list is not
/// inherited and has one entry, for the user.
#[test]
fn the_machine_directory_is_private_to_the_current_user() {
    let dir = PathBuf::from(output("takepkg std.broadcast;\nfunc m{ slout(broadcast.machine_directory()); }"));
    assert!(dir.is_dir(), "{}", dir.display());
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let meta = std::fs::symlink_metadata(&dir).unwrap();
        assert!(meta.file_type().is_dir());
        assert_eq!(meta.permissions().mode() & 0o777, 0o700, "mode {:o}", meta.permissions().mode());
        // Owned by whoever runs the tests: the same owner as a file made now.
        let probe = std::env::temp_dir().join(unique("owner"));
        std::fs::write(&probe, "").unwrap();
        assert_eq!(meta.uid(), std::fs::metadata(&probe).unwrap().uid());
        std::fs::remove_file(probe).unwrap();
    }
    #[cfg(windows)]
    {
        let listing = Command::new("icacls").arg(&dir).output().expect("run icacls");
        let text = String::from_utf8_lossy(&listing.stdout).into_owned();
        let entries: Vec<&str> = text.lines().filter(|line| line.contains(":(")).collect();
        assert_eq!(entries.len(), 1, "{text}");
        let user = std::env::var("USERNAME").unwrap().to_lowercase();
        assert!(entries[0].to_lowercase().contains(&format!("\\{user}:(oi)(ci)(f)")), "{text}");
        // "(I)" would mark an entry inherited from the parent directory.
        assert!(!entries[0].contains("(I)"), "{text}");
    }
}

// ----- network -----

#[test]
fn network_ports_come_from_the_channel_name() {
    let ports = output(r#"
        takepkg std.broadcast;
        func m{
            out(broadcast.network_port("hello"), broadcast.network_port("hello"));
            for (name, arr("a", "b", "scores", "build.jobs")){
                p = broadcast.network_port(name);
                out(p >= 41000 and p <= 48999);
            }
        }
    "#);
    assert_eq!(ports, "46672 46672\ntrue\ntrue\ntrue\ntrue\n");
}

/// Network messages are untrusted: a datagram that is not a message of this
/// channel, or not JSON, is dropped; what receive gives names the sender.
#[test]
fn network_channels_on_loopback_drop_what_is_not_a_message_and_name_the_sender() {
    let name = unique("net");
    let dir = scratch("net");
    let mut child = spawn(&dir, &r#"
        takepkg std.broadcast;
        takepkg std.file;
        takepkg std.strings;

        func m{
            ch = broadcast.channel("NAME", "network");
            out(ch.send(arr(1, 2)));
            out(await ch.receive_for(10));
            run {
                ch.send(strings.repeated("x", 70000));
            } handle(RangeError e){
                out(e.message.starts_with("the message is "), e.message.ends_with("a network channel carries at most 65507, the most one UDP datagram holds"));
            }
            file.write("READY", string(broadcast.network_port("NAME")));
            repeat 2 {
                got = await ch.receive_for(30);
                out(got.get("message"), got.get("from"));
            }
        }
    "#.replace("NAME", &name), &[("MARSLANG_BROADCAST_ADDRESS", "127.0.0.1")]);
    let port: u16 = ready(&dir, &mut child).trim().parse().unwrap();
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let send = |bytes: &[u8]| { socket.send_to(bytes, ("127.0.0.1", port)).unwrap(); };
    send(b"not a marslang message");
    send(format!("marslang-broadcast 1 {name}-other\n\"another channel\"").as_bytes());
    send(format!("marslang-broadcast 1 {name}\n{{not json").as_bytes());
    send(format!("marslang-broadcast 1 {name}\n{}", "[".repeat(5000) + &"]".repeat(5000)).as_bytes());
    send(&[0xff, 0xfe, 0x00]);
    send(format!("marslang-broadcast 1 {name}\n{{\"a\": 1}}").as_bytes());
    send(format!("marslang-broadcast 1 {name}\n\"second\"").as_bytes());
    assert_eq!(finished(child), "null\n{\"message\": [1, 2], \"from\": \"127.0.0.1\"}\ntrue true\n{\"a\": 1} 127.0.0.1\nsecond 127.0.0.1\n");
    let _ = std::fs::remove_dir_all(dir);
}

/// Real broadcast on the local network: needs a network with a broadcast
/// route, and on Windows the firewall asks to let marslang listen. Run with
/// `cargo test -- --ignored`.
#[test]
#[ignore]
fn network_channels_broadcast_between_two_processes() {
    let name = unique("lan");
    let dir = scratch("lan");
    let mut child = spawn(&dir, &r#"
        takepkg std.broadcast;
        takepkg std.file;

        func m{
            ch = broadcast.channel("NAME", "network");
            file.write("READY", "ready");
            got = await ch.receive_for(30);
            out(got.get("message"));
        }
    "#.replace("NAME", &name), &[]);
    ready(&dir, &mut child);
    executes(&format!(r#"
        takepkg std.broadcast;
        func m{{ out(broadcast.channel("{name}", "network").send("hello, network")); }}
    "#), "null\n");
    assert_eq!(finished(child), "hello, network\n");
    let _ = std::fs::remove_dir_all(dir);
}
