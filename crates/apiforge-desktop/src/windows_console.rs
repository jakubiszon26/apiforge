//! Where the program's output goes (REL-04, #205).
//!
//! On Windows a release build is a windowed program with no console of its
//! own. Everything the program prints goes through [`write`]: to standard
//! output or standard error when the parent gave the program that handle (a
//! console of a debug build, a pipe, a file, a variable), and otherwise, on
//! Windows, to `apiforge-desktop.log` beside the managed projects. The program
//! never attaches to a console (decision 2 = B) and uses no `unsafe` code: the
//! only question asked of Windows is the standard library's own
//! `AsRawHandle`, which is null when the parent gave no handle. A panic is
//! written to the same log, and a panic on the window's thread also shows a
//! dialog that names the log. Anywhere else, and whenever the log cannot be
//! opened, the program behaves as before: the window starts regardless.

use std::{
    fmt,
    io::{self, Write},
};
#[cfg(any(windows, test))]
use std::{
    fs,
    path::{Path, PathBuf},
};

/// A log longer than this when it is first opened is renamed to
/// `apiforge-desktop.log.1`, so there is never much more than twice it.
#[cfg(any(windows, test))]
const LOG_LIMIT: u64 = 1024 * 1024;

/// The stream a line belongs to when the parent gave one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Out,
    Err,
}

/// Prints one line, as `println!` or `eprintln!` would, or appends it to the
/// log when that stream has no handle. A failed write is ignored: there is
/// nowhere left to report it.
pub fn write(stream: Stream, line: fmt::Arguments<'_>) {
    #[cfg(windows)]
    if let Some(output) = OUTPUT.get()
        && !(output.present)(stream)
    {
        output.append(line);
        return;
    }
    let _ = match stream {
        Stream::Out => writeln!(io::stdout().lock(), "{line}"),
        Stream::Err => writeln!(io::stderr().lock(), "{line}"),
    };
}

/// Sets up the log and the crash report on Windows before anything is
/// printed; the first statement of `main`. Elsewhere it does nothing.
pub fn prepare() {
    #[cfg(windows)]
    if let Some(data) = dirs::data_dir() {
        install(log_path(&data), given, show_dialog);
    }
}

/// `apiforge-desktop.log` in `logs/` of the application data directory, beside
/// `projects/`.
#[cfg(any(windows, test))]
fn log_path(data: &Path) -> PathBuf {
    data.join("apiforge")
        .join("logs")
        .join("apiforge-desktop.log")
}

/// The line that begins each start's block in the log.
#[cfg(any(windows, test))]
fn start_line(version: &str, time: &str) -> String {
    format!("--- apiforge-desktop {version} {time} ---\n")
}

#[cfg(any(windows, test))]
fn rotates(length: u64) -> bool {
    length > LOG_LIMIT
}

/// What the dialog says when the window's thread panics.
#[cfg(any(windows, test))]
pub fn dialog_text(log: &Path) -> String {
    format!(
        "API Forge stopped unexpectedly. What happened is written to {}.",
        log.display()
    )
}

/// Opens the log for appending, after renaming one over the limit to `.1`,
/// and writes this start's line.
#[cfg(any(windows, test))]
fn open_log(path: &Path) -> io::Result<fs::File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if fs::metadata(path).is_ok_and(|metadata| rotates(metadata.len())) {
        // Replaces an older `.1`. When another window holds the file so that
        // it cannot be renamed, this start appends to it instead.
        let _ = fs::rename(path, path.with_file_name("apiforge-desktop.log.1"));
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %:z");
    file.write_all(start_line(env!("CARGO_PKG_VERSION"), &time.to_string()).as_bytes())?;
    Ok(file)
}

/// Where lines without a handle go, set once by [`install`].
#[cfg(windows)]
static OUTPUT: std::sync::OnceLock<Output> = std::sync::OnceLock::new();

#[cfg(windows)]
struct Output {
    log: PathBuf,
    /// Whether the parent gave the program this stream.
    present: fn(Stream) -> bool,
    /// Opened at the first line that needs it, so that a start which prints
    /// nothing leaves the log alone; `None` when it cannot be opened.
    file: std::sync::OnceLock<Option<std::sync::Mutex<fs::File>>>,
}

#[cfg(windows)]
impl Output {
    /// Appends one line; false when the log cannot be opened or written.
    fn append(&self, line: fmt::Arguments<'_>) -> bool {
        let file = self
            .file
            .get_or_init(|| open_log(&self.log).ok().map(std::sync::Mutex::new));
        let Some(file) = file else {
            return false;
        };
        let mut file = file.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        writeln!(file, "{line}").is_ok()
    }
}

/// Whether the parent gave the program this stream: the standard library
/// reports a stream it was not given as a null handle.
#[cfg(windows)]
fn given(stream: Stream) -> bool {
    use std::os::windows::io::AsRawHandle;
    let handle = match stream {
        Stream::Out => io::stdout().as_raw_handle(),
        Stream::Err => io::stderr().as_raw_handle(),
    };
    // Null, or INVALID_HANDLE_VALUE, which is -1.
    !handle.is_null() && handle as isize != -1
}

/// Sends lines without a handle to `log`, and reports panics there: after a
/// panic's text is appended, a panic on the thread that calls this (the
/// window's) also calls `show` with the dialog's text. A panic on another
/// thread is reported by its text alone: the window survives it and says so
/// itself. When the parent gave standard error, a panic is printed there as
/// before and nothing is shown.
#[cfg(windows)]
pub fn install(
    log: PathBuf,
    present: fn(Stream) -> bool,
    show: impl Fn(&str) + Send + Sync + 'static,
) {
    let output = OUTPUT.get_or_init(|| Output {
        log,
        present,
        file: std::sync::OnceLock::new(),
    });
    let window = std::thread::current().id();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if (output.present)(Stream::Err) {
            previous(info);
            return;
        }
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("<unnamed>");
        let backtrace = std::backtrace::Backtrace::capture();
        let written = if backtrace.status() == std::backtrace::BacktraceStatus::Captured {
            output.append(format_args!("thread '{name}' {info}\n{backtrace}"))
        } else {
            output.append(format_args!("thread '{name}' {info}"))
        };
        if written && thread.id() == window {
            show(&dialog_text(&output.log));
        }
    }));
}

/// The system's error dialog, with one OK button. It runs its own message
/// loop, so it works from inside a panic.
#[cfg(windows)]
fn show_dialog(text: &str) {
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("API Forge")
        .set_description(text)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

#[cfg(test)]
mod tests {
    //! REL-04 (#205), criterion 4: the parts of the log that do not depend on
    //! Windows, on both platforms.
    use super::*;
    use std::{fs, io::Write, path::Path};

    const MIB: usize = 1024 * 1024;

    fn start_prefix() -> String {
        format!("--- apiforge-desktop {} ", env!("CARGO_PKG_VERSION"))
    }

    #[test]
    fn the_log_sits_beside_the_managed_projects() {
        let data = Path::new("data");
        assert_eq!(
            log_path(data),
            data.join("apiforge")
                .join("logs")
                .join("apiforge-desktop.log")
        );
    }

    #[test]
    fn each_start_line_names_the_program_its_version_and_the_time() {
        assert_eq!(
            start_line("0.1.0", "2026-11-02 14:03:07 +01:00"),
            "--- apiforge-desktop 0.1.0 2026-11-02 14:03:07 +01:00 ---\n"
        );
    }

    #[test]
    fn only_a_log_over_one_mib_is_rotated() {
        assert!(!rotates(0));
        assert!(!rotates(MIB as u64));
        assert!(rotates(MIB as u64 + 1));
    }

    #[test]
    fn each_start_appends_its_own_block() {
        let dir = tempfile::tempdir().unwrap();
        let log = log_path(dir.path());
        writeln!(open_log(&log).unwrap(), "Valid v2 document: 4 project(s)").unwrap();
        writeln!(open_log(&log).unwrap(), "apiforge: channel: closed").unwrap();
        let text = fs::read_to_string(&log).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines.len(), 4, "{text}");
        for start in [lines[0], lines[2]] {
            assert!(start.starts_with(&start_prefix()), "{start}");
            assert!(start.ends_with(" ---"), "{start}");
        }
        assert_eq!(lines[1], "Valid v2 document: 4 project(s)");
        assert_eq!(lines[3], "apiforge: channel: closed");
        assert!(!log.with_file_name("apiforge-desktop.log.1").exists());
    }

    #[test]
    fn a_log_over_one_mib_becomes_the_previous_log() {
        let dir = tempfile::tempdir().unwrap();
        let log = log_path(dir.path());
        let previous = log.with_file_name("apiforge-desktop.log.1");
        fs::create_dir_all(log.parent().unwrap()).unwrap();
        fs::write(&log, vec![b'x'; MIB + 1]).unwrap();
        fs::write(&previous, "an older log").unwrap();
        drop(open_log(&log).unwrap());
        assert_eq!(fs::read(&previous).unwrap(), vec![b'x'; MIB + 1]);
        let text = fs::read_to_string(&log).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.starts_with(&start_prefix()), "{text}");
    }

    #[test]
    fn a_log_of_one_mib_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let log = log_path(dir.path());
        fs::create_dir_all(log.parent().unwrap()).unwrap();
        fs::write(&log, vec![b'x'; MIB]).unwrap();
        drop(open_log(&log).unwrap());
        let bytes = fs::read(&log).unwrap();
        assert_eq!(&bytes[..MIB], &vec![b'x'; MIB][..]);
        assert!(String::from_utf8_lossy(&bytes[MIB..]).starts_with(&start_prefix()));
        assert!(!log.with_file_name("apiforge-desktop.log.1").exists());
    }

    #[test]
    fn the_crash_dialog_names_the_log() {
        let log = Path::new(r"C:\Users\owner\AppData\Roaming\apiforge\logs\apiforge-desktop.log");
        assert_eq!(
            dialog_text(log),
            r"API Forge stopped unexpectedly. What happened is written to C:\Users\owner\AppData\Roaming\apiforge\logs\apiforge-desktop.log."
        );
    }
}
