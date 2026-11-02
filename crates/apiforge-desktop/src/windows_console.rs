//! Where the program's output goes on Windows (REL-04, #205).
//!
//! A release build on Windows is a windowed program: it has no console of its
//! own. Standard output and standard error that the parent redirected (a pipe,
//! a file, a variable) are used as they are. Otherwise the program attaches to
//! the console of the terminal that started it, and when there is none (a
//! start from Explorer, the Start menu or the release ZIP) it appends both to
//! `apiforge-desktop.log` beside the managed projects. Every `eprintln!` and
//! Rust's own panic message then land there unchanged, and a panic on the
//! window's thread also shows a dialog that names the log. Anywhere else, and
//! whenever any of this fails, nothing changes: the window starts regardless.

#[cfg(any(windows, test))]
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

/// A log longer than this when the program starts is renamed to
/// `apiforge-desktop.log.1` first, so there is never much more than twice it.
#[cfg(any(windows, test))]
const LOG_LIMIT: u64 = 1024 * 1024;

/// Gives the program somewhere to write before anything is written. The first
/// statement of `main`.
pub fn prepare() {
    #[cfg(windows)]
    native::prepare();
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

/// Opens the log and makes it the program's standard output and error.
#[cfg(windows)]
pub fn log_to(path: &Path) -> io::Result<()> {
    native::redirect(open_log(path)?)
}

/// After the usual panic message, which goes wherever standard error goes, a
/// panic on the thread that calls this (the window's) also calls `show` with
/// the dialog's text. A panic on another thread is reported by the message
/// alone: the window survives it and says so itself.
#[cfg(windows)]
pub fn report_crashes(log: PathBuf, show: impl Fn(&str) + Send + Sync + 'static) {
    let window = std::thread::current().id();
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        previous(info);
        if std::thread::current().id() == window {
            show(&dialog_text(&log));
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

/// The two calls the standard library does not offer. The workspace forbids
/// `unsafe` code; this crate denies it and allows it here only.
#[cfg(windows)]
mod native {
    #![allow(unsafe_code)]

    use std::{
        fs, io,
        os::windows::io::{AsRawHandle, IntoRawHandle, RawHandle},
    };
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::Console::{
            ATTACH_PARENT_PROCESS, AttachConsole, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle,
        },
    };

    pub(super) fn prepare() {
        // The parent gave the program its output: a pipe, a file, a variable.
        if present(io::stdout().as_raw_handle()) || present(io::stderr().as_raw_handle()) {
            return;
        }
        // SAFETY: AttachConsole takes a process identifier by value and
        // touches no memory of this program. On success Windows sets the
        // standard handles to the console's, which the standard library reads
        // on every write.
        if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } != 0 {
            return;
        }
        let Some(data) = dirs::data_dir() else {
            return;
        };
        let log = super::log_path(&data);
        if super::log_to(&log).is_ok() {
            super::report_crashes(log, super::show_dialog);
        }
    }

    fn present(handle: RawHandle) -> bool {
        !handle.is_null() && handle != INVALID_HANDLE_VALUE
    }

    /// Makes `file` the standard output and error for the rest of the
    /// process; its handle is never closed.
    pub(super) fn redirect(file: fs::File) -> io::Result<()> {
        let handle = file.into_raw_handle();
        for which in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            // SAFETY: `handle` is an open file handle that this program owns
            // and never closes, so it stays valid for every later write
            // through the standard handles.
            if unsafe { SetStdHandle(which, handle) } == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }
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
