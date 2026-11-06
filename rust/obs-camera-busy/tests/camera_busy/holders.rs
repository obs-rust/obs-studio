//! Who holds a device: a scan of a fixture `/proc` tree (#3724).

use std::path::Path;

use obs_camera_busy::{Holder, display_name};

#[test]
fn name_is_the_exe_file_name() {
    assert_eq!(
        display_name(Some(Path::new("/opt/zoom/zoom")), "ZoomWebviewHost", b"x\0"),
        "zoom"
    );
}

#[test]
fn a_deleted_exe_keeps_its_name() {
    // An updated binary still running reads "/usr/bin/cheese (deleted)".
    assert_eq!(
        display_name(Some(Path::new("/usr/bin/cheese (deleted)")), "", b""),
        "cheese"
    );
}

#[test]
fn a_nix_wrapper_is_its_program() {
    assert_eq!(
        display_name(
            Some(Path::new("/nix/store/abc-zoom/bin/.zoom-wrapped")),
            "",
            b""
        ),
        "zoom"
    );
}

#[test]
fn without_exe_the_comm_then_the_cmdline() {
    assert_eq!(display_name(None, "guvcview\n", b"/usr/bin/x\0"), "guvcview");
    assert_eq!(
        display_name(None, "", b"/usr/bin/ffplay\0/dev/video0\0"),
        "ffplay"
    );
    assert_eq!(display_name(None, "", b""), "?");
}

#[cfg(unix)]
mod tree {
    use std::fs;
    use std::os::unix::fs::{MetadataExt, symlink};
    use std::path::{Path, PathBuf};

    use obs_camera_busy::{Holder, find_holders, is_alive};

    pub struct Tree(pub PathBuf);

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    pub fn tree(name: &str) -> Tree {
        let p = std::env::temp_dir().join(format!("obs-camera-busy-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Tree(p)
    }

    /// A process `pid` with descriptors on `targets`.
    pub fn process(root: &Path, pid: u32, comm: &str, exe: Option<&str>, targets: &[&str]) {
        let dir = root.join(pid.to_string());
        fs::create_dir_all(dir.join("fd")).unwrap();
        fs::write(dir.join("comm"), format!("{comm}\n")).unwrap();
        fs::write(dir.join("cmdline"), format!("/usr/bin/{comm}\0")).unwrap();
        fs::write(dir.join("stat"), format!("{pid} ({comm}) S 1 1")).unwrap();
        if let Some(exe) = exe {
            symlink(exe, dir.join("exe")).unwrap();
        }
        for (fd, target) in targets.iter().enumerate() {
            symlink(target, dir.join("fd").join((fd + 3).to_string())).unwrap();
        }
    }

    fn uid(root: &Path) -> u32 {
        fs::metadata(root).unwrap().uid()
    }

    fn holder(pid: u32, name: &str, can_close: bool) -> Holder {
        Holder {
            pid,
            name: name.into(),
            can_close,
        }
    }

    #[test]
    fn finds_every_holder_in_pid_order() {
        let t = tree("every");
        process(&t.0, 40, "cheese", None, &["/dev/null", "/dev/video0"]);
        process(&t.0, 12, "zoom", Some("/opt/zoom/zoom"), &["/dev/video0"]);
        process(&t.0, 7, "pipewire", None, &["/dev/video2", "socket:[1]"]);
        let me = uid(&t.0);
        assert_eq!(
            find_holders(&t.0, Path::new("/dev/video0"), 1, Some(me)),
            vec![holder(12, "zoom", true), holder(40, "cheese", true)]
        );
    }

    #[test]
    fn skips_obs_itself() {
        let t = tree("self");
        process(&t.0, 100, "obs", None, &["/dev/video0"]);
        process(&t.0, 12, "zoom", None, &["/dev/video0"]);
        let found = find_holders(&t.0, Path::new("/dev/video0"), 100, Some(uid(&t.0)));
        assert_eq!(found, vec![holder(12, "zoom", true)]);
    }

    #[test]
    fn another_users_process_cannot_be_closed() {
        let t = tree("other-user");
        process(&t.0, 12, "zoom", None, &["/dev/video0"]);
        let other = uid(&t.0).wrapping_add(1);
        assert_eq!(
            find_holders(&t.0, Path::new("/dev/video0"), 1, Some(other)),
            vec![holder(12, "zoom", false)]
        );
        assert_eq!(
            find_holders(&t.0, Path::new("/dev/video0"), 1, None),
            vec![holder(12, "zoom", false)]
        );
    }

    #[test]
    fn ignores_entries_that_are_not_processes() {
        let t = tree("not-processes");
        fs::create_dir_all(t.0.join("self/fd")).unwrap();
        symlink("/dev/video0", t.0.join("self/fd/3")).unwrap();
        fs::create_dir_all(t.0.join("0/fd")).unwrap();
        symlink("/dev/video0", t.0.join("0/fd/3")).unwrap();
        fs::write(t.0.join("12"), "a file, not a process").unwrap();
        // A process whose fd directory cannot be read (another user's).
        fs::create_dir_all(t.0.join("13")).unwrap();
        assert!(find_holders(&t.0, Path::new("/dev/video0"), 1, None).is_empty());
    }

    #[test]
    fn a_missing_root_has_no_holders() {
        let t = tree("missing");
        assert!(find_holders(&t.0.join("nope"), Path::new("/dev/video0"), 1, None).is_empty());
    }

    #[test]
    fn alive_until_gone_or_a_zombie() {
        let t = tree("alive");
        process(&t.0, 12, "zoom", None, &[]);
        assert!(is_alive(&t.0, 12));
        // The comm may hold ") Z": the state is after the last ')'.
        fs::write(t.0.join("12/stat"), "12 (a) Z (b) S 1 1").unwrap();
        assert!(is_alive(&t.0, 12));
        fs::write(t.0.join("12/stat"), "12 (a) S (b) Z 1 1").unwrap();
        assert!(!is_alive(&t.0, 12));
        fs::write(t.0.join("12/stat"), "12 (zoom) X 1 1").unwrap();
        assert!(!is_alive(&t.0, 12));
        assert!(!is_alive(&t.0, 13));
    }
}

#[test]
fn holder_is_plain_data() {
    let h = Holder {
        pid: 1,
        name: "a".into(),
        can_close: false,
    };
    assert_eq!(h.clone(), h);
}
