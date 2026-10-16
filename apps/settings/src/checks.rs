//! Lines of a text dictionary the IME skips, to show where a hand edit went
//! wrong.

use std::fs;
use std::path::Path;

use kanaemi_engine::{InvalidLine, InvalidReason, TextDictionary};

use crate::cache::FileCache;

/// How many invalid lines are named; the rest are only counted.
const NAMED: usize = 3;

/// What to say about the invalid lines of a text dictionary, or `None` when
/// every line is read.
pub fn invalid_lines(path: &Path, user_custom: bool) -> Option<String> {
    thread_local! {
        static KNOWN: FileCache<Option<String>> = FileCache::default();
    }
    KNOWN.with(|known| known.get(&[path.to_owned()], || read_invalid(path, user_custom)))
}

/// Why the IME cannot read or write the file with typed words at `path`, or
/// `None` when it can. A missing file is fine: it is made when written.
pub fn unreachable(path: &Path) -> Option<String> {
    if let Some(folder) = path.parent()
        && !folder.is_dir()
    {
        return Some(format!(
            "フォルダ {} がありません。できるまで空として扱い、できたら書きます",
            folder.display()
        ));
    }
    let exists = match fs::File::open(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Some(format!("読めません：{error}")),
        Ok(_) => true,
    };
    // A rewrite makes a new file beside it and a lock may have to be made:
    // the folder must take new files, whatever the files already there allow.
    let mut probe = path.as_os_str().to_owned();
    probe.push(format!(".{}.check", std::process::id()));
    let writes = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map(|_| {
            let _ = fs::remove_file(&probe);
        });
    let writes = match writes {
        Ok(()) if exists => fs::OpenOptions::new().append(true).open(path).map(drop),
        writes => writes,
    };
    writes.err().map(|error| format!("書けません：{error}"))
}

fn read_invalid(path: &Path, user_custom: bool) -> Option<String> {
    let (_, invalid) = if user_custom {
        TextDictionary::read_user_custom(path).ok()?
    } else {
        TextDictionary::parse(fs::read(path).ok()?)
    };
    describe(&invalid)
}

fn describe(invalid: &[InvalidLine]) -> Option<String> {
    if invalid.is_empty() {
        return None;
    }
    let named: Vec<String> = invalid
        .iter()
        .take(NAMED)
        .map(|l| format!("{} 行目：{}", l.line, reason(l.reason)))
        .collect();
    let more = if invalid.len() > NAMED { "、…" } else { "" };
    Some(format!(
        "読めない行が {} 行あります（{}{more}）",
        invalid.len(),
        named.join("、")
    ))
}

fn reason(reason: InvalidReason) -> &'static str {
    match reason {
        InvalidReason::Encoding => "UTF-8 でない",
        InvalidReason::FieldCount => "列の数が違う",
        InvalidReason::Empty => "読みか表記が空",
        InvalidReason::Escape => "\\ の書き方が違う",
        InvalidReason::Okurigana => "送り仮名の書き方が違う",
        InvalidReason::Hide => "! の行はユーザー辞書だけで使える",
        InvalidReason::ConjugationType => "知らない活用型",
        InvalidReason::Cost => "コストが 0 以上の整数でない",
        InvalidReason::Placeholder => "数の置き場所の書き方が違う",
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "kanaemi-settings-checks-{}-{name}",
            std::process::id()
        ))
    }

    #[test]
    fn a_dictionary_whose_lines_all_read_has_nothing_to_say() {
        let path = temp_path("fine.tsv");
        fs::write(&path, "# 説明\nきしゃ\t記者\n").unwrap();
        assert_eq!(invalid_lines(&path, false), None);
    }

    #[test]
    fn invalid_lines_are_counted_and_the_first_are_named() {
        let path = temp_path("broken.tsv");
        fs::write(
            &path,
            "きしゃ\n!きしゃ\t汽車\nきしゃ\t記者\t\t-1\nか\t書\t謎\na\tb\tc\td\te\tf\n",
        )
        .unwrap();
        assert_eq!(
            invalid_lines(&path, false).unwrap(),
            "読めない行が 5 行あります（1 行目：列の数が違う、2 行目：! の行はユーザー辞書だけで使える、3 行目：コストが 0 以上の整数でない、…）"
        );
    }

    #[test]
    fn a_line_that_is_not_utf_8_is_named_as_such() {
        let path = temp_path("encoding.tsv");
        fs::write(&path, b"\xa4\xad\xa4\xb7\xa4\xe3\t\xb5\xad\xbc\xd4\n").unwrap();

        let said = invalid_lines(&path, false);

        assert_eq!(
            said.as_deref(),
            Some("読めない行が 1 行あります（1 行目：UTF-8 でない）")
        );
    }

    #[test]
    fn a_file_with_typed_words_that_can_be_reached_has_nothing_to_say() {
        let dir = temp_path("reached");
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(
            unreachable(&dir.join("missing.tsv")),
            None,
            "made when written"
        );
        fs::write(dir.join("there.tsv"), "").unwrap();
        assert_eq!(unreachable(&dir.join("there.tsv")), None);
    }

    #[test]
    fn a_file_with_typed_words_in_a_folder_that_is_missing_says_so() {
        let folder = temp_path("missing-folder").join("sync");
        let said = unreachable(&folder.join("custom.tsv")).unwrap();
        assert!(
            said.starts_with(&format!("フォルダ {} がありません", folder.display())),
            "{said}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_file_with_typed_words_that_cannot_be_written_says_so() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_path("read-only");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("custom.tsv");
        fs::write(&path, "").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        let said = unreachable(&path);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(said.unwrap().starts_with("書けません："));
    }

    #[cfg(unix)]
    #[test]
    fn a_file_with_typed_words_in_a_folder_that_cannot_be_written_says_so() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_path("read-only-folder");
        fs::create_dir_all(&dir).unwrap();
        // The file and its lock are already there, writable themselves.
        fs::write(dir.join("written.tsv"), "").unwrap();
        fs::write(dir.join("written.tsv.lock"), "").unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).unwrap();
        let said = ["selections.tsv", "written.tsv"].map(|name| unreachable(&dir.join(name)));
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        for said in said {
            assert!(said.unwrap().starts_with("書けません："));
        }
        let left: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left.len(), 2, "nothing left behind: {left:?}");
    }

    #[test]
    fn hide_lines_are_fine_in_the_user_custom_dictionary() {
        let path = temp_path("custom.tsv");
        fs::write(&path, "!きしゃ\t汽車\n").unwrap();
        assert_eq!(invalid_lines(&path, true), None);
    }
}
