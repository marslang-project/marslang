//! std.http against a small HTTP server on 127.0.0.1, started per test in a
//! thread, so nothing here needs the internet (except the one #[ignore] test).
//! 127.0.0.1 never goes through a proxy, so these pass whatever HTTP_PROXY and
//! friends are set to; the proxy tests run the interpreter in a child process
//! with its own environment.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

fn executes(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("Marslang compilation failed");
    let (stdout, result) = marslang::run_captured(compiled, "");
    if let Err(error) = result { panic!("runtime error: {error}\noutput so far:\n{stdout}"); }
    assert_eq!(stdout, expected);
}

fn runtime_error(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("compile failed");
    let (_, result) = marslang::run_captured(compiled, "");
    let error = result.expect_err(&format!("expected runtime error: {source}")).to_string();
    assert!(error.contains(expected), "expected {expected}, got {error}");
}

// ----- the server -----

struct Request { method: String, target: String, headers: Vec<(String, String)>, body: Vec<u8> }

impl Request {
    fn header(&self, name: &str) -> &str {
        self.headers.iter().find(|(n, _)| n == name).map_or("-", |(_, v)| v.as_str())
    }
}

fn read_request(reader: &mut BufReader<TcpStream>) -> Option<Request> {
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 { return None; }
    let mut parts = line.trim_end().splitn(3, ' ');
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() { break; }
        let (name, value) = header.split_once(':')?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }
    let length = headers.iter().find(|(n, _)| n == "content-length").map_or(0, |(_, v)| v.parse().unwrap());
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Request { method, target, headers, body })
}

fn reply(stream: &mut TcpStream, status: &str, headers: &[(&str, &str)], body: &[u8]) {
    let mut head = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
    for (name, value) in headers { head.push_str(&format!("{name}: {value}\r\n")); }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

/// The body /file serves, for downloads: about 200 KB.
fn file_body() -> String { (0..20000).map(|i| format!("line {i}\n")).collect() }

fn handle(stream: TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut stream = stream;
    let Some(mut request) = read_request(&mut reader) else { return };
    let mut via_proxy = false;
    if request.method == "CONNECT" {
        // Acting as a proxy: the tunnel's request follows on the same connection.
        let _ = stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n");
        let Some(tunneled) = read_request(&mut reader) else { return };
        request = tunneled;
        via_proxy = true;
    }
    let path = match request.target.strip_prefix("http://") {
        Some(rest) => { via_proxy = true; rest.find('/').map_or("/".to_string(), |i| rest[i..].to_string()) }
        None => request.target.clone(),
    };
    let proxied = if via_proxy { "yes" } else { "no" };
    match path.as_str() {
        "/text" => reply(&mut stream, "200 OK", &[("Content-Type", "text/plain"), ("X-Custom", "Yes"), ("X-Via-Proxy", proxied)], b"hello, world"),
        "/json" => reply(&mut stream, "200 OK", &[("Content-Type", "application/json")], br#"{"name": "Ada", "tags": ["math", "code"]}"#),
        "/missing" => reply(&mut stream, "404 Not Found", &[], b"no such page"),
        "/error" => reply(&mut stream, "500 Internal Server Error", &[("Content-Type", "application/json")], br#"{"error": "boom"}"#),
        "/notjson" => reply(&mut stream, "200 OK", &[], b"<html>not json</html>"),
        "/created" => reply(&mut stream, "201 Created", &[], b""),
        "/cookies" => reply(&mut stream, "200 OK", &[("Set-Cookie", "a=1"), ("Set-Cookie", "b=2")], b""),
        "/latin1" => reply(&mut stream, "200 OK", &[], b"caf\xe9!"),
        "/echo" => {
            let echoed = format!(
                "{} {}\ncontent-type: {}\nx-token: {}\nuser-agent: {}\n{}",
                request.method, path, request.header("content-type"), request.header("x-token"),
                request.header("user-agent"), String::from_utf8_lossy(&request.body)
            );
            reply(&mut stream, "200 OK", &[], echoed.as_bytes());
        }
        "/redirect" => reply(&mut stream, "302 Found", &[("Location", "/redirect2")], b""),
        "/redirect2" => reply(&mut stream, "301 Moved Permanently", &[("Location", "/text")], b""),
        "/loop" => reply(&mut stream, "302 Found", &[("Location", "/loop")], b""),
        "/to-echo" => reply(&mut stream, "302 Found", &[("Location", "/echo")], b""),
        "/to-echo-307" => reply(&mut stream, "307 Temporary Redirect", &[("Location", "/echo")], b""),
        "/to-auth" => reply(&mut stream, "302 Found", &[("Location", "/auth")], b""),
        "/to-other-host" => {
            let port = request.header("host").rsplit(':').next().unwrap().to_string();
            reply(&mut stream, "302 Found", &[("Location", &format!("http://localhost:{port}/auth"))], b"");
        }
        "/auth" => reply(&mut stream, "200 OK", &[], request.header("authorization").as_bytes()),
        "/file" => reply(&mut stream, "200 OK", &[("Content-Type", "text/plain")], file_body().as_bytes()),
        "/slow" => thread::sleep(Duration::from_secs(4)),
        "/big" => {
            // Declares more than 100 MB and sends nothing: refused at once.
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 104857601\r\nConnection: close\r\n\r\n");
            thread::sleep(Duration::from_secs(2));
        }
        "/bigchunked" => {
            // More than 100 MB with no length given: refused while reading.
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n");
            let chunk = vec![b'x'; 1 << 20];
            for _ in 0..101 {
                if stream.write_all(b"100000\r\n").and_then(|_| stream.write_all(&chunk)).and_then(|_| stream.write_all(b"\r\n")).is_err() { return; }
            }
            let _ = stream.write_all(b"0\r\n\r\n");
        }
        "/truncated" => {
            // Promises 100000 bytes and hangs up after 10.
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100000\r\nConnection: close\r\n\r\n0123456789");
        }
        _ => reply(&mut stream, "404 Not Found", &[], b"unknown path"),
    }
}

/// Start a server; its port.
fn server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() { thread::spawn(move || handle(stream)); }
    });
    port
}

/// A port with nothing listening on it.
fn closed_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn with_port(source: &str, port: u16) -> String { source.replace("PORT", &port.to_string()) }

/// A new empty directory, with / separators so it can go in a Marslang string.
fn temp_dir(name: &str) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("marslang-http-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let shown = dir.to_string_lossy().replace('\\', "/");
    (dir, shown)
}

fn names_in(dir: &PathBuf) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

// ----- tests -----

#[test]
fn get_returns_status_text_headers_and_url() {
    let port = server();
    executes(&with_port(r#"
        takepkg std.http;
        func m{
            r = http.get("http://127.0.0.1:PORT/text");
            out(r.status, r.ok, r.text);
            out(r.header("content-type"), r.header("X-CUSTOM"), r.header("x-missing"));
            out(r.headers.get("x-custom"), r.headers.has("X-Custom"), r.url);
            c = http.get("http://127.0.0.1:PORT/created");
            out(c.status, c.ok, c.text.len());
            out(http.get("http://127.0.0.1:PORT/cookies").header("set-cookie"));
        }"#, port),
        &with_port("200 true hello, world\ntext/plain Yes null\nYes false http://127.0.0.1:PORT/text\n201 true 0\na=1, b=2\n", port));
}

#[test]
fn error_statuses_are_responses_not_errors() {
    let port = server();
    executes(&with_port(r#"
        takepkg std.http;
        func m{
            r = http.get("http://127.0.0.1:PORT/missing");
            out(r.status, r.ok, r.text);
            e = http.get("http://127.0.0.1:PORT/error");
            out(e.status, e.ok, e.json().get("error"));
        }"#, port),
        "404 false no such page\n500 false boom\n");
}

#[test]
fn bodies_are_text_and_json() {
    let port = server();
    executes(&with_port(r#"
        takepkg std.http;
        func m{
            data = http.get("http://127.0.0.1:PORT/json").json();
            out(data.get("name"), data.get("tags")[1]);
            out(http.get("http://127.0.0.1:PORT/latin1").text == "caf�!");
        }"#, port),
        "Ada code\ntrue\n");
    runtime_error(&with_port(r#"
        takepkg std.http;
        func m{ http.get("http://127.0.0.1:PORT/notjson").json(); }"#, port),
        &with_port("http://127.0.0.1:PORT/notjson: JSON line 1, column 1", port));
}

#[test]
fn post_and_post_json_send_their_bodies() {
    let port = server();
    let version = marslang::VERSION;
    executes(&with_port(r#"
        takepkg std.http;
        func m{
            out(http.post("http://127.0.0.1:PORT/echo", "héllo, wörld").text);
            out(http.post_json("http://127.0.0.1:PORT/echo", map().set("name", "Ada").set("n", arr(1, 2))).text);
        }"#, port),
        &format!("POST /echo\ncontent-type: text/plain; charset=utf-8\nx-token: -\nuser-agent: marslang/{version}\nhéllo, wörld\n\
                  POST /echo\ncontent-type: application/json\nx-token: -\nuser-agent: marslang/{version}\n{{\"name\":\"Ada\",\"n\":[1,2]}}\n"));
}

#[test]
fn request_sends_any_method_and_headers() {
    let port = server();
    executes(&with_port(r#"
        takepkg std.http;
        func m{
            headers = map().set("X-Token", "secret").set("Content-Type", "text/csv").set("User-Agent", "tester/1");
            r = http.request("PUT", "http://127.0.0.1:PORT/echo", headers, "a,b\n1,2");
            out(r.status, r.text);
            out(http.request("delete", "http://127.0.0.1:PORT/echo", map(), null).text.lines()[0]);
            out(http.request("PROPFIND", "http://127.0.0.1:PORT/echo", map(), null).text.lines()[0]);
            out(http.request("GET", "http://127.0.0.1:PORT/echo", map(), "query").text.lines()[4]);
            h = http.request("HEAD", "http://127.0.0.1:PORT/text", map(), null);
            out(h.status, h.text == "", h.header("x-custom"));
        }"#, port),
        "200 PUT /echo\ncontent-type: text/csv\nx-token: secret\nuser-agent: tester/1\na,b\n1,2\nDELETE /echo\nPROPFIND /echo\n\
         query\n200 true Yes\n");
    runtime_error(&with_port(r#"takepkg std.http; func m{ http.request("GET", "http://127.0.0.1:PORT/", map().set("bad name", "x"), null); }"#, port),
        "SyntaxError: \"bad name\" is not a valid header name");
    runtime_error(&with_port(r#"takepkg std.http; func m{ http.request("GET", "http://127.0.0.1:PORT/", map().set("X-A", 1), null); }"#, port),
        "TypeError: http.request needs headers to map strings to strings, got string to int");
    runtime_error(&with_port(r#"takepkg std.http; func m{ http.request("GET", "http://127.0.0.1:PORT/", map(), 5); }"#, port),
        "TypeError: http.request expects the body as a string or null, got int");
    runtime_error(&with_port(r#"takepkg std.http; func m{ http.request("GE T", "http://127.0.0.1:PORT/", map(), null); }"#, port),
        "SyntaxError: \"GE T\" is not an HTTP method");
}

#[test]
fn get_json_parses_and_raises_status_errors() {
    let port = server();
    executes(&with_port(r#"
        takepkg std.http;
        func m{
            out(http.get_json("http://127.0.0.1:PORT/json").get("tags"));
            run{
                http.get_json("http://127.0.0.1:PORT/error");
            } handle(http.StatusError e){
                out(e.status, e.response.json().get("error"));
                out(e.message);
            }
            run{
                http.get_json("http://127.0.0.1:PORT/missing");
            } handle(http.HttpError e){
                out("an HttpError too:", e.status, e.response.text);
            }
        }"#, port),
        &with_port("[\"math\", \"code\"]\n500 boom\nGET http://127.0.0.1:PORT/error: the server answered 500 Internal Server Error\nan HttpError too: 404 no such page\n", port));
    runtime_error(&with_port(r#"takepkg std.http; func m{ http.get_json("http://127.0.0.1:PORT/missing"); }"#, port),
        &with_port("StatusError: GET http://127.0.0.1:PORT/missing: the server answered 404 Not Found", port));
    runtime_error(&with_port(r#"takepkg std.http; func m{ http.get_json("http://127.0.0.1:PORT/notjson"); }"#, port),
        &with_port("SyntaxError: http://127.0.0.1:PORT/notjson: JSON line 1, column 1", port));
}

#[test]
fn redirects_are_followed_and_url_is_the_final_one() {
    let port = server();
    executes(&with_port(r#"
        takepkg std.http;
        func m{
            r = http.get("http://127.0.0.1:PORT/redirect");
            out(r.status, r.text, r.url);
        }"#, port),
        &with_port("200 hello, world http://127.0.0.1:PORT/text\n", port));
    runtime_error(&with_port(r#"takepkg std.http; func m{ http.get("http://127.0.0.1:PORT/loop"); }"#, port),
        &with_port("HttpError: GET http://127.0.0.1:PORT/loop: more than 10 redirects", port));
    // 302 turns a POST into a GET, as browsers and curl do; 307 would send
    // the body again, and is refused.
    executes(&with_port(r#"
        takepkg std.http;
        func m{
            out(http.post("http://127.0.0.1:PORT/to-echo", "body").text.lines()[0]);
            auth = map().set("Authorization", "Bearer secret");
            out(http.request("GET", "http://127.0.0.1:PORT/to-auth", auth, null).text);
            out(http.request("GET", "http://127.0.0.1:PORT/to-other-host", auth, null).text);
        }"#, port),
        "GET /echo\nBearer secret\n-\n");
    runtime_error(&with_port(r#"takepkg std.http; func m{ http.post("http://127.0.0.1:PORT/to-echo-307", "body"); }"#, port),
        &with_port("HttpError: POST http://127.0.0.1:PORT/to-echo-307: the server answered 307 or 308", port));
}

#[test]
fn download_streams_to_a_file_all_or_nothing() {
    let port = server();
    let (dir, shown) = temp_dir("download");
    let source = with_port(r#"
        takepkg std.http;
        takepkg std.file;
        func m{
            path = "DIR/data.txt";
            r = http.download("http://127.0.0.1:PORT/file", path);
            out(r.status, r.ok, r.text.len(), r.header("content-type"));
            text = file.read(path);
            out(text.len(), text.lines()[0], text.lines()[19999]);
            // A failure keeps the file that was there, and leaves no other.
            run{ http.download("http://127.0.0.1:PORT/truncated", path); } handle(http.HttpError e){ out("truncated:", e.message); }
            run{ http.download("http://127.0.0.1:PORT/missing", path); } handle(http.StatusError e){ out("status:", e.status); }
            run{ http.download("http://127.0.0.1:PORT/missing", "DIR/new.txt"); } handle(http.StatusError e){ out("status:", e.status); }
            out(file.read(path) == text, file.exists("DIR/new.txt"));
            run{ http.download("http://127.0.0.1:PORT/file", "DIR/no/such/dir.txt"); } handle(file.NotFoundError e){ out("not found:", e.message); }
            run{ http.download("http://127.0.0.1:PORT/file", "DIR"); } handle(file.IsDirectoryError e){ out("directory"); }
        }"#, port).replace("DIR", &shown);
    let length = file_body().len();
    executes(&source, &format!(
        "200 true 0 text/plain\n{length} line 0 line 19999\n\
         truncated: GET http://127.0.0.1:{port}/truncated: the server closed the connection before it finished answering\n\
         status: 404\nstatus: 404\ntrue false\n\
         not found: {shown}/no/such/dir.txt: the directory {} does not exist; file.make_dir creates it\ndirectory\n",
        PathBuf::from(format!("{shown}/no/such")).display()));
    assert_eq!(names_in(&dir), ["data.txt"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn bodies_over_100_mb_are_refused() {
    let port = server();
    runtime_error(&with_port(r#"takepkg std.http; func m{ http.get("http://127.0.0.1:PORT/big"); }"#, port),
        &with_port("HttpError: GET http://127.0.0.1:PORT/big: the response body is larger than 100 MB; http.download saves a body of any size to a file", port));
    runtime_error(&with_port(r#"takepkg std.http; func m{ http.get("http://127.0.0.1:PORT/bigchunked"); }"#, port),
        "the response body is larger than 100 MB");
}

#[test]
fn a_server_that_never_answers_times_out() {
    let port = server();
    let start = Instant::now();
    runtime_error(&with_port(r#"
        takepkg std.http;
        func m{ http.set_timeout(1); http.get("http://127.0.0.1:PORT/slow"); }"#, port),
        &with_port("HttpError: GET http://127.0.0.1:PORT/slow: no complete answer within 1 second (http.set_timeout changes the limit)", port));
    assert!(start.elapsed() < Duration::from_secs(3), "took {:?}", start.elapsed());
    executes(&with_port(r#"
        takepkg std.http;
        func m{
            http.set_timeout(0.5);
            run{ http.get("http://127.0.0.1:PORT/slow"); } handle(http.HttpError e){ out(e.message.contains("within 0.5 seconds")); }
            http.set_timeout(10);
            out(http.get("http://127.0.0.1:PORT/text").text);
        }"#, port), "true\nhello, world\n");
    runtime_error("takepkg std.http; func m{ http.set_timeout(0); }", "RangeError: http.set_timeout needs a positive number of seconds, got 0");
    runtime_error("takepkg std.http; func m{ http.set_timeout(-2.5); }", "RangeError: http.set_timeout needs a positive number of seconds, got -2.5");
}

#[test]
fn connection_refused_names_the_url() {
    // A closed port can be taken by another test's server in between, so a
    // few ports are tried.
    let mut last = String::new();
    for _ in 0..3 {
        let port = closed_port();
        let compiled = marslang::compile(&with_port(r#"
            takepkg std.http;
            func m{
                run{ http.post("http://localhost:PORT/x", "a"); } handle(http.HttpError e){ out(e.message.starts_with("POST http://localhost:PORT/x: ")); }
                http.get("http://127.0.0.1:PORT/x");
            }"#, port)).unwrap();
        let (stdout, result) = marslang::run_captured(compiled, "");
        last = format!("{stdout}{result:?}");
        let expected = with_port("HttpError: GET http://127.0.0.1:PORT/x: the connection was refused: nothing is listening at that address and port", port);
        if stdout == "true\n" && result.is_err_and(|e| e.to_string().contains(&expected)) { return; }
    }
    panic!("no refused connection: {last}");
}

#[test]
fn urls_that_are_not_http_raise_url_errors() {
    runtime_error(r#"takepkg std.http; func m{ http.get("example.com"); }"#,
        "UrlError: \"example.com\" is not a URL: it must start with http:// or https://");
    runtime_error(r#"takepkg std.http; func m{ http.get("ftp://example.com/file"); }"#,
        "UrlError: ftp://example.com/file: only http:// and https:// URLs are supported, not ftp://");
    runtime_error(r#"takepkg std.http; func m{ http.get("http://example.com/a b"); }"#,
        "UrlError: \"http://example.com/a b\" is not a valid URL");
    runtime_error(r#"takepkg std.http; func m{ http.get("http://"); }"#, "UrlError: \"http://\" is not a valid URL");
    executes(r#"
        takepkg std.http;
        func m{
            run{ http.download("file:///etc/passwd", "x"); } handle(http.HttpError e){ out("an HttpError:", e.message); }
        }"#, "an HttpError: file:///etc/passwd: only http:// and https:// URLs are supported, not file://\n");
}

// ----- proxies, in a child process with its own environment -----

const PROXY_VARIABLES: [&str; 8] = ["http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY", "no_proxy", "NO_PROXY"];

/// Run a program in the marslang executable with only these proxy settings; its output.
fn run_with_env(name: &str, source: &str, vars: &[(&str, String)]) -> String {
    let (dir, _) = temp_dir(name);
    let program = dir.join("main.mars");
    std::fs::write(&program, source).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_marslang"));
    command.arg(&program);
    for var in PROXY_VARIABLES { command.env_remove(var); }
    for (var, value) in vars { command.env(var, value); }
    let output = command.output().expect("marslang runs");
    std::fs::remove_dir_all(&dir).unwrap();
    String::from_utf8_lossy(&output.stdout).into_owned() + &String::from_utf8_lossy(&output.stderr)
}

#[test]
fn proxies_come_from_the_environment() {
    let port = server();
    let proxy = format!("http://127.0.0.1:{port}");
    // A host only the proxy can reach: the test server, acting as the proxy.
    let through = r#"
        takepkg std.http;
        func m{
            run{
                r = http.get("http://example.invalid/text");
                out(r.text, "via proxy:", r.header("x-via-proxy"));
            } handle(http.HttpError e){ out("direct:", e.message); }
        }"#;
    assert_eq!(run_with_env("proxy-http", through, &[("HTTP_PROXY", proxy.clone())]), "hello, world via proxy: yes\n");
    assert_eq!(run_with_env("proxy-all", through, &[("ALL_PROXY", proxy.clone())]), "hello, world via proxy: yes\n");
    // HTTPS_PROXY is for https:// only.
    assert!(run_with_env("proxy-https", through, &[("HTTPS_PROXY", proxy.clone())]).starts_with("direct:"));
    // NO_PROXY's patterns are checked by the unit tests in std/rs/http.rs.
    let output = run_with_env("proxy-no", through, &[("HTTP_PROXY", proxy.clone()), ("NO_PROXY", "other.com, .invalid".into())]);
    assert!(output.starts_with("direct:"), "{output}");
    let output = run_with_env("proxy-no-other", through, &[("HTTP_PROXY", proxy.clone()), ("NO_PROXY", "example.com,10.0.0.0/8".into())]);
    assert_eq!(output, "hello, world via proxy: yes\n");

    // This machine never goes through a proxy, even one that is down.
    let dead = format!("http://127.0.0.1:{}", closed_port());
    let local = with_port(r#"
        takepkg std.http;
        func m{
            out(http.get("http://127.0.0.1:PORT/text").header("x-via-proxy"));
            out(http.get("http://localhost:PORT/text").header("x-via-proxy"));
        }"#, port);
    let vars = [("HTTP_PROXY", dead.clone()), ("HTTPS_PROXY", dead.clone()), ("ALL_PROXY", dead.clone())];
    assert_eq!(run_with_env("proxy-local", &local, &vars), "no\nno\n");

    // A proxy that is down is named in the error. As in
    // connection_refused_names_the_url, a few closed ports are tried.
    let named = (0..3).any(|_| {
        let dead_port = closed_port();
        let output = run_with_env("proxy-dead", through, &[("HTTP_PROXY", format!("http://127.0.0.1:{dead_port}"))]);
        output.starts_with("direct: GET http://example.invalid/text: the connection was refused")
            && output.contains(&format!(", through the proxy 127.0.0.1:{dead_port} (from "))
    });
    assert!(named, "a proxy that is down is not named");
    let output = run_with_env("proxy-bad", through, &[("HTTP_PROXY", "http://bad proxy".into())]);
    assert!(output.contains("is not a valid proxy URL"), "{output}");
}

/// Proves TLS and the built-in certificate roots work. Needs the internet
/// (and honours HTTPS_PROXY): cargo test --test std_http -- --ignored
#[test]
#[ignore]
fn fetches_example_com_over_https() {
    executes(r#"
        takepkg std.http;
        func m{
            r = http.get("https://example.com/");
            out(r.ok, r.text.contains("Example Domain"), r.url);
        }"#, "true true https://example.com/\n");
}
