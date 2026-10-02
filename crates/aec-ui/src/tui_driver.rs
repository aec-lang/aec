//! Interactive terminal driver for the AEC terminal backend.
//!
//! Terminal control is done with ANSI sequences and `termios` through `libc`,
//! which the workspace already depends on, so the backend adds no new
//! dependency and `cargo test --locked` stays reproducible.

use std::io::{Read, Write};
use std::time::Duration;

use aec_ast::{Program, UiDecl};

use crate::tui::{self, Key};
use crate::AecApp;

/// Puts the terminal in raw mode and returns a guard that restores it.
///
/// `termios` only exists on POSIX systems; on Windows the terminal driver is
/// used without raw mode, so the type is a no-op there instead of a build
/// failure (`libc` on Windows has no `termios`).
#[cfg(unix)]
pub struct RawMode {
    #[allow(dead_code)]
    original: libc::termios,
}

/// Non-POSIX stand-in so `RawMode::enable()` keeps compiling everywhere.
#[cfg(not(unix))]
pub struct RawMode;

#[cfg(unix)]
impl RawMode {
    /// Enters raw mode on stdout, restoring the previous settings on drop.
    pub fn enable() -> Result<Self, String> {
        // SAFETY: `tcgetattr` fills a termios we own; every path that fails
        // returns before `original` is read again.
        unsafe {
            let mut original: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(libc::STDOUT_FILENO, &mut original) != 0 {
                return Err("cannot read terminal settings; is stdout a tty?".to_string());
            }
            let mut raw = original;
            // cfmakeraw equivalent, spelled out because libc does not expose
            // cfmakeraw on every platform this builds on.
            raw.c_iflag &= !(libc::BRKINT
                | libc::ICRNL
                | libc::INPCK
                | libc::ISTRIP
                | libc::IXON);
            raw.c_oflag &= !libc::OPOST;
            raw.c_cflag |= libc::CS8;
            raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG);
            raw.c_cc[libc::VMIN] = 1;
            raw.c_cc[libc::VTIME] = 0;
            if libc::tcsetattr(libc::STDOUT_FILENO, libc::TCSANOW, &raw) != 0 {
                return Err("cannot switch the terminal into raw mode".to_string());
            }
            Ok(Self { original })
        }
    }
}

#[cfg(unix)]
impl Drop for RawMode {
    fn drop(&mut self) {
        // SAFETY: `self.original` was read from the same file descriptor and is
        // still a valid, initialized termios.
        unsafe {
            libc::tcsetattr(libc::STDOUT_FILENO, libc::TCSANOW, &self.original);
        }
    }
}

#[cfg(not(unix))]
impl RawMode {
    /// Raw mode is a POSIX concept; Windows keeps its console untouched.
    pub fn enable() -> Result<Self, String> {
        Ok(Self)
    }
}

/// Asks the kernel for the terminal size without reading stdin.
///
/// A CPR or window-size *query* would have to read the reply off stdin, and
/// that swallows whatever the user has already typed. `TIOCGWINSZ` is a pure
/// ioctl, so input is left alone.
pub fn terminal_size() -> (usize, usize) {
    #[cfg(unix)]
    {
        // SAFETY: `winsize` is a plain struct written by the ioctl on success.
        unsafe {
            let mut size: libc::winsize = std::mem::zeroed();
            if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) == 0
                && size.ws_col > 2
                && size.ws_row > 2
            {
                return (size.ws_row as usize, size.ws_col as usize);
            }
        }
    }
    fallback_size()
}

/// Reads the terminal size from the environment, as a fallback.
fn fallback_size() -> (usize, usize) {
    let read = |key: &str| {
        std::env::var(key)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 2)
    };
    (
        read("LINES").unwrap_or(24),
        read("COLUMNS").unwrap_or(80),
    )
}

/// Decodes one key from terminal input bytes.
pub fn decode_key(bytes: &[u8]) -> Option<(Key, usize)> {
    let first = *bytes.first()?;
    match first {
        b'\r' | b'\n' => Some((Key::Enter, 1)),
        b'\t' => Some((Key::Tab, 1)),
        0x7f | 0x08 => Some((Key::Backspace, 1)),
        0x1b => match bytes.get(1) {
            None => Some((Key::Escape, 1)),
            Some(b'[') => match bytes.get(2) {
                Some(b'3') => Some((Key::Delete, 3)),
                Some(b'A') => Some((Key::Char('↑'), 3)),
                Some(b'B') => Some((Key::Char('↓'), 3)),
                _ => Some((Key::Escape, 1)),
            },
            _ => Some((Key::Escape, 1)),
        },
        _ => {
            // Decode one UTF-8 scalar so Persian input arrives intact.
            let len = utf8_len(first);
            let slice = bytes.get(..len.min(bytes.len()))?;
            let text = std::str::from_utf8(slice).ok()?;
            let ch = text.chars().next()?;
            Some((Key::Char(ch), len))
        }
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

/// Runs the terminal UI until the user presses `q` or Ctrl-C.
pub fn run_tui(program: &Program, ui: &UiDecl) -> Result<(), String> {
    let mut app = AecApp::new(program.clone(), ui.clone())?;
    let _raw = RawMode::enable()?;

    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(b"\x1b[?1049h");
    let _ = stdout.flush();

    let (rows, cols) = terminal_size();
    let result = event_loop(&mut app, rows, cols);
    let _ = stdout.write_all(b"\x1b[0m\x1b[?1049l");
    let _ = stdout.flush();
    result
}

fn event_loop(app: &mut AecApp, mut rows: usize, mut cols: usize) -> Result<(), String> {
    let mut out = std::io::stdout();
    let mut stdin = std::io::stdin();

    loop {
        let grid = tui::render_grid(app, cols, rows);
        let _ = std::io::Write::write_all(&mut out, tui::grid_to_ansi(&grid).as_bytes());
        let _ = std::io::Write::flush(&mut out);

        // A blocking read would freeze the app on a resize, so wait with a
        // timeout and repaint only when the size actually changed.
        if !wait_for_input(std::time::Duration::from_millis(100)) {
            let (current_rows, current_cols) = terminal_size();
            if current_rows != rows || current_cols != cols {
                rows = current_rows;
                cols = current_cols;
            }
            continue;
        }

        let mut buffer = [0u8; 64];
        let read = match stdin.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(count) => count,
            Err(_) => return Ok(()),
        };
        let mut offset = 0usize;
        while offset < read {
            let Some((key, used)) = decode_key(&buffer[offset..read]) else {
                break;
            };
            offset += used;
            if matches!(key, Key::Char('\u{3}')) {
                return Ok(());
            }
            if matches!(key, Key::Char('q')) && tui::focused_input().is_none() {
                return Ok(());
            }
            tui::handle_key(app, key);
        }
    }
}

/// Waits for input to become readable.
fn wait_for_input(timeout: Duration) -> bool {
    #[cfg(unix)]
    {
        let mut set: libc::fd_set = unsafe { std::mem::zeroed() };
        unsafe {
            libc::FD_ZERO(&mut set);
            libc::FD_SET(libc::STDIN_FILENO, &mut set);
            let mut timeval = libc::timeval {
                tv_sec: timeout.as_secs() as libc::time_t,
                tv_usec: timeout.subsec_micros() as libc::suseconds_t,
            };
            libc::select(
                libc::STDIN_FILENO + 1,
                &mut set,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut timeval,
            ) > 0
        }
    }
    #[cfg(not(unix))]
    {
        let _ = timeout;
        false
    }
}
