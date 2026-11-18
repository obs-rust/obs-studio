//! Mirrors every assertion in test_texture_file_data_legacy.c over a safe session.
use obs_graphics::texture_file_data::{Backend, Metadata, create_texture_file_data};

const BEFORE: Metadata = Metadata {
    format: 4,
    width: 91,
    height: 92,
};
const AFTER: Metadata = Metadata {
    format: 3,
    width: 2,
    height: 1,
};
const PIXELS: [u8; 8] = [1, 2, 3, 128, 4, 5, 6, 0];
struct Recording {
    initialized: bool,
    decoded: bool,
    calls: Vec<&'static str>,
    outputs: Metadata,
    pixels: Option<Box<[u8; 8]>>,
    identity: usize,
}
impl Backend for Recording {
    type Pixels = Box<[u8; 8]>;
    fn init(&mut self) -> bool {
        assert!(self.calls.is_empty());
        self.calls.push("init");
        self.initialized
    }
    fn decode_straight(&mut self) -> Option<Self::Pixels> {
        assert_eq!(self.calls, ["init"]);
        assert_eq!(self.outputs, BEFORE);
        self.calls.push("decode-straight");
        self.pixels.take()
    }
    fn metadata(&mut self) -> Metadata {
        assert_eq!(self.calls, ["init", "decode-straight"]);
        self.calls.push("metadata");
        AFTER
    }
    fn publish(&mut self, metadata: Metadata) {
        self.calls.push("publish");
        self.outputs = metadata;
    }
    fn free(&mut self) {
        assert_eq!(self.outputs, if self.decoded { AFTER } else { BEFORE });
        self.calls.push("free");
    }
}
fn check_load(initialized: bool, decoded: bool) {
    let pixels = decoded.then(|| Box::new(PIXELS));
    let identity = pixels.as_ref().map_or(0, |p| p.as_ptr() as usize);
    let mut session = Recording {
        initialized,
        decoded,
        calls: vec![],
        outputs: BEFORE,
        pixels,
        identity,
    };
    let result = create_texture_file_data(&mut session);
    assert_eq!(result.is_some(), initialized && decoded);
    if let Some(data) = result {
        assert_eq!(data.as_ptr() as usize, session.identity);
        assert_eq!(*data, PIXELS);
    }
    let expected: &[&str] = if !initialized {
        &["init"]
    } else if !decoded {
        &["init", "decode-straight", "free"]
    } else {
        &["init", "decode-straight", "metadata", "publish", "free"]
    };
    assert_eq!(session.calls, expected);
    assert_eq!(
        session.outputs,
        if initialized && decoded {
            AFTER
        } else {
            BEFORE
        }
    );
}
#[test]
fn failed_init_has_no_decode_or_second_free() {
    check_load(false, false);
}
#[test]
fn failed_decode_frees_without_output_writes() {
    check_load(true, false);
}
#[test]
fn successful_decode_publishes_before_free() {
    check_load(true, true);
}
#[test]
fn repeated_failure_and_success() {
    for _ in 0..8 {
        check_load(false, false);
        check_load(true, false);
        check_load(true, true);
    }
}
