//! Tier 1: all four unchanged test_samplerstate_lifecycle C characterizations.
use obs_graphics::samplerstate_lifecycle::{self as core, Entry, Host, Rejection};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Settings([i64; 6]);
#[derive(Default)]
struct Fixture<'a> {
    context: bool,
    callbacks: bool,
    expected_info: Option<&'a Settings>,
    expected_sampler: usize,
    result: Option<usize>,
    calls: [usize; 2],
    logs: Vec<(i32, String)>,
    events: String,
}
impl<'a> Host for Fixture<'a> {
    type Info = &'a Settings;
    type Sampler = usize;
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, entry: Entry, reason: Rejection) {
        self.logs.push((
            400,
            format!("{}: {}", entry.name(), reason.message().to_str().unwrap()),
        ));
        self.events.push('L');
    }
    fn create(&mut self, info: &'a Settings) -> Option<usize> {
        assert!(self.context && self.callbacks);
        assert!(std::ptr::eq(info, self.expected_info.unwrap()));
        self.calls[0] += 1;
        self.events.push('C');
        self.result
    }
    fn destroy(&mut self, sampler: usize) {
        assert!(self.context && self.callbacks);
        assert_eq!(sampler, self.expected_sampler);
        self.calls[1] += 1;
        self.events.push('D');
    }
}
fn diagnostic(f: &Fixture<'_>, count: usize, message: &str) {
    assert_eq!(f.logs.len(), count);
    assert_eq!(f.logs.last(), Some(&(400, message.to_owned())));
    assert_eq!(f.calls, [0, 0]);
}
#[test]
fn test_samplerstate_lifecycle_no_context() {
    let info = Settings([0; 6]);
    let mut f = Fixture {
        callbacks: true,
        ..Fixture::default()
    };
    for i in 0..2 {
        assert_eq!(core::create(&mut f, Some(&info)), None);
        diagnostic(
            &f,
            4 * i + 1,
            "gs_samplerstate_create: called while not in a graphics context",
        );
        assert_eq!(core::create(&mut f, None), None);
        diagnostic(
            &f,
            4 * i + 2,
            "gs_samplerstate_create: called while not in a graphics context",
        );
        core::destroy(&mut f, Some(1));
        diagnostic(
            &f,
            4 * i + 3,
            "gs_samplerstate_destroy: called while not in a graphics context",
        );
        core::destroy(&mut f, None);
        diagnostic(
            &f,
            4 * i + 4,
            "gs_samplerstate_destroy: called while not in a graphics context",
        );
        f.callbacks = false;
    }
    assert_eq!(f.events, "LLLLLLLL");
}
#[test]
fn test_samplerstate_lifecycle_null_arguments_in_context() {
    let mut f = Fixture {
        context: true,
        callbacks: true,
        ..Fixture::default()
    };
    for i in 0..2 {
        assert_eq!(core::create(&mut f, None), None);
        diagnostic(&f, i + 1, "gs_samplerstate_create: Null 'info' parameter");
        core::destroy(&mut f, None);
        diagnostic(&f, i + 1, "gs_samplerstate_create: Null 'info' parameter");
        f.callbacks = false;
    }
    assert_eq!(f.events, "LL");
}
#[test]
fn test_samplerstate_lifecycle_creation_forwards_info_and_result() {
    let infos = [
        Settings([0, 0, 1, 2, i64::from(i32::MIN), 0]),
        Settings([2, 3, 4, 0, i64::from(i32::MAX), i64::from(u32::MAX)]),
    ];
    let before = infos;
    let mut f = Fixture {
        context: true,
        callbacks: true,
        ..Fixture::default()
    };
    for i in 0..3 {
        f.expected_info = Some(&infos[i % 2]);
        f.result = (i < 2).then_some(i + 1);
        let expected = f.result;
        assert_eq!(core::create(&mut f, Some(&infos[i % 2])), expected);
        assert_eq!(f.calls, [i + 1, 0]);
        assert!(f.logs.is_empty());
        assert_eq!(infos, before);
    }
    assert_eq!(f.events, "CCC");
}
#[test]
fn test_samplerstate_lifecycle_destruction_forwards_handles_in_order() {
    let mut f = Fixture {
        context: true,
        callbacks: true,
        ..Fixture::default()
    };
    for i in 0..3 {
        f.expected_sampler = i % 2 + 1;
        let sampler = f.expected_sampler;
        core::destroy(&mut f, Some(sampler));
        assert_eq!(f.calls, [0, i + 1]);
        core::destroy(&mut f, None);
        assert_eq!(f.calls, [0, i + 1]);
        assert!(f.logs.is_empty());
    }
    assert_eq!(f.events, "DDD");
}
