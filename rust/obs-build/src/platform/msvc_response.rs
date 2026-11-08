//! The object list and options of an MSVC link, passed in response files, and their
//! private regression check.
//!
//! `cl.exe` takes the libobs objects (one per unity group, and one
//! precompiled-header object per group) on its command line. Windows caps a
//! command line at 32767 characters, and with a CI checkout's long paths
//! the two paths per group passed it ("The filename or extension is too
//! long", os error 206). `cl.exe` and `link.exe` read `@file` arguments in place, so
//! the objects and final linker options go into files named by the command line.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Append the final linker options, keeping MSVC invocations short.
pub fn configure_link_options(
    target: &str,
    command: &mut Command,
    artifact: &Path,
    options: &[String],
    msvc: bool,
) -> Result<(), String> {
    if target.contains("windows") && msvc && !options.is_empty() {
        let rsp = artifact.with_extension("link.rsp");
        let mut arg = OsString::from("@");
        arg.push(&rsp);
        let existing = command.get_args().any(|value| value == arg);
        let mut text = if existing {
            std::fs::read_to_string(&rsp).map_err(|e| format!("read {}: {e}", rsp.display()))?
        } else {
            String::new()
        };
        let direct = is_direct_linker(command);

        for option in options {
            if option.contains(['\r', '\n', '\0']) {
                return Err(format!("invalid MSVC response-file option: {option:?}"));
            }
            // MSVC treats backslashes before quotes (including the closing
            // quote) as escapes. Preserve embedded quotes and trailing slashes.
            // CL forwards only the rest of this line after /link. Repeat the
            // switch for each option; a bare /link line would forward nothing.
            if !direct {
                text.push_str("/link ");
            }
            text.push('"');
            let mut slashes = 0;
            for ch in option.chars() {
                if ch == '\\' {
                    slashes += 1;
                    continue;
                }
                let count = if ch == '"' { slashes * 2 + 1 } else { slashes };
                text.extend(std::iter::repeat_n('\\', count));
                text.push(ch);
                slashes = 0;
            }
            text.extend(std::iter::repeat_n('\\', slashes * 2));
            text.push_str("\"\n");
        }
        std::fs::write(&rsp, text).map_err(|e| format!("write {}: {e}", rsp.display()))?;
        if !existing {
            command.arg(arg);
        }
    } else {
        command.args(options);
    }
    Ok(())
}

/// Configure the MSVC driver or direct linker before appending final options.
pub(super) fn configure_msvc_link(
    command: &mut Command,
    artifact: &Path,
    objects: &[PathBuf],
    dependencies: &[PathBuf],
    shared: bool,
) -> Result<(), String> {
    if shared {
        command.arg(if is_direct_linker(command) {
            "/DLL"
        } else {
            "/LD"
        });
    }
    command.arg(msvc_response_file(
        &artifact.with_extension("objects.rsp"),
        objects,
    )?);
    command.args(dependencies);
    let mut options = vec![format!("/OUT:{}", artifact.display())];
    if shared {
        options.push(format!(
            "/IMPLIB:{}",
            artifact.with_extension("lib").display()
        ));
    }
    configure_link_options("windows", command, artifact, &options, true)
}

/// Recognize the linker itself, including Windows paths when testing on Unix.
pub(super) fn is_direct_linker(command: &Command) -> bool {
    let program = command.get_program().to_string_lossy();
    let name = program.rsplit(['/', '\\']).next().unwrap_or(&program);
    name.eq_ignore_ascii_case("link.exe") || name.eq_ignore_ascii_case("link")
}

/// Writes `objects` to `rsp`, one double-quoted path per line, and returns
/// the `@rsp` argument that stands for them on the command line.
pub fn msvc_response_file(rsp: &Path, objects: &[PathBuf]) -> Result<OsString, String> {
    let mut text = String::new();
    for object in objects {
        let path = object
            .to_str()
            .ok_or_else(|| format!("object path is not UTF-8: {}", object.display()))?;
        if path.contains('"') {
            return Err(format!("object path has a quote: {path}"));
        }
        text.push('"');
        text.push_str(path);
        text.push_str("\"\n");
    }
    std::fs::write(rsp, text).map_err(|e| format!("write {}: {e}", rsp.display()))?;
    let mut arg = OsString::from("@");
    arg.push(rsp);
    Ok(arg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_options_preserve_msvc_quoting_and_other_toolchains() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("obs.dll");
        let options = vec![
            r"/WHOLEARCHIVE:C:\Program Files\obs.lib".into(),
            "trailing\\".into(),
            "embedded\"quote".into(),
            String::new(),
        ];
        let mut command = Command::new("cl.exe");
        configure_link_options(
            "x86_64-pc-windows-msvc",
            &mut command,
            &artifact,
            &options,
            true,
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(artifact.with_extension("link.rsp")).unwrap(),
            "/link \"/WHOLEARCHIVE:C:\\Program Files\\obs.lib\"\n/link \"trailing\\\\\"\n/link \"embedded\\\"quote\"\n/link \"\"\n"
        );
        for (target, msvc) in [
            ("x86_64-unknown-linux-gnu", false),
            ("x86_64-pc-windows-gnu", false),
        ] {
            let mut command = Command::new("cc");
            configure_link_options(target, &mut command, &artifact, &options, msvc).unwrap();
            assert_eq!(
                command.get_args().collect::<Vec<_>>(),
                options.iter().map(std::ffi::OsStr::new).collect::<Vec<_>>()
            );
        }
        let mut command = Command::new("cl.exe");
        configure_link_options("x86_64-pc-windows-msvc", &mut command, &artifact, &[], true)
            .unwrap();
        assert_eq!(command.get_args().count(), 0);
        for option in ["bad\noption", "bad\roption", "bad\0option"] {
            assert!(
                configure_link_options(
                    "x86_64-pc-windows-msvc",
                    &mut command,
                    &artifact,
                    &[option.into()],
                    true
                )
                .is_err()
            );
        }
        let missing = dir.path().join("missing").join("obs.dll");
        assert!(
            configure_link_options(
                "x86_64-pc-windows-msvc",
                &mut command,
                &missing,
                &options,
                true
            )
            .unwrap_err()
            .contains("write")
        );
    }

    #[test]
    fn msvc_export_options_use_a_short_response_file_invocation() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("libobs with spaces.dll");
        let exports: Vec<String> = (0..3000)
            .map(|i| format!("/EXPORT:obs_synthetic_export_{i:04}"))
            .collect();
        assert!(exports.iter().map(String::len).sum::<usize>() > 32767);
        for program in ["cl.exe", r"C:\VS tools\LINK.EXE"] {
            let mut command = Command::new(program);
            configure_msvc_link(&mut command, &artifact, &objects(dir.path()), &[], true).unwrap();
            configure_link_options(
                "x86_64-pc-windows-msvc",
                &mut command,
                &artifact,
                &exports,
                true,
            )
            .unwrap();
            assert!(format!("{command:?}").len() < 8000);
            let args: Vec<_> = command.get_args().collect();
            let direct = program != "cl.exe";
            assert_eq!(
                args,
                vec![
                    std::ffi::OsStr::new(if direct { "/DLL" } else { "/LD" }),
                    &OsString::from(format!(
                        "@{}",
                        artifact.with_extension("objects.rsp").display()
                    )),
                    &OsString::from(format!(
                        "@{}",
                        artifact.with_extension("link.rsp").display()
                    )),
                ]
            );
            let text = std::fs::read_to_string(artifact.with_extension("link.rsp")).unwrap();
            let prefix = if direct { "" } else { "/link " };
            let expected: Vec<_> = [
                format!("/OUT:{}", artifact.display()),
                format!("/IMPLIB:{}", artifact.with_extension("lib").display()),
            ]
            .into_iter()
            .chain(exports.iter().cloned())
            .map(|arg| format!("{prefix}\"{arg}\""))
            .collect();
            assert_eq!(text.lines().collect::<Vec<_>>(), expected);

            let mut executable = Command::new(program);
            configure_msvc_link(&mut executable, &artifact, &objects(dir.path()), &[], false)
                .unwrap();
            configure_link_options(
                "x86_64-pc-windows-msvc",
                &mut executable,
                &artifact,
                &exports,
                true,
            )
            .unwrap();
            assert_eq!(executable.get_args().collect::<Vec<_>>(), args[1..]);
            let text = std::fs::read_to_string(artifact.with_extension("link.rsp")).unwrap();
            let expected: Vec<_> = expected
                .into_iter()
                .enumerate()
                .filter_map(|(i, line)| (i != 1).then_some(line))
                .collect();
            assert_eq!(text.lines().collect::<Vec<_>>(), expected);
        }
    }

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
