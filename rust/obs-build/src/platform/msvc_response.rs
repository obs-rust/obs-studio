//! The object list of an MSVC link, passed in a response file, and its
//! private regression check.
//!
//! `cl.exe` takes the libobs objects (one per unity group, and one
//! precompiled-header object per group) on its command line. Windows caps a
//! command line at 32767 characters, and with a CI checkout's long paths
//! the two paths per group passed it ("The filename or extension is too
//! long", os error 206). `cl.exe` and `link.exe` read `@file` arguments in place, so
//! the objects go into one file and the command line names only it.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Writes `objects` to `rsp`, one double-quoted path per line, and returns
/// the `@rsp` argument that stands for them on the command line.
#[expect(clippy::todo, reason = "RED: the fix follows")]
pub fn msvc_response_file(rsp: &Path, objects: &[PathBuf]) -> Result<OsString, String> {
    let _ = (rsp, objects);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path with spaces, as a checkout under "Program Files" has.
    fn objects(dir: &Path) -> Vec<PathBuf> {
        vec![
            dir.join("libobs.core+.h.o"),
            dir.join("pch dir").join("prefix.obj"),
        ]
    }

    #[test]
    fn msvc_response_file_lists_each_object_quoted() {
        let dir = tempfile::tempdir().unwrap();
        let rsp = dir.path().join("obs.objects.rsp");
        let objects = objects(dir.path());
        let arg = msvc_response_file(&rsp, &objects).unwrap();
        let mut want = OsString::from("@");
        want.push(&rsp);
        assert_eq!(arg, want);
        let text = std::fs::read_to_string(&rsp).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines,
            objects
                .iter()
                .map(|o| format!("\"{}\"", o.display()))
                .collect::<Vec<_>>()
        );
    }

    /// The command line stays short however many objects there are.
    #[test]
    fn msvc_response_file_keeps_the_argument_short() {
        let dir = tempfile::tempdir().unwrap();
        let rsp = dir.path().join("obs.objects.rsp");
        let long = dir.path().join("x".repeat(200));
        let objects: Vec<PathBuf> = (0..400)
            .map(|i| long.join(format!("group{i}.obj")))
            .collect();
        let total: usize = objects.iter().map(|o| o.as_os_str().len()).sum();
        assert!(total > 32767, "the fixture must exceed the Windows limit");
        let arg = msvc_response_file(&rsp, &objects).unwrap();
        assert!(arg.len() < 1024, "{arg:?}");
        assert_eq!(std::fs::read_to_string(&rsp).unwrap().lines().count(), 400);
    }

    /// A quote cannot be escaped in a response file path, so it is refused.
    #[test]
    fn msvc_response_file_refuses_a_quote() {
        let dir = tempfile::tempdir().unwrap();
        let rsp = dir.path().join("obs.objects.rsp");
        let bad = [dir.path().join("a\"b.obj")];
        assert!(msvc_response_file(&rsp, &bad).is_err());
    }
}
