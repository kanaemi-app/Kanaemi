//! The functions the user writes in the settings folder, read again when a
//! file of theirs changes.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use kanaemi_config::{BUILTIN_PREFIX, FUNCTIONS_DIR};
use kanaemi_functions::{EXTENSION, FunctionError, LuauFunctions, Without};

use crate::dictionaries::{FileStamp, file_stamp};
use crate::masked;

/// Every file of the functions and their modules, with its stamp.
pub(crate) type Stamp = Vec<(PathBuf, FileStamp)>;

pub(crate) fn stamp(support_dir: &Path) -> Stamp {
    let mut files = Vec::new();
    collect(
        &support_dir.join(FUNCTIONS_DIR),
        &mut HashSet::new(),
        &mut files,
    );
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let stamp = file_stamp(&path);
            (path, stamp)
        })
        .collect()
}

/// The module files in `dir` at any depth, as `require` reaches them. A
/// folder already `seen` is not entered again, so a symlink that loops ends.
fn collect(dir: &Path, seen: &mut HashSet<PathBuf>, files: &mut Vec<PathBuf>) {
    let Ok(real) = dir.canonicalize() else {
        return;
    };
    if !seen.insert(real) {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.is_dir() {
            collect(&path, seen, files);
        } else if path.is_file() && path.extension().is_some_and(|e| e == EXTENSION) {
            files.push(path);
        }
    }
}

/// The functions in the settings folder but those `disabled` names, as the
/// settings write them, with what was wrong in them logged.
pub(crate) fn open(support_dir: &Path, disabled: &[String]) -> Rc<LuauFunctions> {
    let mut without = Without::default();
    for name in disabled {
        match name.strip_prefix(BUILTIN_PREFIX) {
            Some(builtin) => without.builtins.push(builtin.to_owned()),
            None => without.files.push(name.clone()),
        }
    }
    let functions = LuauFunctions::open_without(support_dir.join(FUNCTIONS_DIR), &without);
    let count = functions.names().count();
    if count > 0 {
        tracing::info!(count, "functions loaded");
    }
    log(&functions);
    Rc::new(functions)
}

/// Logs what the functions printed and how they failed since the last call.
pub(crate) fn log(functions: &LuauFunctions) {
    for printed in functions.take_printed() {
        tracing::info!(
            function = printed.function,
            text = printed.text,
            "function printed"
        );
    }
    for error in functions.take_errors() {
        match error {
            FunctionError::Failed { name, message } => tracing::warn!(
                function = name,
                error = %masked_failure(&message),
                "function skipped"
            ),
            error => tracing::warn!(%error, "function skipped"),
        }
    }
}

/// What a function failed with, its own words masked whole: they may say
/// what the function was given, which the user typed. Where it failed and
/// the calls that led there stay.
fn masked_failure(message: &str) -> String {
    let (said, trace) = message
        .find("\nstack traceback:")
        .map_or((message, ""), |at| message.split_at(at));
    // Luau puts where it failed first, as `file.luau:line: `.
    let words = said
        .match_indices(": ")
        .map(|(at, _)| at + 2)
        .find(|&at| {
            said[..at - 2].rsplit_once(':').is_some_and(|(file, line)| {
                file.ends_with(EXTENSION) && line.bytes().all(|b| b.is_ascii_digit())
            })
        })
        .unwrap_or(0);
    format!("{}{}{trace}", &said[..words], masked(&said[words..]))
}

#[cfg(test)]
mod tests {
    use kanaemi_engine::{Call, Functions};

    use super::*;
    use crate::captured::logged;

    #[test]
    fn what_a_function_failed_on_is_logged_masked() {
        let dir = std::env::temp_dir().join(format!(
            "kanaemi-runtime-functions-{}-error",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(FUNCTIONS_DIR)).unwrap();
        std::fs::write(
            dir.join(FUNCTIONS_DIR).join("leak.luau"),
            "return function(source) error(\"no \" .. source) end",
        )
        .unwrap();
        let functions = open(&dir, &[]);
        functions.call(&Call {
            name: "leak",
            source: "secret\u{3088}\u{307f}",
            argument: None,
        });

        let log = logged(|| log(&functions));

        assert!(log.contains("function skipped"), "{log}");
        assert!(
            log.contains("leak.luau:1: aa "),
            "where it failed stays: {log}"
        );
        assert!(!log.contains("secret"), "{log}");
        assert!(!log.contains('\u{3088}'), "{log}");
    }
}
