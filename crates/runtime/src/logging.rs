use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use tracing_subscriber::fmt::MakeWriter;

/// How large the log may grow before the next start begins a new one.
const LOG_LIMIT: u64 = 1024 * 1024;

/// Sends the log to the platform's log file; without one, nothing is logged.
pub fn init_logging() {
    let Some(path) = kanaemi_config::log_file() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    rotate(&path, LOG_LIMIT);
    if open_log(&path).is_err() {
        return;
    }
    // Debug logs record every key typed, so only a development build writes
    // them; a release build compiles them out (release_max_level_info).
    let level = if cfg!(debug_assertions) {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    tracing_subscriber::fmt()
        .with_writer(LogFile(path))
        .with_ansi(false)
        .with_max_level(level)
        .try_init()
        .ok();
}

/// The log, opened at its path for each line rather than held open: another
/// process may begin a new log at any start, and a file held open would go on
/// growing as the old log, out of sight of the settings app.
struct LogFile(PathBuf);

impl<'a> MakeWriter<'a> for LogFile {
    type Writer = Box<dyn Write>;

    fn make_writer(&'a self) -> Self::Writer {
        match open_log(&self.0) {
            Ok(file) => Box::new(file),
            Err(_) => Box::new(io::sink()),
        }
    }
}

fn open_log(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

/// Keeps a log grown past `limit` bytes as the old log, in place of the one
/// kept before, so that a new one is begun. A process that finds it too large
/// at the same moment as another may move the new log instead, losing the old
/// one; locking every start of every process against that once-in-a-while
/// loss of an old log is not worth it.
fn rotate(log: &Path, limit: u64) {
    if std::fs::metadata(log).is_ok_and(|meta| meta.len() > limit) {
        let _ = std::fs::rename(log, old_log(log));
    }
}

/// The log kept from before the current one began.
fn old_log(log: &Path) -> PathBuf {
    let mut name = log.file_name().unwrap_or_default().to_owned();
    name.push(".1");
    log.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;

    use tracing_subscriber::fmt::MakeWriter;

    use super::*;

    fn temp_log(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kanaemi-runtime-logging-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("kanaemi.log")
    }

    #[test]
    fn a_log_grown_past_the_limit_is_kept_as_the_old_one_and_started_anew() {
        let log = temp_log("past");
        fs::write(&log, "a".repeat(11)).unwrap();

        rotate(&log, 10);

        assert!(!log.exists());
        assert_eq!(fs::read_to_string(old_log(&log)).unwrap(), "a".repeat(11));
    }

    #[test]
    fn a_log_within_the_limit_is_left_to_grow() {
        let log = temp_log("within");
        fs::write(&log, "a".repeat(10)).unwrap();

        rotate(&log, 10);

        assert_eq!(fs::read_to_string(&log).unwrap(), "a".repeat(10));
        assert!(!old_log(&log).exists());
    }

    #[test]
    fn only_the_last_old_log_is_kept() {
        let log = temp_log("older");
        fs::write(old_log(&log), "older").unwrap();
        fs::write(&log, "a".repeat(11)).unwrap();

        rotate(&log, 10);

        assert_eq!(fs::read_to_string(old_log(&log)).unwrap(), "a".repeat(11));
    }

    #[test]
    fn a_missing_log_is_nothing_to_rotate() {
        let log = temp_log("missing");

        rotate(&log, 10);

        assert!(!log.exists() && !old_log(&log).exists());
    }

    #[test]
    fn a_line_written_after_another_process_began_a_new_log_goes_to_the_new_one() {
        let log = temp_log("writer");
        let file = LogFile(log.clone());
        file.make_writer().write_all(b"before\n").unwrap();
        fs::write(&log, "a".repeat(11)).unwrap();

        rotate(&log, 10);
        file.make_writer().write_all(b"after\n").unwrap();

        assert_eq!(fs::read_to_string(&log).unwrap(), "after\n");
    }

    #[test]
    fn the_old_log_sits_beside_the_log() {
        let log = temp_log("name");
        assert_eq!(old_log(&log), log.with_file_name("kanaemi.log.1"));
    }
}
