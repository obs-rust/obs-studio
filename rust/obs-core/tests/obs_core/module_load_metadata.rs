use obs_core::module_load_metadata::{ManifestSource, load};

struct Fixture {
    exists: bool,
    populated: bool,
}
impl ManifestSource for Fixture {
    fn open(&mut self) -> bool {
        self.exists
    }
    fn string(&self, key: &str) -> Vec<u8> {
        if !self.populated {
            return Vec::new();
        }
        match key {
            "display_name" => b"Shown".to_vec(),
            "id" => b"id".to_vec(),
            "version" => b"1.2".to_vec(),
            "os_arch" => b"arch".to_vec(),
            "name" => b"name".to_vec(),
            "description" => b"short".to_vec(),
            "long_description" => b"long".to_vec(),
            _ => panic!("unexpected key"),
        }
    }
    fn url(&self, key: &str) -> Vec<u8> {
        if !self.populated {
            return Vec::new();
        }
        match key {
            "repository" => b"repo".to_vec(),
            "website" => b"web".to_vec(),
            "support" => b"help".to_vec(),
            _ => panic!("unexpected URL"),
        }
    }
    fn boolean(&self, key: &str) -> bool {
        assert!(matches!(key, "has_banner" | "has_icon"));
        self.populated
    }
}

#[test]
fn check_manifest() {
    // Mirrors test_module_load_metadata.c, including all ten strings and both flags.
    let m = load(&mut Fixture {
        exists: true,
        populated: true,
    })
    .unwrap();
    let expected: [&[u8]; 10] = [
        b"Shown", b"id", b"1.2", b"arch", b"name", b"short", b"long", b"repo", b"web", b"help",
    ];
    for (actual, expected) in m.strings.iter().zip(expected) {
        assert_eq!(actual, expected);
    }
    assert!(m.has_banner);
    assert!(m.has_icon);
    // The data reader supplies empty defaults for {}, malformed JSON, arrays and wrong types.
    for _ in 0..4 {
        let m = load(&mut Fixture {
            exists: true,
            populated: false,
        })
        .unwrap();
        assert!(m.strings.iter().all(Vec::is_empty));
        assert!(!m.has_banner);
        assert!(!m.has_icon);
    }
    assert!(
        load(&mut Fixture {
            exists: false,
            populated: true
        })
        .is_none()
    );
}
