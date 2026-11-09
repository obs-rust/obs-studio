//! Tier 1 mirrors test_source_save_load.c, including settings/data identity.
use obs_source::save_load::{self, Backend};

#[derive(Clone)]
struct Source {
    valid: bool,
    private: bool,
    callback: bool,
    name: char,
    settings: Box<[i32; 2]>,
    filters: Vec<usize>,
}
struct Probe {
    sources: Vec<Source>,
    events: String,
}
impl Probe {
    fn new(callback: bool, valid: bool, private: bool, name: char) -> Self {
        Self {
            sources: vec![Source {
                valid,
                private,
                callback,
                name,
                settings: Box::new([0, 0]),
                filters: vec![],
            }],
            events: String::new(),
        }
    }
    fn event(&mut self, source: usize, kind: char) {
        self.events.push(kind);
        self.events.push(self.sources[source].name);
    }
}
impl Backend for Probe {
    type Source = usize;
    fn valid(&mut self, source: usize, _: &'static str) -> bool {
        self.sources.get(source).is_some_and(|s| s.valid)
    }
    fn signal(&mut self, source: usize, _: bool) {
        if !self.sources[source].private {
            self.event(source, 'G');
        }
        self.event(source, 'S');
    }
    fn callback(&mut self, source: usize, save: bool) {
        if self.sources[source].callback {
            self.event(source, 'C');
            self.sources[source].settings[usize::from(!save)] = if save { 42 } else { 73 };
        }
    }
    fn filter_count(&mut self, source: usize) -> usize {
        self.sources[source].filters.len()
    }
    fn filter(&mut self, source: usize, index: usize) -> usize {
        self.sources[source].filters[index]
    }
}

#[test]
fn callback_order_and_settings() {
    let mut p = Probe::new(true, true, false, 'p');
    let settings = p.sources[0].settings.as_ptr();
    save_load::save(&mut p, 0);
    assert_eq!(p.events, "GpSpCp");
    assert_eq!(p.sources[0].settings[0], 42);
    p.events.clear();
    save_load::load(&mut p, 0);
    assert_eq!(p.events, "CpGpSp");
    assert_eq!(p.sources[0].settings[1], 73);
    assert_eq!(settings, p.sources[0].settings.as_ptr());
}
#[test]
fn optional_callbacks_still_signal() {
    let mut p = Probe::new(false, true, false, 'o');
    save_load::save(&mut p, 0);
    save_load::load(&mut p, 0);
    save_load::load2(&mut p, 0);
    assert_eq!(p.events, "GoSoGoSoGoSo");
}
#[test]
fn invalid_plugin_data_is_silent() {
    let mut p = Probe::new(true, false, false, 'i');
    p.sources
        .push(Probe::new(true, true, true, 'f').sources.remove(0));
    p.sources[0].filters.push(1);
    for source in [usize::MAX, 0] {
        save_load::save(&mut p, source);
        save_load::load(&mut p, source);
        save_load::load2(&mut p, source);
    }
    assert_eq!(p.events, "");
}
#[test]
fn private_source_only_signals_locally() {
    let mut p = Probe::new(true, true, true, 'p');
    save_load::save(&mut p, 0);
    save_load::load2(&mut p, 0);
    assert_eq!(p.events, "SpCpCpSp");
}
#[test]
fn load2_loads_parent_then_reverse_filters() {
    let mut p = Probe::new(true, true, false, 'p');
    for (name, valid) in [('a', true), ('i', false), ('b', true)] {
        p.sources
            .push(Probe::new(true, valid, true, name).sources.remove(0));
        let index = p.sources.len() - 1;
        p.sources[0].filters.insert(0, index);
    }
    assert_eq!(p.sources[0].filters, [3, 2, 1]);
    save_load::load(&mut p, 0);
    assert_eq!(p.events, "CpGpSp");
    p.events.clear();
    save_load::load2(&mut p, 0);
    assert_eq!(p.events, "CpGpSpCaSaCbSb");
    p.events.clear();
    save_load::save(&mut p, 0);
    assert_eq!(p.events, "GpSpCp");
}
