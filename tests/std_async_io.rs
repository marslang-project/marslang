//! Waiting I/O inside tasks: std.http requests and time.sleep suspend only the
//! task that waits, so tasks that wait together take as long as the slowest.
//! A small HTTP server on 127.0.0.1 answers after a delay; nothing here needs
//! the internet.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

fn executes(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("Marslang compilation failed");
    let (stdout, result) = marslang::run_captured(compiled, "");
    if let Err(error) = result { panic!("runtime error: {}\noutput so far:\n{stdout}", error.report()); }
    assert_eq!(stdout, expected);
}

fn timed(source: &str, expected: &str) -> Duration {
    let started = Instant::now();
    executes(source, expected);
    started.elapsed()
}

// ----- the server -----

fn read_request(reader: &mut BufReader<TcpStream>) -> Option<(String, String, Vec<u8>)> {
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 { return None; }
    let mut parts = line.trim_end().splitn(3, ' ');
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() { break; }
        let (name, value) = header.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("content-length") { length = value.trim().parse().ok()?; }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some((method, target, body))
}

fn reply(stream: &mut TcpStream, status: &str, body: &[u8]) {
    let head = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

fn handle(stream: TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut stream = stream;
    let Some((method, target, body)) = read_request(&mut reader) else { return };
    match target.as_str() {
        // Answers after 300 ms.
        "/delay" => { thread::sleep(Duration::from_millis(300)); reply(&mut stream, "200 OK", b"late"); }
        "/echo" => {
            thread::sleep(Duration::from_millis(300));
            reply(&mut stream, "200 OK", format!("{method} {}", String::from_utf8_lossy(&body)).as_bytes());
        }
        "/json" => { thread::sleep(Duration::from_millis(300)); reply(&mut stream, "200 OK", br#"{"n": 7}"#); }
        "/file" => { thread::sleep(Duration::from_millis(300)); reply(&mut stream, "200 OK", b"the file"); }
        // A body that arrives over about 3 seconds, in 60 pieces.
        "/trickle" => {
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 60000\r\nConnection: close\r\n\r\n");
            for _ in 0..60 {
                if stream.write_all(&[b'x'; 1000]).and_then(|_| stream.flush()).is_err() { return; }
                thread::sleep(Duration::from_millis(50));
            }
        }
        _ => reply(&mut stream, "404 Not Found", b"unknown path"),
    }
}

fn server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() { thread::spawn(move || handle(stream)); }
    });
    port
}

fn closed_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn with_port(source: &str, port: u16) -> String { source.replace("PORT", &port.to_string()) }

fn temp_dir(name: &str) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("marslang-asyncio-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shown = dir.to_string_lossy().replace('\\', "/");
    (dir, shown)
}

// ----- http -----

#[test]
fn five_requests_in_tasks_take_as_long_as_one() {
    let port = server();
    let elapsed = timed(&with_port(r#"
        takepkg std.http;
        takepkg std.async;
        async func fetch(int n) => http.get("http://127.0.0.1:PORT/delay").text + string(n);
        func m{
            out(await async.all(arr(fetch(1), fetch(2), fetch(3), fetch(4), fetch(5))));
        }"#, port),
        "[\"late1\", \"late2\", \"late3\", \"late4\", \"late5\"]\n");
    assert!(elapsed >= Duration::from_millis(300), "took {elapsed:?}");
    assert!(elapsed < Duration::from_millis(700), "took {elapsed:?}");
}

#[test]
fn every_kind_of_request_waits_without_blocking() {
    let port = server();
    let (dir, shown) = temp_dir("kinds");
    let source = with_port(r#"
        takepkg std.http;
        takepkg std.async;
        takepkg std.file;
        async func get() => http.get("http://127.0.0.1:PORT/delay").text;
        async func post() => http.post("http://127.0.0.1:PORT/echo", "hi").text;
        async func post_json() => http.post_json("http://127.0.0.1:PORT/echo", map().set("a", 1)).text;
        async func request() => http.request("PUT", "http://127.0.0.1:PORT/echo", map(), "put body").text;
        async func get_json() => http.get_json("http://127.0.0.1:PORT/json").get("n");
        async func download(){
            http.download("http://127.0.0.1:PORT/file", "DIR/saved.txt");
            ret file.read("DIR/saved.txt");
        }
        func m{
            out(await async.all(arr(get(), post(), post_json(), request(), get_json(), download())));
        }"#, port).replace("DIR", &shown);
    let elapsed = timed(&source, "[\"late\", \"POST hi\", \"POST {\\\"a\\\":1}\", \"PUT put body\", 7, \"the file\"]\n");
    assert!(elapsed < Duration::from_millis(800), "took {elapsed:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_failing_request_raises_in_its_own_task_only() {
    let port = server();
    let dead = closed_port();
    let source = with_port(r#"
        takepkg std.http;
        takepkg std.async;
        async func good() => http.get("http://127.0.0.1:PORT/delay").text;
        async func bad() => http.get("http://127.0.0.1:DEAD/").text;
        async func badurl() => http.get("not a url").text;
        func m{
            for (outcome, await async.settled(arr(good(), bad(), badurl()))){
                out(outcome[0]);
            }
            run { await bad(); } handle(http.HttpError e){ out("refused:", e.message.contains("refused")); }
            run { await badurl(); } handle(http.UrlError e){ out("url error"); }
        }"#, port).replace("DEAD", &dead.to_string());
    executes(&source, "true\nfalse\nfalse\nrefused: true\nurl error\n");
}

#[test]
fn requests_outside_tasks_still_block_and_work() {
    let port = server();
    executes(&with_port(r#"
        takepkg std.http;
        func m{
            out(http.get("http://127.0.0.1:PORT/json").json().get("n"));
            out(http.post("http://127.0.0.1:PORT/echo", "x").text);
        }"#, port), "7\nPOST x\n");
}

#[test]
fn other_tasks_run_while_a_request_waits() {
    let port = server();
    executes(&with_port(r#"
        takepkg std.http;
        takepkg std.async;
        async func slow() => http.get("http://127.0.0.1:PORT/delay").text;
        async func ticks(){
            n = 0;
            for (i = 0, i < 5, i++){ await async.sleep(0.02); n += 1; }
            ret n;
        }
        func m{
            t = slow();
            out(await ticks());
            out(t.done());
            out(await t);
        }"#, port), "5\nfalse\nlate\n");
}

#[test]
fn a_cancelled_request_raises_at_once() {
    let port = server();
    let elapsed = timed(&with_port(r#"
        takepkg std.http;
        takepkg std.async;
        async func slow() => http.get("http://127.0.0.1:PORT/trickle").text;
        func m{
            t = slow();
            await async.sleep(0.1);
            out(t.cancel());
            run { await t; } handle(CancelledError e){ out("cancelled"); }
        }"#, port), "true\ncancelled\n");
    assert!(elapsed < Duration::from_millis(1500), "took {elapsed:?}");
}

#[test]
fn a_cancelled_download_leaves_no_file_behind() {
    let port = server();
    let (dir, shown) = temp_dir("cancel");
    executes(&with_port(r#"
        takepkg std.http;
        takepkg std.async;
        async func get() => http.download("http://127.0.0.1:PORT/trickle", "DIR/data.bin");
        func m{
            t = get();
            await async.sleep(0.4);
            out(t.cancel());
            run { await t; } handle(CancelledError e){ out("cancelled"); }
            // The worker notices at the next piece of data and cleans up.
            await async.sleep(0.5);
        }"#, port).replace("DIR", &shown), "true\ncancelled\n");
    // The download would have taken 3 seconds: it was stopped, not finished.
    let names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert!(names.is_empty(), "left behind: {names:?}");
    let _ = std::fs::remove_dir_all(dir);
}

// ----- time.sleep -----

#[test]
fn time_sleep_in_tasks_suspends_only_the_task() {
    let elapsed = timed(r#"
        takepkg std.time;
        takepkg std.async;
        async func nap(int n){
            time.sleep(0.3);
            ret n;
        }
        func m{
            out(await async.all(arr(nap(1), nap(2), nap(3), nap(4), nap(5))));
        }"#, "[1, 2, 3, 4, 5]\n");
    assert!(elapsed >= Duration::from_millis(300), "took {elapsed:?}");
    assert!(elapsed < Duration::from_millis(700), "took {elapsed:?}");
}

#[test]
fn time_sleep_lets_other_tasks_take_turns_in_order() {
    executes(r#"
        takepkg std.time;
        takepkg std.async;
        async func nap(string name, float seconds){
            time.sleep(seconds);
            out(name);
        }
        func m{
            await async.all(arr(nap("slow", 0.15), nap("fast", 0.03), nap("middle", 0.08)));
            // Durations and other numeric kinds work as before.
            await async.all(arr(nap("a", 0.01)));
            t = async.spawn(func() => time.sleep(time.milliseconds(20)));
            await t;
            out("done");
        }"#, "fast\nmiddle\nslow\na\ndone\n");
}

#[test]
fn time_sleep_outside_tasks_still_pauses_the_program() {
    let elapsed = timed(r#"
        takepkg std.time;
        func m{
            start = time.monotonic();
            time.sleep(0.2);
            time.sleep(time.milliseconds(100));
            out(time.monotonic() - start >= 0.3);
        }"#, "true\n");
    assert!(elapsed >= Duration::from_millis(300), "took {elapsed:?}");
}

#[test]
fn a_task_sleeping_with_time_sleep_can_be_cancelled() {
    let elapsed = timed(r#"
        takepkg std.time;
        takepkg std.async;
        async func long(){
            run { time.sleep(60); } then { out("cleanup"); }
        }
        func m{
            t = long();
            await async.sleep(0.05);
            out(t.cancel());
            run { await t; } handle(CancelledError e){ out("cancelled"); }
        }"#, "true\ncleanup\ncancelled\n");
    assert!(elapsed < Duration::from_secs(5), "took {elapsed:?}");
}

#[test]
fn time_sleep_still_rejects_bad_lengths_in_tasks() {
    executes(r#"
        takepkg std.time;
        takepkg std.async;
        async func bad() => time.sleep(-1);
        func m{
            run { await bad(); } handle(RangeError e){ out("RangeError"); }
        }"#, "RangeError\n");
}
