//! `takepkg rs.broadcast;` - what std.broadcast cannot do in Marslang:
//! waking a waiting task (a runtime `Signal`), and the transports of the
//! "machine" and "network" scopes. Channels, local delivery, the queue of
//! each local channel object, JSON, `on`, and `receive_for` are Marslang
//! (std/broadcast.mars).
//!
//! - **machine**: each channel object listens on a Unix domain socket (AF_UNIX,
//!   which Windows 10 and later support too) in a directory only the current
//!   user can open: 0700 and owned by the user on Linux and macOS, and on
//!   Windows an access list that grants the user alone. A socket's file is its
//!   registration, named by a hash of the channel name, the process id, and a
//!   counter. A sender connects to each registration of the channel in turn,
//!   and removes those whose program has died. There is no daemon.
//! - **network**: UDP datagrams to the broadcast address, on a port derived
//!   from the channel name. Anyone on the network can send and read them.
//!
//! Sockets are read by a background thread per channel object, which queues
//! messages, as text, in an `Inbox`; no `Value` crosses a thread. A task
//! waits for its inbox on a worker thread (`runtime::block_on_worker`), so
//! only that task waits. Every reader thread stops, and every registration is
//! removed, when its channel object is closed or the program's thread ends.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, UdpSocket};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::runtime::{self, Signal};
use crate::value::*;

/// Messages kept for a channel object that nobody is receiving from; beyond
/// this the oldest is dropped.
const QUEUE_LIMIT: usize = 1000;
/// The most JSON a machine message may hold, in bytes.
const MACHINE_LIMIT: usize = 1 << 20;
/// The most a network datagram may hold, header included: the largest UDP
/// payload over IPv4.
const NETWORK_LIMIT: usize = 65_507;
/// What every message starts with, then the channel name and a newline.
const HEADER: &str = "marslang-broadcast 1 ";
/// How long a sender or a reader waits on one connection before giving up.
const CONNECTION_TIME: Duration = Duration::from_secs(2);
/// Network ports are this, plus a hash of the name below PORT_SPAN.
const PORT_BASE: u32 = 41_000;
const PORT_SPAN: u32 = 8_000;

/// Messages a reader thread has received, for the interpreter thread.
#[derive(Default)]
struct Inbox {
    state: Mutex<InboxState>,
    ready: Condvar,
}

#[derive(Default)]
struct InboxState {
    /// Each message's JSON text, and who sent it, for network messages.
    queue: VecDeque<(String, Option<String>)>,
    closed: bool,
}

impl Inbox {
    fn lock(&self) -> std::sync::MutexGuard<'_, InboxState> {
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn push(&self, text: String, from: Option<String>) {
        let mut state = self.lock();
        if state.closed { return; }
        if state.queue.len() >= QUEUE_LIMIT { state.queue.pop_front(); }
        state.queue.push_back((text, from));
        drop(state);
        self.ready.notify_all();
    }

    fn close(&self) {
        self.lock().closed = true;
        self.ready.notify_all();
    }
}

/// One open channel object's native side.
enum Endpoint {
    /// A local channel object: Marslang keeps its queue, and waits here.
    Local(Rc<Signal>),
    Machine { inbox: Arc<Inbox>, stop: Arc<AtomicBool>, path: PathBuf },
    Network { inbox: Arc<Inbox>, stop: Arc<AtomicBool> },
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        match self {
            // std.broadcast wakes its waiters before closing it; here the
            // runtime may already be gone, as the program's thread ends.
            Endpoint::Local(_) => {}
            Endpoint::Machine { inbox, stop, path } => {
                stop.store(true, Ordering::SeqCst);
                // Wake the reader from accept, so it sees `stop` and ends.
                drop(sys::connect(path));
                let _ = std::fs::remove_file(&*path);
                open_registrations().remove(&*path);
                inbox.close();
            }
            // The reader wakes every quarter second to look at `stop`.
            Endpoint::Network { inbox, stop } => {
                stop.store(true, Ordering::SeqCst);
                inbox.close();
            }
        }
    }
}

/// What a wait needs of an endpoint, taken out so no borrow is held across it.
enum Waitable { Local(Rc<Signal>), Inbox(Arc<Inbox>) }

thread_local! {
    /// The program's open channel objects, by number. Dropped, and so closed,
    /// when the program's thread ends.
    static ENDPOINTS: RefCell<HashMap<i64, Endpoint>> = RefCell::new(HashMap::new());
    static NEXT: Cell<i64> = const { Cell::new(0) };
}

/// Registrations made by this process, for unique socket names across the
/// programs that tests run on threads of one process.
static REGISTRATIONS: AtomicU64 = AtomicU64::new(0);

/// The registrations of this process that are open, in any of its programs.
fn open_registrations() -> std::sync::MutexGuard<'static, std::collections::HashSet<PathBuf>> {
    static OPEN: OnceLock<Mutex<std::collections::HashSet<PathBuf>>> = OnceLock::new();
    OPEN.get_or_init(Default::default).lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn add(endpoint: Endpoint) -> i64 {
    let id = NEXT.with(|next| { next.set(next.get() + 1); next.get() });
    ENDPOINTS.with(|endpoints| endpoints.borrow_mut().insert(id, endpoint));
    id
}

fn waitable(id: i64) -> Option<Waitable> {
    ENDPOINTS.with(|endpoints| endpoints.borrow().get(&id).map(|endpoint| match endpoint {
        Endpoint::Local(signal) => Waitable::Local(signal.clone()),
        Endpoint::Machine { inbox, .. } | Endpoint::Network { inbox, .. } => Waitable::Inbox(inbox.clone()),
    }))
}

fn id(value: &Value, name: &str) -> RResult<i64> {
    match value {
        Value::Int(id) => Ok(*id),
        other => type_err(format!("rs.broadcast.{name} expects a channel number, got {}", other.type_name())),
    }
}

fn text<'a>(value: &'a Value, what: &str) -> RResult<&'a str> {
    match value {
        Value::Str(text) => Ok(text),
        other => type_err(format!("rs.broadcast expects {what} as a string, got {}", other.type_name())),
    }
}

fn digest(name: &str) -> Vec<u8> { Sha256::digest(name.as_bytes()).to_vec() }

/// The start of every registration file of the channel `name`.
fn prefix(name: &str) -> String {
    digest(name)[..6].iter().map(|byte| format!("{byte:02x}")).collect::<String>() + "-"
}

/// The UDP port of the network channel `name`.
fn port(name: &str) -> u16 {
    let hash = digest(name);
    let n = u32::from_be_bytes([hash[6], hash[7], hash[8], hash[9]]);
    (PORT_BASE + n % PORT_SPAN) as u16
}

/// Where network messages go: the limited broadcast address, or the address
/// in MARSLANG_BROADCAST_ADDRESS, such as a subnet's broadcast address, or a
/// loopback address to keep messages on this computer.
fn network_address() -> RResult<Ipv4Addr> {
    match std::env::var("MARSLANG_BROADCAST_ADDRESS") {
        Ok(text) if !text.trim().is_empty() => text.trim().parse().or_else(|_| err(ErrorKind::Error,
            format!("MARSLANG_BROADCAST_ADDRESS must be an IPv4 address such as 192.168.1.255, got \"{text}\""))),
        _ => Ok(Ipv4Addr::BROADCAST),
    }
}

/// A message's bytes on the wire.
fn frame(name: &str, json: &str) -> Vec<u8> { format!("{HEADER}{name}\n{json}").into_bytes() }

/// The JSON of a message on the wire for channel `name`; `None` for anything
/// else, which is dropped.
fn unframe(name: &str, bytes: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    let (channel, json) = text.strip_prefix(HEADER)?.split_once('\n')?;
    (channel == name).then(|| json.to_string())
}

/// The per-user directory of machine registrations, made and checked once.
fn directory() -> RResult<PathBuf> {
    static DIRECTORY: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    DIRECTORY.get_or_init(sys::directory).clone().or_else(|message| err(ErrorKind::Error, message))
}

/// Open a machine channel object: a socket in the per-user directory and a
/// thread that reads it.
fn machine_open(name: &str) -> RResult<Value> {
    let dir = directory()?;
    let n = REGISTRATIONS.fetch_add(1, Ordering::SeqCst);
    let path = dir.join(format!("{}{}-{n}.sock", prefix(name), std::process::id()));
    let listener = sys::listen(&path).or_else(|error| err(ErrorKind::Error,
        format!("could not listen on machine channel \"{name}\" at {}: {error}", path.display())))?;
    let inbox = Arc::new(Inbox::default());
    let stop = Arc::new(AtomicBool::new(false));
    let (thread_inbox, thread_stop, channel) = (inbox.clone(), stop.clone(), name.to_string());
    let started = std::thread::Builder::new().name("marslang broadcast".into()).spawn(move || loop {
        let accepted = sys::accept(&listener);
        if thread_stop.load(Ordering::SeqCst) { break; }
        let Ok(mut stream) = accepted else {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        };
        let _ = stream.set_read_timeout(Some(CONNECTION_TIME));
        let mut bytes = Vec::new();
        let limit = (MACHINE_LIMIT + HEADER.len() + channel.len() + 1) as u64;
        if (&mut stream).take(limit + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > limit { continue; }
        if let Some(json) = unframe(&channel, &bytes) { thread_inbox.push(json, None); }
    });
    if let Err(error) = started {
        let _ = std::fs::remove_file(&path);
        return err(ErrorKind::Error, format!("could not start a thread for machine channel \"{name}\": {error}"));
    }
    let shown = path.display().to_string();
    open_registrations().insert(path.clone());
    let id = add(Endpoint::Machine { inbox, stop, path });
    Ok(Value::array(vec![Value::Int(id), Value::str(&shown)]))
}

/// Deliver to every registration of machine channel `name`: how many took it.
fn machine_send(name: &str, json: &str) -> RResult<Value> {
    if json.len() > MACHINE_LIMIT {
        return range_err(format!("the message is {} bytes as JSON; a machine channel carries at most {MACHINE_LIMIT}", json.len()));
    }
    let dir = directory()?;
    let start = prefix(name);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .or_else(|error| err(ErrorKind::Error, format!("could not read {}: {error}", dir.display())))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&start) && n.ends_with(".sock")))
        .collect();
    paths.sort();
    let bytes = frame(name, json);
    let mut reached = 0;
    for path in paths {
        match sys::connect(&path) {
            Ok(mut stream) => {
                let _ = stream.set_write_timeout(Some(CONNECTION_TIME));
                if stream.write_all(&bytes).and_then(|()| stream.flush()).is_ok() { reached += 1; }
                let _ = stream.shutdown(std::net::Shutdown::Write);
            }
            Err(_) if !owner_alive(&path) => { let _ = std::fs::remove_file(&path); }
            // A program that is alive but did not answer keeps its registration.
            Err(_) => {}
        }
    }
    Ok(Value::Int(reached))
}

/// Whether the program that made a registration is still running: its
/// process exists, and, when it is this process, the registration is open.
fn owner_alive(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let Some(pid) = name.split('-').nth(1).and_then(|pid| pid.parse::<u32>().ok()) else { return false };
    if pid == std::process::id() { return open_registrations().contains(path); }
    sys::alive(pid)
}

fn network_open(name: &str) -> RResult<Value> {
    let address = network_address()?;
    let bind = if address.is_loopback() { address } else { Ipv4Addr::UNSPECIFIED };
    let port = port(name);
    let socket = sys::udp(bind, port).or_else(|error| err(ErrorKind::Error,
        format!("could not listen on network channel \"{name}\", UDP port {port}: {error}")))?;
    let _ = socket.set_read_timeout(Some(Duration::from_millis(250)));
    let inbox = Arc::new(Inbox::default());
    let stop = Arc::new(AtomicBool::new(false));
    let (thread_inbox, thread_stop, channel) = (inbox.clone(), stop.clone(), name.to_string());
    std::thread::Builder::new().name("marslang broadcast".into()).spawn(move || {
        let mut buffer = vec![0u8; 65_536];
        while !thread_stop.load(Ordering::SeqCst) {
            // Timeouts, and on Windows reports of datagrams that went
            // nowhere, are errors to wait past.
            let Ok((length, from)) = socket.recv_from(&mut buffer) else { continue };
            if length > NETWORK_LIMIT { continue; }
            if let Some(json) = unframe(&channel, &buffer[..length]) { thread_inbox.push(json, Some(from.ip().to_string())); }
        }
    }).or_else(|error| err(ErrorKind::Error, format!("could not start a thread for network channel \"{name}\": {error}")))?;
    Ok(Value::Int(add(Endpoint::Network { inbox, stop })))
}

fn network_send(name: &str, json: &str) -> RResult<Value> {
    let bytes = frame(name, json);
    if bytes.len() > NETWORK_LIMIT {
        return range_err(format!("the message is {} bytes with its header; a network channel carries at most {NETWORK_LIMIT}, \
            the most one UDP datagram holds", bytes.len()));
    }
    let address = network_address()?;
    let failed = |error: std::io::Error| RuntimeError::new(ErrorKind::Error, format!("could not send on network channel \"{name}\": {error}"));
    let local = if address.is_loopback() { address } else { Ipv4Addr::UNSPECIFIED };
    let socket = UdpSocket::bind((local, 0)).map_err(failed)?;
    socket.set_broadcast(true).map_err(failed)?;
    socket.send_to(&bytes, (address, port(name))).map_err(failed)?;
    Ok(Value::Null)
}

/// Wait until channel object `id` has a message, and take it: arr(text,
/// from), or null once it is closed. Only the calling task waits.
fn next(id: i64) -> RResult<Value> {
    loop {
        let inbox = match waitable(id) {
            None => return Ok(Value::Null),
            Some(Waitable::Local(signal)) => { signal.wait()?; return Ok(Value::Null); }
            Some(Waitable::Inbox(inbox)) => inbox,
        };
        {
            let mut state = inbox.lock();
            if let Some((text, from)) = state.queue.pop_front() {
                return Ok(Value::array(vec![Value::str(&text), from.map(|f| Value::str(&f)).unwrap_or(Value::Null)]));
            }
            if state.closed { return Ok(Value::Null); }
        }
        // The worker only waits for a message, and leaves taking it to this
        // thread, so a task cancelled meanwhile loses nothing.
        let abandoned = Arc::new(AtomicBool::new(false));
        let (worker_inbox, worker_abandoned) = (inbox.clone(), abandoned.clone());
        let waited = runtime::block_on_worker(move || {
            let mut state = worker_inbox.lock();
            while state.queue.is_empty() && !state.closed && !worker_abandoned.load(Ordering::SeqCst) {
                state = worker_inbox.ready.wait(state).unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        });
        if waited.is_err() {
            // Let the worker go: nobody waits for it now.
            let guard = inbox.lock();
            abandoned.store(true, Ordering::SeqCst);
            drop(guard);
            inbox.ready.notify_all();
        }
        waited?;
    }
}

pub fn package() -> Value {
    NativePackage::new("rs.broadcast")
        // A local channel object's signal: its number.
        .function("local_open", 0, |_| Ok(Value::Int(add(Endpoint::Local(Rc::new(Signal::new()))))))
        // Wake the tasks waiting on a local channel object.
        .function("notify", 1, |args| {
            let signal = waitable(id(&args[0], "notify")?);
            if let Some(Waitable::Local(signal)) = signal { signal.notify_all(); }
            Ok(Value::Null)
        })
        .function("machine_open", 1, |args| machine_open(text(&args[0], "the channel name")?))
        .function("machine_send", 2, |args| machine_send(text(&args[0], "the channel name")?, text(&args[1], "the message")?))
        .function("network_open", 1, |args| network_open(text(&args[0], "the channel name")?))
        .function("network_send", 2, |args| network_send(text(&args[0], "the channel name")?, text(&args[1], "the message")?))
        // The next message of a machine or network channel object, waiting
        // for one; for a local one, wait until it is notified, and give null.
        .function("next", 1, |args| next(id(&args[0], "next")?))
        // Stop listening: the reader thread ends and the registration goes.
        .function("close", 1, |args| {
            let id = id(&args[0], "close")?;
            let endpoint = ENDPOINTS.with(|endpoints| endpoints.borrow_mut().remove(&id));
            drop(endpoint);
            Ok(Value::Null)
        })
        .function("network_port", 1, |args| Ok(Value::Int(port(text(&args[0], "the channel name")?) as i64)))
        .function("directory", 0, |_| Ok(Value::str(&directory()?.display().to_string())))
        .build()
}

#[cfg(unix)]
mod sys {
    use std::io;
    use std::net::{Ipv4Addr, UdpSocket};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use std::os::unix::io::FromRawFd;
    pub use std::os::unix::net::{UnixListener as Listener, UnixStream as Stream};
    use std::path::{Path, PathBuf};

    /// The longest socket path: sun_path less its closing zero.
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
    const PATH_LIMIT: usize = 103;
    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd")))]
    const PATH_LIMIT: usize = 107;

    /// `marslang-broadcast-UID` in XDG_RUNTIME_DIR, which is the user's own,
    /// or else the temporary directory: made 0700, and refused unless it is
    /// a real directory that this user owns and only this user can open.
    pub fn directory() -> Result<PathBuf, String> {
        let uid = unsafe { libc::geteuid() };
        let base = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)
            .filter(|base| base.is_absolute() && base.is_dir())
            .unwrap_or_else(std::env::temp_dir);
        let dir = base.join(format!("marslang-broadcast-{uid}"));
        match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("could not make the machine channel directory {}: {error}", dir.display())),
        }
        let meta = std::fs::symlink_metadata(&dir).map_err(|error| format!("could not read {}: {error}", dir.display()))?;
        if !meta.file_type().is_dir() || meta.uid() != uid {
            return Err(format!("{} is not a directory of this user's, so machine channels cannot use it safely", dir.display()));
        }
        if meta.mode() & 0o777 != 0o700 {
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|error| format!("could not make {} private: {error}", dir.display()))?;
        }
        Ok(dir)
    }

    fn fits(path: &Path) -> io::Result<()> {
        let length = path.as_os_str().len();
        if length > PATH_LIMIT {
            return Err(io::Error::other(format!("the path is {length} bytes, more than a socket path may be ({PATH_LIMIT})")));
        }
        Ok(())
    }

    pub fn listen(path: &Path) -> io::Result<Listener> {
        fits(path)?;
        let listener = Listener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(listener)
    }

    pub fn accept(listener: &Listener) -> io::Result<Stream> { listener.accept().map(|(stream, _)| stream) }

    pub fn connect(path: &Path) -> io::Result<Stream> {
        fits(path)?;
        Stream::connect(path)
    }

    pub fn alive(pid: u32) -> bool {
        let Ok(pid) = libc::pid_t::try_from(pid) else { return false };
        // EPERM: it exists, though another user's (a reused pid).
        let signalled = unsafe { libc::kill(pid, 0) } == 0;
        signalled || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }

    /// A UDP socket bound to `ip:port` that other sockets may bind too, so
    /// several channel objects and programs hear each broadcast.
    pub fn udp(ip: Ipv4Addr, port: u16) -> io::Result<UdpSocket> {
        unsafe {
            let fd = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0);
            if fd < 0 { return Err(io::Error::last_os_error()); }
            // Owned from here, so it is closed on every return.
            let socket = UdpSocket::from_raw_fd(fd);
            libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
            let on: libc::c_int = 1;
            for option in [libc::SO_REUSEADDR, libc::SO_REUSEPORT] {
                let set = libc::setsockopt(fd, libc::SOL_SOCKET, option, &on as *const libc::c_int as *const libc::c_void,
                    std::mem::size_of::<libc::c_int>() as libc::socklen_t);
                if set != 0 { return Err(io::Error::last_os_error()); }
            }
            let mut address: libc::sockaddr_in = std::mem::zeroed();
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
            { address.sin_len = std::mem::size_of::<libc::sockaddr_in>() as u8; }
            address.sin_family = libc::AF_INET as libc::sa_family_t;
            address.sin_port = port.to_be();
            address.sin_addr = libc::in_addr { s_addr: u32::from(ip).to_be() };
            let bound = libc::bind(fd, &address as *const libc::sockaddr_in as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t);
            if bound != 0 { return Err(io::Error::last_os_error()); }
            Ok(socket)
        }
    }
}

#[cfg(windows)]
mod sys {
    use std::io;
    use std::net::{Ipv4Addr, Shutdown, UdpSocket};
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawSocket;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, STILL_ACTIVE};
    use windows_sys::Win32::Networking::WinSock as ws;
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW, SetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        GetSecurityDescriptorDacl, GetTokenInformation, TokenUser, ACL, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetExitCodeProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION};

    fn wide(text: &std::ffi::OsStr) -> Vec<u16> { text.encode_wide().chain(Some(0)).collect() }

    /// `marslang-broadcast` in the user's temporary directory, %TEMP%, which
    /// is in the user's profile: refused unless the user (or Administrators
    /// or SYSTEM, who can open anything anyway) owns it, and given an access
    /// list, not inherited, that grants the current user alone, and that what
    /// is made in it inherits. %LOCALAPPDATA% is not used: some file system
    /// filters there break AF_UNIX socket files.
    pub fn directory() -> Result<PathBuf, String> {
        let dir = std::env::temp_dir().join("marslang-broadcast");
        std::fs::create_dir_all(&dir).map_err(|error| format!("could not make the machine channel directory {}: {error}", dir.display()))?;
        let meta = std::fs::symlink_metadata(&dir).map_err(|error| format!("could not read {}: {error}", dir.display()))?;
        if !meta.file_type().is_dir() {
            return Err(format!("{} is not a directory, so machine channels cannot use it safely", dir.display()));
        }
        let user = user_sid().map_err(|error| format!("could not find the current user: {error}"))?;
        let owner = owner_sid(&dir).map_err(|error| format!("could not read who owns {}: {error}", dir.display()))?;
        if owner != user && owner != "S-1-5-32-544" && owner != "S-1-5-18" {
            return Err(format!("{} belongs to another user ({owner}), so machine channels cannot use it safely", dir.display()));
        }
        restrict(&dir, &user).map_err(|error| format!("could not make {} private: {error}", dir.display()))?;
        Ok(dir)
    }

    /// A SID as text, such as S-1-5-21-...
    unsafe fn sid_text(sid: windows_sys::Win32::Security::PSID) -> io::Result<String> {
        let mut text = std::ptr::null_mut();
        if ConvertSidToStringSidW(sid, &mut text) == 0 { return Err(io::Error::last_os_error()); }
        let length = (0..).take_while(|&i| *text.add(i) != 0).count();
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, length));
        LocalFree(text.cast());
        Ok(sid)
    }

    /// Who owns a file or directory, as a SID.
    fn owner_sid(path: &Path) -> io::Result<String> {
        let path = wide(path.as_os_str());
        unsafe {
            let mut owner = std::ptr::null_mut();
            let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            let code = GetNamedSecurityInfoW(path.as_ptr(), SE_FILE_OBJECT, OWNER_SECURITY_INFORMATION, &mut owner,
                std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), &mut descriptor);
            if code != 0 { return Err(io::Error::from_raw_os_error(code as i32)); }
            let sid = sid_text(owner);
            LocalFree(descriptor);
            sid
        }
    }

    /// The current user's SID, as text such as S-1-5-21-...
    fn user_sid() -> io::Result<String> {
        unsafe {
            let mut token = std::ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 { return Err(io::Error::last_os_error()); }
            let mut needed = 0u32;
            GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
            // u64s, so the TOKEN_USER at its start is aligned.
            let mut buffer = vec![0u64; (needed as usize).div_ceil(8).max(1)];
            let got = GetTokenInformation(token, TokenUser, buffer.as_mut_ptr().cast(), (buffer.len() * 8) as u32, &mut needed);
            CloseHandle(token);
            if got == 0 { return Err(io::Error::last_os_error()); }
            let user = &*(buffer.as_ptr() as *const TOKEN_USER);
            sid_text(user.User.Sid)
        }
    }

    /// Replace the directory's access list with one entry: full control for
    /// the current user, inherited by everything in it. SetNamedSecurityInfo
    /// passes it on to what the directory already holds.
    fn restrict(dir: &Path, user: &str) -> io::Result<()> {
        let sddl = wide(std::ffi::OsStr::new(&format!("D:P(A;OICI;FA;;;{user})")));
        unsafe {
            let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut descriptor, std::ptr::null_mut()) == 0 {
                return Err(io::Error::last_os_error());
            }
            let (mut present, mut defaulted) = (0, 0);
            let mut dacl: *mut ACL = std::ptr::null_mut();
            let result = if GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) == 0 {
                Err(io::Error::last_os_error())
            } else {
                let path = wide(dir.as_os_str());
                match SetNamedSecurityInfoW(path.as_ptr(), SE_FILE_OBJECT, DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    std::ptr::null_mut(), std::ptr::null_mut(), dacl, std::ptr::null()) {
                    0 => Ok(()),
                    code => Err(io::Error::from_raw_os_error(code as i32)),
                }
            };
            LocalFree(descriptor);
            result
        }
    }

    fn startup() {
        static STARTED: std::sync::Once = std::sync::Once::new();
        STARTED.call_once(|| unsafe {
            let mut data: ws::WSADATA = std::mem::zeroed();
            ws::WSAStartup(0x202, &mut data);
        });
    }

    fn last() -> io::Error { io::Error::from_raw_os_error(unsafe { ws::WSAGetLastError() }) }

    fn check(result: i32) -> io::Result<()> { if result == ws::SOCKET_ERROR { Err(last()) } else { Ok(()) } }

    /// An AF_UNIX stream socket.
    pub struct Stream(ws::SOCKET);
    pub struct Listener(Stream);

    impl Drop for Stream {
        fn drop(&mut self) { unsafe { ws::closesocket(self.0); } }
    }

    fn address(path: &Path) -> io::Result<ws::SOCKADDR_UN> {
        let bytes = path.to_str().ok_or_else(|| io::Error::other("the socket path is not valid Unicode"))?.as_bytes();
        let mut address: ws::SOCKADDR_UN = unsafe { std::mem::zeroed() };
        if bytes.len() >= address.sun_path.len() {
            return Err(io::Error::other(format!("the path is {} bytes, more than a socket path may be ({})", bytes.len(), address.sun_path.len() - 1)));
        }
        address.sun_family = ws::AF_UNIX;
        for (to, from) in address.sun_path.iter_mut().zip(bytes) { *to = *from as i8; }
        Ok(address)
    }

    fn unix_socket() -> io::Result<Stream> {
        startup();
        let socket = unsafe { ws::socket(ws::AF_UNIX as i32, ws::SOCK_STREAM, 0) };
        if socket == ws::INVALID_SOCKET { return Err(last()); }
        Ok(Stream(socket))
    }

    const ADDRESS_SIZE: i32 = std::mem::size_of::<ws::SOCKADDR_UN>() as i32;

    pub fn listen(path: &Path) -> io::Result<Listener> {
        let address = address(path)?;
        let socket = unix_socket()?;
        unsafe {
            check(ws::bind(socket.0, &address as *const ws::SOCKADDR_UN as *const ws::SOCKADDR, ADDRESS_SIZE))?;
            check(ws::listen(socket.0, 128))?;
        }
        Ok(Listener(socket))
    }

    pub fn accept(listener: &Listener) -> io::Result<Stream> {
        let socket = unsafe { ws::accept((listener.0).0, std::ptr::null_mut(), std::ptr::null_mut()) };
        if socket == ws::INVALID_SOCKET { return Err(last()); }
        Ok(Stream(socket))
    }

    pub fn connect(path: &Path) -> io::Result<Stream> {
        let address = address(path)?;
        let socket = unix_socket()?;
        unsafe { check(ws::connect(socket.0, &address as *const ws::SOCKADDR_UN as *const ws::SOCKADDR, ADDRESS_SIZE))?; }
        Ok(socket)
    }

    impl Stream {
        fn timeout(&self, option: i32, length: Option<Duration>) -> io::Result<()> {
            let ms: u32 = length.map(|d| d.as_millis().clamp(1, u32::MAX as u128) as u32).unwrap_or(0);
            unsafe { check(ws::setsockopt(self.0, ws::SOL_SOCKET, option, &ms as *const u32 as *const u8, 4)) }
        }
        pub fn set_read_timeout(&self, length: Option<Duration>) -> io::Result<()> { self.timeout(ws::SO_RCVTIMEO, length) }
        pub fn set_write_timeout(&self, length: Option<Duration>) -> io::Result<()> { self.timeout(ws::SO_SNDTIMEO, length) }
        pub fn shutdown(&self, _how: Shutdown) -> io::Result<()> { unsafe { check(ws::shutdown(self.0, ws::SD_SEND)) } }
    }

    impl io::Read for Stream {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let length = buffer.len().min(i32::MAX as usize) as i32;
            let got = unsafe { ws::recv(self.0, buffer.as_mut_ptr(), length, 0) };
            if got < 0 { Err(last()) } else { Ok(got as usize) }
        }
    }

    impl io::Write for Stream {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let length = bytes.len().min(i32::MAX as usize) as i32;
            let sent = unsafe { ws::send(self.0, bytes.as_ptr(), length, 0) };
            if sent < 0 { Err(last()) } else { Ok(sent as usize) }
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    pub fn alive(pid: u32) -> bool {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() { return false; }
            let mut code = 0u32;
            let got = GetExitCodeProcess(process, &mut code);
            CloseHandle(process);
            got != 0 && code == STILL_ACTIVE as u32
        }
    }

    /// A UDP socket bound to `ip:port` that other sockets may bind too, so
    /// several channel objects and programs hear each broadcast.
    pub fn udp(ip: Ipv4Addr, port: u16) -> io::Result<UdpSocket> {
        startup();
        unsafe {
            let raw = ws::socket(ws::AF_INET as i32, ws::SOCK_DGRAM, ws::IPPROTO_UDP);
            if raw == ws::INVALID_SOCKET { return Err(last()); }
            // Owned from here, so it is closed on every return.
            let socket = UdpSocket::from_raw_socket(raw as u64);
            let on: i32 = 1;
            check(ws::setsockopt(raw, ws::SOL_SOCKET, ws::SO_REUSEADDR, &on as *const i32 as *const u8, 4))?;
            let mut address: ws::SOCKADDR_IN = std::mem::zeroed();
            address.sin_family = ws::AF_INET;
            address.sin_port = port.to_be();
            address.sin_addr.S_un.S_addr = u32::from(ip).to_be();
            check(ws::bind(raw, &address as *const ws::SOCKADDR_IN as *const ws::SOCKADDR, std::mem::size_of::<ws::SOCKADDR_IN>() as i32))?;
            Ok(socket)
        }
    }
}
