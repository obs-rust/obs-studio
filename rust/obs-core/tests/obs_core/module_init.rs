use obs_core::module_init::{Lifecycle, initialize};

#[derive(Default)]
struct Module {
    loaded: bool,
    loading: bool,
    result: bool,
    events: Vec<&'static str>,
}
impl Lifecycle for Module {
    type Profile = usize;
    fn loaded(&self) -> bool {
        self.loaded
    }
    fn set_loaded(&mut self, value: bool) {
        self.loaded = value;
        self.events.push("stored");
    }
    fn profile_start(&mut self) -> usize {
        self.events.push("start");
        42
    }
    fn loading(&mut self, active: bool) {
        self.loading = active;
        self.events.push(if active { "enter" } else { "clear" });
    }
    fn load(&mut self) -> bool {
        assert!(self.loading);
        self.events.push("load");
        self.result
    }
    fn warning(&mut self) {
        assert!(!self.loading);
        assert!(!self.loaded);
        self.events.push("warning");
    }
    fn profile_end(&mut self, profile: usize) {
        assert_eq!(profile, 42);
        assert!(!self.loading);
        self.events.push("end");
    }
}
#[test]
fn missing_core_and_module_have_no_effects() {
    assert!(!initialize::<Module>(true, None));
    let mut m = Module::default();
    assert!(!initialize(false, Some(&mut m)));
    assert!(m.events.is_empty());
}
#[test]
fn callback_result_is_stored_and_failure_retries() {
    let mut m = Module::default();
    assert!(!initialize(true, Some(&mut m)));
    assert_eq!(
        m.events,
        [
            "start", "enter", "load", "stored", "clear", "warning", "end"
        ]
    );
    m.events.clear();
    m.result = true;
    assert!(initialize(true, Some(&mut m)));
    assert!(m.loaded);
    assert_eq!(
        m.events,
        ["start", "enter", "load", "stored", "clear", "end"]
    );
    m.events.clear();
    assert!(initialize(true, Some(&mut m)));
    assert!(m.events.is_empty());
}
