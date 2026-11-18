//! Tier 1 mirrors test/cmocka/test_graphics_texture_copy.c through the safe API.
use obs_graphics::texture_copy::{Entry, Host, Rejection, copy_texture, copy_texture_region};

#[derive(Debug, PartialEq, Eq)]
struct Call {
    kind: Entry,
    device: u8,
    dst: u8,
    src: Option<u8>,
    values: [u32; 6],
}

#[derive(Default)]
struct Fixture {
    context: bool,
    calls: Vec<Call>,
    logs: Vec<(i32, String)>,
}

impl Fixture {
    fn diagnostic(&self, expected: &str, count: usize) {
        assert_eq!(self.logs.len(), count);
        assert_eq!(self.logs.last(), Some(&(400, expected.into())));
        assert!(self.calls.is_empty());
    }
    fn forwarded(&self, n: usize, kind: Entry, dst: u8, src: Option<u8>, values: [u32; 6]) {
        assert_eq!(self.calls.len(), n + 1);
        assert_eq!(
            self.calls[n],
            Call {
                kind,
                device: 7,
                dst,
                src,
                values
            }
        );
        assert!(self.logs.is_empty());
    }
}

impl Host for Fixture {
    type Texture = u8;
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, entry: Entry, reason: Rejection) {
        self.logs.push((
            400,
            format!("{}: {}", entry.name(), reason.message().to_str().unwrap()),
        ));
    }
    fn whole(&mut self, dst: u8, src: u8) {
        self.calls.push(Call {
            kind: Entry::Whole,
            device: 7,
            dst,
            src: Some(src),
            values: [0; 6],
        });
    }
    fn region(&mut self, dst: u8, src: Option<u8>, values: [u32; 6]) {
        self.calls.push(Call {
            kind: Entry::Region,
            device: 7,
            dst,
            src,
            values,
        });
    }
}

#[test]
fn test_graphics_texture_copy_context_before_pointers() {
    let mut f = Fixture::default();
    let mut logs = 0;
    for mask in 0..4 {
        let dst = (mask & 1 != 0).then_some(1);
        let src = (mask & 2 != 0).then_some(2);
        copy_texture(&mut f, dst, src);
        logs += 1;
        f.diagnostic(
            "gs_copy_texture: called while not in a graphics context",
            logs,
        );
        copy_texture_region(&mut f, dst, src, [1, 2, 3, 4, 5, 6]);
        logs += 1;
        f.diagnostic(
            "gs_copy_texture_region: called while not in a graphics context",
            logs,
        );
    }
}

#[test]
fn test_graphics_texture_copy_pointer_guards() {
    let mut f = Fixture {
        context: true,
        ..Fixture::default()
    };
    copy_texture(&mut f, None, None);
    f.diagnostic("gs_copy_texture: Null 'dst' parameter", 1);
    copy_texture(&mut f, None, Some(2));
    f.diagnostic("gs_copy_texture: Null 'dst' parameter", 2);
    copy_texture(&mut f, Some(1), None);
    f.diagnostic("gs_copy_texture: Null 'src' parameter", 3);
    copy_texture_region(&mut f, None, None, [1, 2, 3, 4, 5, 6]);
    f.diagnostic("gs_copy_texture_region: Null 'dst' parameter", 4);
    copy_texture_region(&mut f, None, Some(2), [1, 2, 3, 4, 5, 6]);
    f.diagnostic("gs_copy_texture_region: Null 'dst' parameter", 5);
}

#[test]
fn test_graphics_texture_copy_whole_forwarding() {
    let mut f = Fixture {
        context: true,
        ..Fixture::default()
    };
    copy_texture(&mut f, Some(1), Some(2));
    f.forwarded(0, Entry::Whole, 1, Some(2), [0; 6]);
    copy_texture(&mut f, Some(1), Some(1));
    f.forwarded(1, Entry::Whole, 1, Some(1), [0; 6]);
    f.context = false;
    f.calls.clear();
    copy_texture(&mut f, Some(1), Some(2));
    f.diagnostic("gs_copy_texture: called while not in a graphics context", 1);
}

#[test]
fn test_graphics_texture_copy_region_forwarding() {
    let mut f = Fixture {
        context: true,
        ..Fixture::default()
    };
    let mut values = vec![[11, 23, 37, 41, 53, 67], [0; 6]];
    for i in 0..6 {
        let mut v = [1, 2, 3, 4, 5, 0];
        // Mirror each distinct per-field UINT32_MAX row in the C test.
        for (index, value) in v.iter_mut().enumerate() {
            *value = if index < i {
                index as u32 + 1
            } else {
                index as u32
            };
        }
        v[i] = u32::MAX;
        values.push(v);
    }
    for src in [None, Some(2), Some(1)] {
        for &v in &values {
            let n = f.calls.len();
            copy_texture_region(&mut f, Some(1), src, v);
            f.forwarded(n, Entry::Region, 1, src, v);
        }
    }
    f.context = false;
    f.calls.clear();
    copy_texture_region(&mut f, Some(1), None, [1, 2, 3, 4, 5, 6]);
    f.diagnostic(
        "gs_copy_texture_region: called while not in a graphics context",
        1,
    );
}

#[test]
fn diagnostic_names_match_c() {
    for entry in [Entry::Whole, Entry::Region] {
        assert_eq!(entry.name().as_bytes(), entry.c_name().to_bytes());
    }
}
