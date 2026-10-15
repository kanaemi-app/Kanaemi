use std::any::Any;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};

use tracing_subscriber::fmt::MakeWriter;

use crate::masked_message;

/// How large a log may grow before a new one is begun.
const LOG_LIMIT: u64 = 4 * 1024 * 1024;

/// How many logs are kept from before the current one.
const OLD_LOGS: usize = 5;

/// How many lines may wait for the disk before more are dropped.
const QUEUE: usize = 4096;

/// Sends the log to the platform's log file; without one, nothing is logged.
/// A panic is logged too, with where it happened.
pub fn init_logging() {
    let Some(path) = kanaemi_config::log_file() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // A log that cannot be written is no reason to start a thread for it.
    if OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .is_err()
    {
        return;
    }
    let Ok(writer) = Writer::start(path, LOG_LIMIT, OLD_LOGS) else {
        return;
    };
    let home = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let writer = writer.without(std::env::var(home).ok());
    // A release build compiles out what is below INFO
    // (release_max_level_info); what is logged there is enough to find a
    // fault, with the text typed masked.
    let level = if cfg!(debug_assertions) {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    let installed = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_max_level(level)
        .try_init()
        .is_ok();
    if installed {
        log_panics();
    }
}

/// Logs each panic with its message, where it happened and the calls that
/// led there, before the panic goes on as it would have. The input method
/// catches a panic to keep running, which leaves nothing else of it.
fn log_panics() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!(
            message = %panic_message(info.payload()),
            location = %info.location().map_or_else(String::new, ToString::to_string),
            thread = std::thread::current().name().unwrap_or("unnamed"),
            "panicked\n{backtrace}"
        );
        previous(info);
    }));
}

/// Lines for the log, handed to a thread that writes them: the keys are
/// handled without waiting on the disk.
#[derive(Clone)]
struct Writer {
    lines: SyncSender<Vec<u8>>,
    /// Written as `~`: the folder's name is the user's.
    home: Option<String>,
}

impl Writer {
    fn start(path: PathBuf, limit: u64, keep: usize) -> io::Result<Self> {
        let (lines, received) = std::sync::mpsc::sync_channel(QUEUE);
        std::thread::Builder::new()
            .name("kanaemi-log".to_owned())
            .spawn(move || write_lines(received, &path, limit, keep))?;
        Ok(Self { lines, home: None })
    }

    fn without(self, home: Option<String>) -> Self {
        Self {
            home: home.filter(|home| !home.is_empty()),
            ..self
        }
    }
}

impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let mut entry = String::from_utf8_lossy(bytes).into_owned();
        if let Some(home) = &self.home {
            entry = entry.replace(home.as_str(), "~");
        }
        let entry = with_process(&entry, std::process::id());
        // A disk too slow to keep up loses lines rather than the keys.
        match self.lines.try_send(entry.into_bytes()) {
            Ok(()) | Err(TrySendError::Full(_)) => Ok(bytes.len()),
            Err(TrySendError::Disconnected(_)) => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Writer {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// `entry` told as written by the process `pid`: on Windows the input
/// method runs in each application, and they all write the one log.
fn with_process(entry: &str, pid: u32) -> String {
    match entry.strip_suffix('\n') {
        Some(body) => format!("{body} pid={pid}\n"),
        None => format!("{entry} pid={pid}"),
    }
}

fn write_lines(lines: Receiver<Vec<u8>>, path: &Path, limit: u64, keep: usize) {
    while let Ok(mut bytes) = lines.recv() {
        // Lines that came while the last were written go to the disk at once.
        while let Ok(more) = lines.try_recv() {
            bytes.extend_from_slice(&more);
        }
        let _ = append(path, &bytes, limit, keep);
    }
}

/// Adds `bytes` to the log at `path`, opened for this alone rather than held
/// open: another process may begin a new log at any time, and a file held
/// open would go on growing as an old log, out of sight of the settings app.
fn append(path: &Path, bytes: &[u8], limit: u64, keep: usize) -> io::Result<()> {
    rotate(path, limit, keep);
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(bytes)
}

/// Keeps a log grown past `limit` bytes as the newest of `keep` old logs,
/// so that a new one is begun. A process that finds it too large at the
/// same moment as another may move the new log instead, losing an old one;
/// locking every write of every process against that once-in-a-while loss
/// of an old log is not worth it.
fn rotate(log: &Path, limit: u64, keep: usize) {
    if !std::fs::metadata(log).is_ok_and(|meta| meta.len() > limit) {
        return;
    }
    let _ = std::fs::remove_file(old_log(log, keep));
    for nth in (1..keep).rev() {
        let _ = std::fs::rename(old_log(log, nth), old_log(log, nth + 1));
    }
    let _ = std::fs::rename(log, old_log(log, 1));
}

/// The `nth` log kept from before the current one, 1 the newest.
fn old_log(log: &Path, nth: usize) -> PathBuf {
    let mut name = log.file_name().unwrap_or_default().to_owned();
    name.push(format!(".{nth}"));
    log.with_file_name(name)
}

/// What a panic says, with what the user may have typed in it masked: a
/// panic on a string, such as one sliced in the middle of a character,
/// quotes the string.
fn panic_message(payload: &(dyn Any + Send)) -> String {
    let message = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str));
    message.map_or_else(|| "(no message)".to_owned(), masked_message)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

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
    fn a_log_grown_past_the_limit_is_kept_as_the_newest_old_one() {
        let log = temp_log("past");
        fs::write(&log, "a".repeat(11)).unwrap();

        rotate(&log, 10, 3);

        assert!(!log.exists());
        assert_eq!(
            fs::read_to_string(old_log(&log, 1)).unwrap(),
            "a".repeat(11)
        );
    }

    #[test]
    fn a_log_within_the_limit_is_left_to_grow() {
        let log = temp_log("within");
        fs::write(&log, "a".repeat(10)).unwrap();

        rotate(&log, 10, 3);

        assert_eq!(fs::read_to_string(&log).unwrap(), "a".repeat(10));
        assert!(!old_log(&log, 1).exists());
    }

    #[test]
    fn old_logs_move_back_and_the_oldest_beyond_those_kept_goes() {
        let log = temp_log("older");
        fs::write(old_log(&log, 1), "first").unwrap();
        fs::write(old_log(&log, 2), "second").unwrap();
        fs::write(&log, "a".repeat(11)).unwrap();

        rotate(&log, 10, 2);

        assert_eq!(
            fs::read_to_string(old_log(&log, 1)).unwrap(),
            "a".repeat(11)
        );
        assert_eq!(fs::read_to_string(old_log(&log, 2)).unwrap(), "first");
        assert!(!old_log(&log, 3).exists());
    }

    #[test]
    fn a_missing_log_is_nothing_to_rotate() {
        let log = temp_log("missing");

        rotate(&log, 10, 3);

        assert!(!log.exists() && !old_log(&log, 1).exists());
    }

    #[test]
    fn a_line_added_to_a_log_past_the_limit_begins_a_new_one() {
        let log = temp_log("append");
        fs::write(&log, "a".repeat(11)).unwrap();

        append(&log, b"line\n", 10, 3).unwrap();

        assert_eq!(fs::read_to_string(&log).unwrap(), "line\n");
        assert_eq!(
            fs::read_to_string(old_log(&log, 1)).unwrap(),
            "a".repeat(11)
        );
    }

    #[test]
    fn a_line_added_after_another_process_began_a_new_log_goes_to_the_new_one() {
        let log = temp_log("moved");
        append(&log, b"before\n", 100, 3).unwrap();
        fs::rename(&log, old_log(&log, 1)).unwrap();

        append(&log, b"after\n", 100, 3).unwrap();

        assert_eq!(fs::read_to_string(&log).unwrap(), "after\n");
    }

    #[test]
    fn lines_written_reach_the_log_in_order() {
        let log = temp_log("writer");
        let mut writer = Writer::start(log.clone(), 1000, 3).unwrap();

        writer.write_all(b"one\n").unwrap();
        writer.write_all(b"two\n").unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        let pid = std::process::id();
        let expected = format!("one pid={pid}\ntwo pid={pid}\n");
        while fs::read_to_string(&log).unwrap_or_default() != expected {
            assert!(Instant::now() < deadline, "lines never written");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn the_home_folder_is_written_as_a_tilde() {
        let log = temp_log("home");
        let mut writer = Writer::start(log.clone(), 1000, 3)
            .unwrap()
            .without(Some("/Users/someone".to_owned()));

        writer
            .write_all(b"path=/Users/someone/Library/kanaemi.log\n")
            .unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        let expected = format!("path=~/Library/kanaemi.log pid={}\n", std::process::id());
        while fs::read_to_string(&log).unwrap_or_default() != expected {
            assert!(Instant::now() < deadline, "line never written as expected");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn each_entry_tells_the_process_that_wrote_it() {
        assert_eq!(with_process("a line\n", 42), "a line pid=42\n");
        assert_eq!(
            with_process("panicked\n  0: frame\n", 42),
            "panicked\n  0: frame pid=42\n"
        );
        assert_eq!(with_process("no newline", 42), "no newline pid=42");
    }

    #[test]
    fn old_logs_sit_beside_the_log() {
        let log = temp_log("name");
        assert_eq!(old_log(&log, 1), log.with_file_name("kanaemi.log.1"));
        assert_eq!(old_log(&log, 2), log.with_file_name("kanaemi.log.2"));
    }

    #[test]
    fn a_panic_tells_its_message_with_the_typed_text_masked() {
        let quoted: Box<dyn Any + Send> = Box::new(String::from(
            "byte index 4 is not a char boundary of `›てい`",
        ));
        assert_eq!(
            panic_message(&*quoted),
            "byte index 4 is not a char boundary of `›ああ`"
        );
        let fixed: Box<dyn Any + Send> = Box::new("called `Option::unwrap()` on a `None` value");
        assert_eq!(
            panic_message(&*fixed),
            "called `Aaaaaa::aaaaaa()` aa a `Aaaa` aaaaa",
            "from the first quote on, the message may be typed, and is masked; the place tells the rest"
        );
        let other: Box<dyn Any + Send> = Box::new(42);
        assert_eq!(panic_message(&*other), "(no message)");
    }
}
