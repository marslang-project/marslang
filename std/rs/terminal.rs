//! `takepkg rs.terminal;` - what only the operating system can say about the
//! terminal, for `std.terminal`: whether standard output is one, how big it
//! is, and (on Windows) whether its console understands ANSI escape codes.
//!
//! Native because these are system calls. Everything else in `std.terminal`
//! is Marslang, in `std/terminal.mars`: building the escape sequences,
//! styles and colours, stripping them, and drawing progress bars.

use crate::value::*;
use std::io::IsTerminal;

const DEFAULT_COLUMNS: i64 = 80;
const DEFAULT_ROWS: i64 = 24;

pub fn package() -> Value {
    NativePackage::new("rs.terminal")
        // Whether standard output is a terminal, not a file or a pipe.
        .function("is_terminal", 0, |_| Ok(Value::Bool(std::io::stdout().is_terminal())))
        // The terminal's width in columns, or what COLUMNS says, or 80.
        .function("columns", 0, |_| Ok(Value::Int(size().0)))
        // The terminal's height in rows, or what LINES says, or 24.
        .function("rows", 0, |_| Ok(Value::Int(size().1)))
        // Whether ANSI escape codes will work in standard output. On Windows
        // this turns them on, once; elsewhere they always work.
        .function("enable_colors", 0, |_| Ok(Value::Bool(ansi_ready())))
        .build()
}

/// (columns, rows): asked of the terminal when standard output is one,
/// otherwise COLUMNS and LINES, otherwise 80 by 24. Each is settled apart.
fn size() -> (i64, i64) {
    let (os_columns, os_rows) = if std::io::stdout().is_terminal() { os_size() } else { (None, None) };
    (
        os_columns.or_else(|| env_size("COLUMNS")).unwrap_or(DEFAULT_COLUMNS),
        os_rows.or_else(|| env_size("LINES")).unwrap_or(DEFAULT_ROWS),
    )
}

/// A whole number above 0 from an environment variable.
fn env_size(name: &str) -> Option<i64> {
    std::env::var(name).ok()?.trim().parse::<i64>().ok().filter(|n| *n > 0)
}

#[cfg(not(windows))]
fn os_size() -> (Option<i64>, Option<i64>) {
    match terminal_size::terminal_size() {
        // Some pseudo-terminals report 0 by 0 until told otherwise.
        Some((terminal_size::Width(w), terminal_size::Height(h))) => {
            (Some(i64::from(w)).filter(|n| *n > 0), Some(i64::from(h)).filter(|n| *n > 0))
        }
        None => (None, None),
    }
}

#[cfg(not(windows))]
fn ansi_ready() -> bool { true }

/// The Windows console, asked for directly as src/date.rs asks for the time
/// zone: three kernel32 calls do not need the windows-sys crate.
#[cfg(windows)]
mod console {
    use std::ffi::c_void;

    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

    #[repr(C)]
    struct Coord { x: i16, y: i16 }
    #[repr(C)]
    struct SmallRect { left: i16, top: i16, right: i16, bottom: i16 }
    #[repr(C)]
    struct ScreenBufferInfo {
        size: Coord,
        cursor: Coord,
        attributes: u16,
        window: SmallRect,
        maximum_window_size: Coord,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(which: u32) -> *mut c_void;
        fn GetConsoleMode(handle: *mut c_void, mode: *mut u32) -> i32;
        fn SetConsoleMode(handle: *mut c_void, mode: u32) -> i32;
        fn GetConsoleScreenBufferInfo(handle: *mut c_void, info: *mut ScreenBufferInfo) -> i32;
    }

    /// Standard output's handle, or `None` when the process has none.
    fn stdout_handle() -> Option<*mut c_void> {
        // SAFETY: GetStdHandle only reads the process's own table of handles.
        let handle = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
        // INVALID_HANDLE_VALUE is -1; no handle at all is null.
        (!handle.is_null() && handle as isize != -1).then_some(handle)
    }

    /// Turn on ANSI escape codes for standard output's console. `None` when
    /// standard output is not a console at all; `Some` of whether they work.
    pub fn enable_virtual_terminal() -> Option<bool> {
        let handle = stdout_handle()?;
        let mut mode = 0u32;
        // SAFETY: the call only writes the mode into `mode`.
        if unsafe { GetConsoleMode(handle, &mut mode) } == 0 { return None; }
        if mode & ENABLE_VIRTUAL_TERMINAL_PROCESSING != 0 { return Some(true); }
        // SAFETY: sets a documented flag on a console handle just read.
        Some(unsafe { SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) } != 0)
    }

    /// The visible window's (columns, rows), not the scrolling buffer's.
    pub fn window_size() -> Option<(i64, i64)> {
        let handle = stdout_handle()?;
        let mut info = std::mem::MaybeUninit::<ScreenBufferInfo>::zeroed();
        // SAFETY: the structure matches CONSOLE_SCREEN_BUFFER_INFO and the
        // call only writes into it.
        if unsafe { GetConsoleScreenBufferInfo(handle, info.as_mut_ptr()) } == 0 { return None; }
        // SAFETY: zeroed, then filled by the call; every field is plain data.
        let info = unsafe { info.assume_init() };
        let columns = i64::from(info.window.right) - i64::from(info.window.left) + 1;
        let rows = i64::from(info.window.bottom) - i64::from(info.window.top) + 1;
        Some((columns, rows))
    }
}

#[cfg(windows)]
fn os_size() -> (Option<i64>, Option<i64>) {
    match console::window_size() {
        Some((columns, rows)) => (Some(columns).filter(|n| *n > 0), Some(rows).filter(|n| *n > 0)),
        None => (None, None),
    }
}

/// Windows: once, turn on ENABLE_VIRTUAL_TERMINAL_PROCESSING for standard
/// output's console, and remember whether that worked. Output that is a
/// terminal but not a console, such as Git Bash's mintty, takes ANSI codes
/// already; output that is not a terminal at all gets none.
#[cfg(windows)]
fn ansi_ready() -> bool {
    use std::sync::OnceLock;
    static READY: OnceLock<bool> = OnceLock::new();
    *READY.get_or_init(|| console::enable_virtual_terminal().unwrap_or_else(|| std::io::stdout().is_terminal()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_is_positive() {
        let (columns, rows) = size();
        assert!(columns > 0 && rows > 0);
    }

    #[test]
    fn environment_sizes_need_a_positive_whole_number() {
        // Names no test sets; the parser is what is checked.
        assert_eq!(env_size("MARSLANG_TERMINAL_TEST_UNSET"), None);
    }
}
