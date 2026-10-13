//! `takepkg rs.http;` - the network side of `std.http`: sending a request and
//! reading the answer, by the ureq crate over rustls.
//!
//! Rust, because Marslang has no sockets: this file connects (through a proxy
//! from the environment when one applies), speaks HTTP/1.1 and TLS, follows
//! redirects, enforces the timeout and the body limit, and streams downloads
//! to a file. Everything else, the `Response` family, the error families, JSON,
//! and when a status is an error, is Marslang, in `std/http.mars`.
//!
//! A request returns `[true, [status, text, headers, url]]` or
//! `[false, kind, message]`, and `std/http.mars` raises the family `kind`
//! names (`Http` is `http.HttpError`, `Url` is `http.UrlError`, and `NotFound`,
//! `Permission`, `IsDirectory`, and `File` are the `std.file` families, for a
//! download's file), because a native package can raise only built-in errors.
//!
//! TLS certificates are checked against Mozilla's roots, built in by the
//! webpki-roots crate, so a static binary needs no system CA files; the
//! crypto is ring's, which builds without cmake or nasm.

use std::cell::Cell;
use std::fs;
use std::io::{self, Read, Write};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use indexmap::IndexMap;
use ureq::http::{self, HeaderName, HeaderValue, Method, Uri};
use ureq::{Agent, AsSendBody, Proxy, ResponseExt};

use crate::value::*;

/// Bodies larger than this are refused by everything but download.
const MAX_BODY: u64 = 100 * 1024 * 1024;
const MAX_REDIRECTS: u32 = 10;

thread_local! {
    /// One agent per program (each runs on its own thread), so connections
    /// to the same server are reused between requests.
    static AGENT: Agent = Agent::new_with_config(
        Agent::config_builder()
            // A 404 or 500 is an answer, not a failure: std.http decides.
            .http_status_as_error(false)
            .max_redirects(MAX_REDIRECTS)
            // Authorization goes along on a redirect within the same host,
            // never to another host or from https:// to http://.
            .redirect_auth_headers(ureq::config::RedirectAuthHeaders::SameHost)
            .user_agent(format!("marslang/{}", crate::VERSION))
            // Chosen per request from the environment, in proxy_for.
            .proxy(None)
            .allow_non_standard_methods(true)
            .build(),
    );
    static TIMEOUT: Cell<Duration> = const { Cell::new(Duration::from_secs(30)) };
}

// ----- results -----

fn ok(value: Value) -> RResult<Value> { Ok(Value::array(vec![Value::Bool(true), value])) }

fn fail(kind: &str, message: impl Into<String>) -> RResult<Value> {
    Ok(Value::array(vec![Value::Bool(false), Value::str(kind), Value::str(&message.into())]))
}

fn text<'a>(value: &'a Value, what: &str) -> RResult<&'a str> {
    match value {
        Value::Str(text) => Ok(text),
        other => type_err(format!("{what} expects a string, got {}", other.type_name())),
    }
}

// ----- URLs and proxies -----

/// The URL, checked: http:// or https:// with a host. The message says what is wrong.
fn parse_url(url: &str) -> Result<Uri, String> {
    let Some((scheme, _)) = url.split_once("://") else {
        return Err(format!("\"{url}\" is not a URL: it must start with http:// or https://"));
    };
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return Err(format!("{url}: only http:// and https:// URLs are supported, not {scheme}://"));
    }
    let uri: Uri = url.parse().map_err(|e: http::uri::InvalidUri| {
        let hint = if url.contains(' ') { "; write a space as %20" } else if !url.is_ascii() {
            "; write other characters percent-encoded"
        } else { "" };
        format!("\"{url}\" is not a valid URL: {e}{hint}")
    })?;
    if uri.host().is_none_or(str::is_empty) {
        return Err(format!("\"{url}\" is not a valid URL: it has no host"));
    }
    Ok(uri)
}

/// The host without the brackets of an IPv6 address, in lower case.
fn bare_host(uri: &Uri) -> String {
    uri.host().unwrap_or("").trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase()
}

/// This machine: never reached through a proxy.
fn is_local(host: &str) -> bool {
    host == "localhost" || host.ends_with(".localhost") || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// The first of the environment variables that is set and not empty, with its name.
fn env(names: &[&'static str]) -> Option<(&'static str, String)> {
    names.iter().find_map(|&name| std::env::var(name).ok().filter(|v| !v.trim().is_empty()).map(|v| (name, v)))
}

/// Whether NO_PROXY (or no_proxy) says to reach the host directly. As in
/// curl: entries are separated by commas; `*` matches every host;
/// `example.com`, `.example.com`, and `*.example.com` match example.com and
/// every name under it; an IP address matches itself, and `10.0.0.0/8` the
/// addresses in that range. A port on an entry is ignored.
fn no_proxy(host: &str) -> bool {
    env(&["no_proxy", "NO_PROXY"]).is_some_and(|(_, list)| no_proxy_matches(&list, host))
}

fn no_proxy_matches(list: &str, host: &str) -> bool {
    let ip = host.parse::<IpAddr>().ok();
    list.split(',').map(|entry| entry.trim().to_ascii_lowercase()).filter(|e| !e.is_empty()).any(|entry| {
        if entry == "*" { return true; }
        if let (Some(ip), Some((net, bits))) = (ip, entry.split_once('/')) {
            return in_range(ip, net, bits);
        }
        let entry = entry.trim_start_matches('[');
        let entry = match entry.split_once(']') {
            Some((v6, _)) => v6,
            None if entry.matches(':').count() == 1 => entry.split(':').next().unwrap_or(entry),
            None => entry,
        };
        let entry = entry.trim_start_matches("*.").trim_start_matches('.');
        host == entry || host.strip_suffix(entry).is_some_and(|rest| rest.ends_with('.'))
    })
}

fn in_range(ip: IpAddr, net: &str, bits: &str) -> bool {
    let (Ok(net), Ok(bits)) = (net.parse::<IpAddr>(), bits.parse::<u32>()) else { return false };
    match (ip, net) {
        (IpAddr::V4(a), IpAddr::V4(b)) if bits <= 32 => {
            let mask = if bits == 0 { 0 } else { u32::MAX << (32 - bits) };
            u32::from(a) & mask == u32::from(b) & mask
        }
        (IpAddr::V6(a), IpAddr::V6(b)) if bits <= 128 => {
            let mask = if bits == 0 { 0 } else { u128::MAX << (128 - bits) };
            u128::from(a) & mask == u128::from(b) & mask
        }
        _ => false,
    }
}

/// The proxy for a URL, from the environment, and how to name it in messages:
/// HTTPS_PROXY for https:// and HTTP_PROXY for http://, either falling back to
/// ALL_PROXY, in lower or upper case; none for this machine or a host NO_PROXY
/// names.
fn proxy_for(uri: &Uri) -> Result<Option<(Proxy, String)>, String> {
    let host = bare_host(uri);
    if is_local(&host) || no_proxy(&host) { return Ok(None); }
    let names: &[&'static str] = if uri.scheme_str().is_some_and(|s| s.eq_ignore_ascii_case("https")) {
        &["https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY"]
    } else {
        &["http_proxy", "HTTP_PROXY", "all_proxy", "ALL_PROXY"]
    };
    let Some((name, value)) = env(names) else { return Ok(None) };
    // Windows ignores the case of variable names, and they are usually upper case there.
    let name = if cfg!(windows) { name.to_ascii_uppercase() } else { name.to_string() };
    // The value is not shown: it may hold a password.
    let proxy = Proxy::new(value.trim()).map_err(|_| format!("{name} is not a valid proxy URL"))?;
    let shown = format!("the proxy {}:{} (from {name})", proxy.host(), proxy.port());
    Ok(Some((proxy, shown)))
}

// ----- errors -----

fn seconds(duration: Duration) -> String {
    let s = duration.as_secs_f64();
    if s == 1.0 { "1 second".into() } else { format!("{s} seconds") }
}

/// A failure, in plain words.
fn reason(error: &ureq::Error, timeout: Duration) -> String {
    use io::ErrorKind::*;
    use ureq::Error as E;
    match error {
        E::Timeout(_) => format!("no complete answer within {} (http.set_timeout changes the limit)", seconds(timeout)),
        E::HostNotFound => "the host was not found (DNS lookup failed)".into(),
        E::Io(e) => match e.kind() {
            ConnectionRefused => "the connection was refused: nothing is listening at that address and port".into(),
            ConnectionReset | ConnectionAborted => "the connection was closed by the other side".into(),
            UnexpectedEof => "the server closed the connection before it finished answering".into(),
            TimedOut | WouldBlock => format!("no complete answer within {} (http.set_timeout changes the limit)", seconds(timeout)),
            AddrNotAvailable | NetworkUnreachable | HostUnreachable => format!("the host cannot be reached: {e}"),
            // rustls reports a failed handshake as data that is not valid.
            InvalidData => tls(&e.to_string()),
            _ => format!("{e}"),
        },
        E::Tls(e) => tls(e),
        E::Rustls(e) => tls(&e.to_string()),
        E::TooManyRedirects => format!("more than {MAX_REDIRECTS} redirects"),
        E::RedirectFailed => "the server answered 307 or 308, asking for the request to be sent again elsewhere, which std.http does not do for a POST, PUT, PATCH, or DELETE".into(),
        E::BodyExceedsLimit(_) => body_too_large(),
        E::Protocol(e) => format!("the answer is not valid HTTP: {e}"),
        E::ConnectionFailed => "could not connect".into(),
        E::BadUri(e) => format!("the URL is not valid: {e}"),
        other => other.to_string(),
    }
}

/// A TLS failure, with the usual certificate problems in plain words.
fn tls(detail: &str) -> String {
    let why = if detail.contains("UnknownIssuer") {
        "the server's certificate is not signed by an authority std.http trusts".to_string()
    } else if detail.contains("certificate expired") {
        "the server's certificate has expired".to_string()
    } else if detail.contains("not valid for name") {
        "the server's certificate is for a different host name".to_string()
    } else {
        return format!("the secure (TLS) connection failed: {detail}");
    };
    format!("the secure (TLS) connection failed: {why} ({detail})")
}

fn body_too_large() -> String {
    format!("the response body is larger than {} MB; http.download saves a body of any size to a file", MAX_BODY >> 20)
}

/// The message for a failed request: the method and URL, why, and the proxy
/// when one was used, since a proxy that is down looks like any other failure.
fn net_fail(method: &Method, url: &str, why: String, proxy: &Option<(Proxy, String)>) -> RResult<Value> {
    let through = proxy.as_ref().map(|(_, shown)| format!(", through {shown}")).unwrap_or_default();
    fail("Http", format!("{method} {url}: {why}{through}"))
}

// ----- requests -----

/// Send a request; with `save_to`, stream a 2xx body to that file instead of
/// reading it. headers is a map of strings and body a string or null.
fn request(method: &str, url: &str, headers: &Value, body: &Value, save_to: Option<&str>) -> RResult<Value> {
    let method = Method::from_bytes(method.to_ascii_uppercase().as_bytes())
        .or_else(|_| err(ErrorKind::SyntaxError, format!("\"{method}\" is not an HTTP method")))?;
    let uri = match parse_url(url) { Ok(uri) => uri, Err(message) => return fail("Url", message) };
    let mut builder = http::Request::builder().method(method.clone()).uri(uri.clone());
    match headers {
        Value::Map(map) => for (name, value) in map.items.borrow().values() {
            let (Value::Str(name), Value::Str(value)) = (name, value) else {
                return type_err(format!(
                    "http.request needs headers to map strings to strings, got {} to {}", name.type_name(), value.type_name()
                ));
            };
            let name = HeaderName::from_bytes(name.as_bytes())
                .or_else(|_| err(ErrorKind::SyntaxError, format!("\"{name}\" is not a valid header name")))?;
            let value = HeaderValue::from_str(value)
                .or_else(|_| err(ErrorKind::SyntaxError, format!("the {name} header's value has a line break or other character headers cannot hold")))?;
            builder = builder.header(name, value);
        },
        Value::Null => {}
        other => return type_err(format!("http.request expects headers as a map, got {}", other.type_name())),
    }
    let proxy = match proxy_for(&uri) { Ok(p) => p, Err(message) => return fail("Http", format!("{method} {url}: {message}")) };
    let timeout = TIMEOUT.with(Cell::get);
    let sent = match body {
        Value::Null => send(built(builder.body(()))?, &proxy, timeout, save_to.is_some()),
        Value::Str(text) => send(built(builder.body(text.as_bytes()))?, &proxy, timeout, save_to.is_some()),
        other => return type_err(format!("http.request expects the body as a string or null, got {}", other.type_name())),
    };
    let mut response = match sent {
        Ok(response) => response,
        Err(error) => return net_fail(&method, url, reason(&error, timeout), &proxy),
    };
    let status = response.status().as_u16();
    let final_url = response.get_uri().to_string();
    let headers = response_headers(response.headers());
    let body = match save_to {
        Some(path) if (200..300).contains(&status) => match save(&mut response, path) {
            Ok(()) => String::new(),
            Err(Saving::File(kind, message)) => return fail(kind, message),
            Err(Saving::Net(error)) => return net_fail(&method, url, reason(&error, timeout), &proxy),
        },
        Some(_) => String::new(),
        None => {
            if response.body().content_length().is_some_and(|n| n > MAX_BODY) {
                return net_fail(&method, url, body_too_large(), &proxy);
            }
            match response.body_mut().with_config().limit(MAX_BODY).read_to_vec() {
                Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                Err(error) => return net_fail(&method, url, reason(&error, timeout), &proxy),
            }
        }
    };
    ok(Value::array(vec![Value::Int(status as i64), Value::str(&body), headers, Value::str(&final_url)]))
}

/// The request, or SyntaxError for a part the http crate refused, though
/// every part was checked above.
fn built<S>(request: Result<http::Request<S>, http::Error>) -> RResult<http::Request<S>> {
    request.or_else(|e| err(ErrorKind::SyntaxError, format!("the request is not valid: {e}")))
}

fn send<S: AsSendBody>(
    request: http::Request<S>, proxy: &Option<(Proxy, String)>, timeout: Duration, download: bool,
) -> Result<http::Response<ureq::Body>, ureq::Error> {
    AGENT.with(|agent| {
        let config = agent.configure_request(request).proxy(proxy.as_ref().map(|(p, _)| p.clone()));
        let request = if download {
            // A large file can take long, so a download's time limit covers
            // each step up to the start of the answer, not the body.
            config.timeout_global(None).timeout_resolve(Some(timeout)).timeout_connect(Some(timeout))
                .timeout_send_request(Some(timeout)).timeout_send_body(Some(timeout)).timeout_recv_response(Some(timeout))
                .build()
        } else {
            config.timeout_global(Some(timeout)).build()
        };
        agent.run(request)
    })
}

/// Header names in lower case; a header sent more than once has its values
/// joined by ", ", as HTTP allows.
fn response_headers(headers: &http::HeaderMap) -> Value {
    let mut joined: IndexMap<String, String> = IndexMap::new();
    for (name, value) in headers {
        let value = String::from_utf8_lossy(value.as_bytes());
        joined.entry(name.as_str().to_string())
            .and_modify(|v| { v.push_str(", "); v.push_str(&value); })
            .or_insert_with(|| value.into_owned());
    }
    let items = joined.into_iter().map(|(name, value)| {
        let key = Value::str(&name);
        (Key::of(&key), (key, Value::str(&value)))
    }).collect();
    Value::new_map(items, Meta::default())
}

// ----- downloads -----

enum Saving { File(&'static str, String), Net(ureq::Error) }

static TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// A name no one can guess: SipHash keyed from the operating system's random
/// source, so another user cannot plant a file or link where it will go.
fn unguessable() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(TEMPORARY.fetch_add(1, Ordering::Relaxed));
    hasher.write_u32(std::process::id());
    format!("{:016x}", hasher.finish())
}

fn file_fail(path: &str, error: io::Error) -> Saving {
    use io::ErrorKind::*;
    match error.kind() {
        NotFound => Saving::File("NotFound", format!("{path}: no such file or directory")),
        PermissionDenied => Saving::File("Permission", format!("{path}: permission denied")),
        IsADirectory => Saving::File("IsDirectory", format!("{path}: is a directory")),
        _ => Saving::File("File", format!("{path}: {error}")),
    }
}

/// Stream the body into the file all at once, as file.write does: into a
/// temporary file beside it, created exclusively under an unguessable name,
/// flushed to disk, then renamed over the target. A failure part way removes
/// the temporary file, so it leaves no partial file and the old one intact.
/// An existing file keeps its permissions; writing through a link updates the
/// file it points to.
fn save(response: &mut http::Response<ureq::Body>, path: &str) -> Result<(), Saving> {
    let mut target = PathBuf::from(path);
    if target.is_dir() { return Err(Saving::File("IsDirectory", format!("{path}: is a directory"))); }
    if let Some(parent) = target.parent().filter(|p| !p.as_os_str().is_empty()) {
        if !parent.is_dir() {
            return Err(Saving::File("NotFound", format!(
                "{path}: the directory {} does not exist; file.make_dir creates it", parent.display()
            )));
        }
    }
    if fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()) {
        if let Ok(real) = fs::canonicalize(&target) { target = real; }
    }
    let existing = fs::metadata(&target).ok().filter(|m| m.is_file());
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut temporary = PathBuf::new();
    let saved = (|| {
        let mut attempts = 0;
        let mut file = loop {
            temporary = target.with_file_name(format!(".{name}.{}.download", unguessable()));
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            // Private while it is written; it gets its final permissions below.
            #[cfg(unix)]
            if existing.is_some() { std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600); }
            match options.open(&temporary) {
                Ok(file) => break file,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists && attempts < 8 => attempts += 1,
                Err(e) => { temporary = PathBuf::new(); return Err(file_fail(path, e)); }
            }
        };
        let mut reader = response.body_mut().as_reader();
        let mut buffer = vec![0; 64 * 1024];
        loop {
            let read = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(Saving::Net(ureq::Error::from(e))),
            };
            file.write_all(&buffer[..read]).map_err(|e| file_fail(path, e))?;
        }
        file.sync_all().map_err(|e| file_fail(path, e))?;
        drop(file);
        if let Some(old) = &existing {
            fs::set_permissions(&temporary, old.permissions()).map_err(|e| file_fail(path, e))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                // Keeps the owner when the writer may; otherwise the writer owns it.
                let _ = std::os::unix::fs::chown(&temporary, Some(old.uid()), Some(old.gid()));
            }
        }
        fs::rename(&temporary, &target).map_err(|e| file_fail(path, e))
    })();
    if saved.is_err() && !temporary.as_os_str().is_empty() { let _ = fs::remove_file(&temporary); }
    saved
}

// ----- the package -----

pub fn package() -> Value {
    NativePackage::new("rs.http")
        .function("request", 4, |a| request(text(&a[0], "http.request")?, text(&a[1], "http.request")?, &a[2], &a[3], None))
        .function("download", 2, |a| {
            let url = text(&a[0], "http.download")?;
            request("GET", url, &Value::Null, &Value::Null, Some(text(&a[1], "http.download")?))
        })
        .function("set_timeout", 1, |a| {
            let seconds = match &a[0] {
                Value::Int(n) | Value::Long(n) => *n as f64,
                Value::Float(f) => *f,
                other => return type_err(format!("http.set_timeout expects a number of seconds, got {}", other.type_name())),
            };
            match Duration::try_from_secs_f64(seconds) {
                Ok(duration) if seconds > 0.0 && seconds <= 1e9 => { TIMEOUT.with(|t| t.set(duration)); Ok(Value::Null) }
                _ => range_err(format!("http.set_timeout needs a positive number of seconds, got {seconds}")),
            }
        })
        .function("timeout", 0, |_| Ok(Value::Float(TIMEOUT.with(Cell::get).as_secs_f64())))
        // The standard wording for a status, such as "Not Found" for 404, or "".
        .function("reason", 1, |a| match &a[0] {
            Value::Int(n) => Ok(Value::str(
                u16::try_from(*n).ok().and_then(|n| http::StatusCode::from_u16(n).ok())
                    .and_then(|s| s.canonical_reason()).unwrap_or(""),
            )),
            other => type_err(format!("http.reason expects an int, got {}", other.type_name())),
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_checked() {
        assert!(parse_url("https://example.com/a?b=c").is_ok());
        assert!(parse_url("HTTP://example.com").is_ok());
        assert!(parse_url("example.com").unwrap_err().contains("must start with http:// or https://"));
        assert!(parse_url("ftp://example.com").unwrap_err().contains("not ftp://"));
        assert!(parse_url("http://exa mple.com").unwrap_err().contains("%20"));
        assert!(parse_url("http://").is_err());
    }

    #[test]
    fn this_machine_is_local() {
        for host in ["localhost", "a.localhost", "127.0.0.1", "127.8.9.10", "::1"] { assert!(is_local(host), "{host}"); }
        for host in ["example.com", "10.0.0.1", "localhost.example.com"] { assert!(!is_local(host), "{host}"); }
        assert_eq!(bare_host(&"http://[::1]:8080/".parse().unwrap()), "::1");
    }

    #[test]
    fn no_proxy_lists_match_hosts() {
        for list in ["example.com", ".example.com", "*.example.com", "a.org, EXAMPLE.com ", "example.com:8080", "*"] {
            assert!(no_proxy_matches(list, "example.com"), "{list}");
            assert!(no_proxy_matches(list, "api.example.com"), "{list}");
        }
        for list in ["", "ample.com", "example.co", "other.com,a.example.com"] {
            assert!(!no_proxy_matches(list, "example.com"), "{list}");
        }
        assert!(no_proxy_matches("10.0.0.0/8", "10.20.30.40"));
        assert!(!no_proxy_matches("10.0.0.0/8", "11.0.0.1"));
        assert!(no_proxy_matches("192.168.1.5", "192.168.1.5"));
        assert!(no_proxy_matches("[fd00::1]:443", "fd00::1"));
        assert!(no_proxy_matches("fd00::/8", "fd12::3"));
    }

    #[test]
    fn ranges_match_addresses() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        assert!(in_range(ip("10.1.2.3"), "10.0.0.0", "8"));
        assert!(!in_range(ip("11.1.2.3"), "10.0.0.0", "8"));
        assert!(in_range(ip("fd00::1"), "fd00::", "8"));
        assert!(!in_range(ip("10.1.2.3"), "fd00::", "8"));
    }
}
