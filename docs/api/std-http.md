# std.http

An HTTP and HTTPS client: fetching web pages, calling JSON APIs, and
downloading files.

This example needs the internet:

```mars
takepkg std.http;

func m{
    page = http.get("https://example.com/");
    out(page.status, page.ok, page.header("content-type"));   // 200 true text/html

    release = http.get_json("https://api.github.com/repos/marslang-project/marslang/releases/latest");
    out(release.get("tag_name"));                              // rs-0.16.0, or whatever is newest

    http.download("https://example.com/", "example.html");
}
```

A request that gets an answer returns a `Response`, whatever the status: a 404
or a 500 is an answer, so check `ok`. Only a failure to get an answer at all
(the host was not found, the connection was refused, the time ran out) raises
an error. `get_json` and `download` are the exceptions: they want a 2xx answer
and raise `http.StatusError` for any other.

| Function | Result |
| --- | --- |
| `get(url)` | The `Response` to a GET |
| `post(url, text)` | The `Response` to a POST of `text`, as `text/plain; charset=utf-8` |
| `post_json(url, value)` | The `Response` to a POST of `json.stringify(value)`, as `application/json` |
| `request(method, url, headers, body)` | The `Response` to any request: `method` such as `"PUT"`, `headers` a map of strings, `body` a string or `null` |
| `get_json(url)` | The body of a GET parsed as JSON; `StatusError` unless the status is 2xx |
| `download(url, path)` | Streams the body of a GET to a file; returns the `Response`, with an empty `text` |
| `set_timeout(seconds)` | Sets the time limit for later requests; the default is 30 seconds |

Every function takes a full URL, starting with `http://` or `https://`.

## Responses

| Member | Value |
| --- | --- |
| `status` | The status as an `int`, such as `200` or `404` |
| `ok` | Whether `status` is from `200` to `299` |
| `text` | The body as a string |
| `headers` | A map from each header name, in lower case, to its value |
| `url` | The URL that answered, after any redirects |
| `json()` | `text` parsed as JSON, as `json.parse` does |
| `header(name)` | The value of a header, whatever the case of `name`; `null` when absent |

A header sent more than once, such as `Set-Cookie`, has its values joined by
`", "` in one string. Bytes of the body that are not UTF-8 become the
replacement character `U+FFFD` rather than raising an error, so a page in
another encoding still reads, with its non-ASCII letters lost; use `download`
for images, archives, and other bytes that are not text.

## JSON APIs

```mars
takepkg std.http;

func m{
    // Needs the internet, and a service to talk to.
    api = "https://example.com/api";
    created = http.post_json(api + "/notes", map().set("title", "milk").set("done", false));
    if (not created.ok){ out("refused:", created.status, created.text); }

    run{
        note = http.get_json(api + "/notes/7");
        out(note.get("title"));
    } handle(http.StatusError e){
        out(e.status, e.response.text);   // such as 404 and the service's explanation
    }

    headers = map().set("Authorization", "Bearer " + "my-token").set("Accept", "application/json");
    gone = http.request("DELETE", api + "/notes/7", headers, null);
    out(gone.status);
}
```

`post_json` sends `Content-Type: application/json` and `get_json` sends
`Accept: application/json`. `request` sends only the headers you give it, plus
`User-Agent: marslang/<version>` (such as `marslang/rs-0.16.0`) unless you set
your own, and works with any method, including ones of your own such as
`PROPFIND`; the method is put in upper case. Content-Type is not guessed: put
it in `headers` when sending a body.

## Downloads

`download(url, path)` streams the body into the file, so a file of any size
takes little memory and has no size limit. It is all or nothing, as
`file.write` is: the body goes into a temporary file beside `path`, created
under a name no one can guess, flushed to disk, and renamed over `path` only
once all of it has arrived. A download that fails part way removes the
temporary file, so it never leaves half a file, and a file already at `path`
stays as it was. An existing file keeps its permissions.

A status outside 200 to 299 raises `StatusError` and writes nothing, since an
error page saved under the name of the file you wanted is a surprise found
later. A directory in `path` that does not exist raises `file.NotFoundError`,
and other problems with the file the other `std.file` errors.

## Many requests at once

Every request runs on a worker thread, so in [tasks](async.md) only the task
that sent it waits, and the others run. Ten requests to a server that takes a
second each, sent from ten tasks, take about a second, not ten:

```mars
takepkg std.http;
takepkg std.async;

async func size(string url) => http.get(url).text.len();

func m{
    tasks = arr();
    for (url, arr("https://example.com/", "https://example.org/")){ tasks.add(size(url)); }
    out(await async.all(tasks));            // two page sizes, fetched together
}
```

Outside a task a request simply waits for the answer, as it always did. The
time limit, the proxies, and the limits below apply to each request as before.

**Cancelling.** When a task is cancelled (`t.cancel()`, `async.timeout`, `async.race`),
the request it is waiting for stops mattering at once: the task raises
`CancelledError` without waiting for the server, and the answer, if one
arrives, is thrown away. The request itself may go on in the background until
it finishes or reaches the time limit, since the network call cannot be
interrupted, so a cancelled `POST` may still reach the server. A cancelled
`download` stops writing at the next piece of data it receives and removes
its temporary file, so a half-written file is never left; the file at `path`
is either what it was before or, if the last piece had already arrived when
the task was cancelled, the whole new body, never part of it.

## Errors

| Error | When |
| --- | --- |
| `http.HttpError` | No answer: the host was not found, the connection was refused or dropped, TLS failed, the time ran out, more than 10 redirects, or a body over 100 MB |
| `http.UrlError` | The URL is not `http://` or `https://`, or is not a valid URL; inherits from `HttpError` |
| `http.StatusError` | `get_json` or `download` got a status outside 200 to 299; inherits from `HttpError` |
| `SyntaxError` | A header name, header value, or method that is not valid, or a `get_json` body that is not JSON |
| `TypeError` | A header that is not a string, or a body that is neither a string nor `null` |

`handle(http.HttpError e)` catches all three of the package's own errors.
Messages name the method and the URL and say why in plain words, and when a
proxy was used they name it too, since a proxy that is down looks like any
other failure:

```
HttpError: GET http://localhost:9/: the connection was refused: nothing is listening at that address and port
HttpError: GET https://example.com/: no complete answer within 30 seconds (http.set_timeout changes the limit)
HttpError: GET https://example.com/: the connection was refused: nothing is listening at that address and port, through the proxy 127.0.0.1:7890 (from HTTPS_PROXY)
UrlError: "example.com" is not a URL: it must start with http:// or https://
StatusError: GET https://example.com/missing: the server answered 404 Not Found
```

A `StatusError` has two fields besides `message`: `status`, the status as an
`int`, and `response`, the whole `Response`, whose `text` often explains what
went wrong.

## Proxies

Proxies come from the environment, as in curl and most other tools:

| Variable | Used for |
| --- | --- |
| `HTTPS_PROXY` | `https://` URLs |
| `HTTP_PROXY` | `http://` URLs |
| `ALL_PROXY` | Either, when the one above is not set |
| `NO_PROXY` | Hosts to reach directly, separated by commas |

Each may be written in lower case too; when both are set, the lower-case one
is used. A proxy is written `http://host:port`, or `host:port`; `http://`,
`https://`, `socks4://`, `socks4a://`, `socks5://`, and `socks5h://` proxies
work, with `user:password@` before the host when the proxy needs a login.

In `NO_PROXY`, `example.com`, `.example.com`, and `*.example.com` all match
`example.com` and every name under it; an IP address matches itself;
`10.0.0.0/8` matches the addresses in that range; and `*` matches every host.
`localhost`, names ending in `.localhost`, `127.0.0.1` (and the rest of
`127.0.0.0/8`), and `::1` are always reached directly, whatever these say.

The proxy is chosen for the URL you give; redirects to another host keep the
same choice.

## Limits

- **Time:** a request must finish, from connecting to the last byte of the
  body, within 30 seconds, or `set_timeout(seconds)` (an `int` or a `float`).
  For `download` the limit covers each step up to the start of the answer but
  not the body, which takes as long as it needs; a server that stops sending
  part way through a download can make it wait indefinitely.
- **Redirects:** up to 10 are followed; one more raises `HttpError`. 301, 302,
  and 303 turn a POST or other method into a GET, as browsers and curl do;
  307 and 308 keep the method, and for a POST, PUT, PATCH, or DELETE raise
  `HttpError` rather than send it again. An `Authorization` header goes along
  only to the same host, and never from `https://` to `http://`.
- **Size:** `get`, `post`, `post_json`, `request`, and `get_json` refuse a body
  over 100 MB, at once when the server says its size and otherwise when the
  limit is reached. `download` has no limit.
- **URLs** must be `http://` or `https://` and contain only the characters
  URLs allow: write a space as `%20`, and other characters percent-encoded.
- **Not supported:** compressed (gzip) answers, cookies kept between requests,
  HTTP/2, and client certificates. Certificates are always checked; there is
  no way to turn that off.

## How it works

The network is native ([std/rs/http.rs](../../std/rs/http.rs)), by Rust's
[ureq](https://docs.rs/ureq) crate, which speaks HTTP/1.1 and reuses
connections between requests to the same server. TLS is
[rustls](https://docs.rs/rustls) with the ring cryptography library, and
certificates are checked against Mozilla's list of root certificates, built
into the interpreter by the webpki-roots crate, so a static Linux binary in a
minimal container such as Alpine or Debian slim needs no system CA files. The
system's own certificate store is not consulted, so a certificate trusted
only there (as some company networks add) is not trusted by `std.http`.

`Response`, the error families, JSON, and which statuses are errors are
Marslang, in [std/http.mars](../../std/http.mars).

Each request is sent from a worker thread, with one connection pool shared by
the whole program, so requests from different tasks reuse connections too.
