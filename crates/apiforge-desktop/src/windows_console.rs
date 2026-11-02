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
