//! Safe mirrors of test_source_properties.c, including callback arguments and effects.
use obs_source::properties::{Host, construct};

#[derive(Default)]
struct Model {
    modern: bool,
    legacy: bool,
    instance: bool,
    null_result: bool,
    events: Vec<&'static str>,
    seen_data: Option<u32>,
    seen_type: Option<u32>,
    value: i64,
}
impl Host for Model {
    type Properties = Option<&'static str>;
    type Settings = i64;
    fn modern(&self) -> bool {
        self.modern
    }
    fn legacy(&self) -> bool {
        self.legacy
    }
    fn defaults(&mut self) -> i64 {
        self.events.push("defaults");
        if self.modern { 29 } else { 17 }
    }
    fn construct(&mut self, modern: bool) -> Self::Properties {
        self.events.push(if modern { "modern" } else { "legacy" });
        self.seen_data = self.instance.then_some(83);
        self.seen_type = modern.then_some(42);
        (!self.null_result).then_some("value")
    }
    fn apply(&mut self, props: &Self::Properties, defaults: Option<&i64>) {
        self.events.push("apply");
        if props.is_some() {
            self.value = defaults.copied().unwrap_or(83);
        }
    }
    fn release(&mut self, _: i64) {
        self.events.push("release");
    }
}

#[test]
fn test_type_properties() {
    for (legacy, modern) in [(true, false), (true, true), (false, true)] {
        let mut h = Model {
            legacy,
            modern,
            ..Model::default()
        };
        assert_eq!(construct(&mut h, true), Some("value"));
        assert_eq!(h.seen_data, None);
        assert_eq!(h.seen_type, modern.then_some(42));
        assert_eq!(h.value, if modern { 29 } else { 17 });
        assert_eq!(
            h.events,
            [
                "defaults",
                if modern { "modern" } else { "legacy" },
                "apply",
                "release"
            ]
        );
    }
}

#[test]
fn test_instance_properties() {
    for (legacy, modern) in [(true, false), (true, true), (false, true), (false, false)] {
        let mut h = Model {
            legacy,
            modern,
            instance: true,
            ..Model::default()
        };
        let props = construct(&mut h, false);
        if legacy || modern {
            assert_eq!(props, Some("value"));
            assert_eq!(h.seen_data, Some(83));
            assert_eq!(h.seen_type, modern.then_some(42));
            assert_eq!(h.value, 83);
            assert_eq!(
                h.events,
                [if modern { "modern" } else { "legacy" }, "apply"]
            );
        } else {
            assert_eq!(props, None);
            assert!(h.events.is_empty());
        }
    }
}

#[test]
fn test_absent_properties() {
    for type_query in [false, true] {
        let mut h = Model::default();
        assert_eq!(construct(&mut h, type_query), None);
        assert!(h.events.is_empty());
    }
}

#[test]
fn test_null_callback_result() {
    for modern in [false, true] {
        for type_query in [false, true] {
            let mut h = Model {
                legacy: true,
                modern,
                instance: !type_query,
                null_result: true,
                ..Model::default()
            };
            assert_eq!(construct(&mut h, type_query), None);
            assert_eq!(h.value, 0);
            assert_eq!(h.seen_data, (!type_query).then_some(83));
            assert_eq!(h.seen_type, modern.then_some(42));
            assert_eq!(
                h.events.iter().filter(|e| **e == "defaults").count(),
                usize::from(type_query)
            );
            assert_eq!(
                h.events.iter().filter(|e| **e == "release").count(),
                usize::from(type_query)
            );
            assert_eq!(h.events.iter().filter(|e| **e == "apply").count(), 1);
        }
    }
}
