//! Tier 1 mirrors every assertion of test_graphics_device_queries.c.
use obs_graphics::device_queries::{self as queries, Entry, Host};

#[derive(Default)]
struct Fixture {
    context: bool,
    optional_callback: bool,
    device_type: i32,
    adapter_count: u32,
    type_calls: u32,
    adapter_calls: u32,
    logs: Vec<(i32, String)>,
}

impl Host for Fixture {
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, entry: Entry) {
        self.logs.push((400, format!("{}: called while not in a graphics context", entry.name())));
    }
    fn device_type(&mut self) -> i32 {
        assert!(self.context);
        self.type_calls += 1;
        self.device_type
    }
    fn has_adapter_count(&self) -> bool {
        assert!(self.context, "context must precede even callback field access");
        self.optional_callback
    }
    fn adapter_count(&mut self) -> u32 {
        assert!(self.context && self.optional_callback);
        self.adapter_calls += 1;
        self.adapter_count
    }
}

#[test]
fn test_graphics_device_queries_no_context() {
    let mut host = Fixture::default();
    for (i, callbacks) in [true, false].into_iter().enumerate() {
        host.optional_callback = callbacks;
        let value = queries::get_device_type(&mut host);
        assert_eq!(value, None);
        assert_eq!(value.unwrap_or(-1), -1);
        assert_eq!(host.logs.len(), 2 * i + 1);
        assert_eq!(host.logs.last(), Some(&(400, "gs_get_device_type: called while not in a graphics context".into())));
        let value = queries::get_adapter_count(&mut host);
        assert_eq!(value, None);
        assert_eq!(value.unwrap_or(0), 0);
        assert_eq!(host.logs.len(), 2 * i + 2);
        assert_eq!(host.logs.last(), Some(&(400, "gs_get_adapter_count: called while not in a graphics context".into())));
        assert_eq!((host.type_calls, host.adapter_calls), (0, 0));
    }
}

#[test]
fn test_graphics_device_queries_optional_adapter_callback() {
    let mut host = Fixture { context: true, ..Fixture::default() };
    for (present, calls) in [(false, 0), (true, 1), (false, 1)] {
        host.optional_callback = present;
        assert_eq!(queries::get_adapter_count(&mut host), Some(0));
        assert_eq!((host.type_calls, host.adapter_calls), (0, calls));
        assert!(host.logs.is_empty());
    }
}

#[test]
fn test_graphics_device_queries_device_type_values() {
    let mut host = Fixture { context: true, ..Fixture::default() };
    for (i, value) in [i32::MIN, -7, -1, 0, 1, 2, 42, i32::MAX].into_iter().enumerate() {
        host.device_type = value;
        assert_eq!(queries::get_device_type(&mut host), Some(value));
        assert_eq!((host.type_calls, host.adapter_calls), (i as u32 + 1, 0));
        assert!(host.logs.is_empty());
    }
}

#[test]
fn test_graphics_device_queries_adapter_count_values() {
    let mut host = Fixture { context: true, optional_callback: true, ..Fixture::default() };
    for (i, value) in [0, 1, 2, 37, 0x8000_0000, u32::MAX].into_iter().enumerate() {
        host.adapter_count = value;
        assert_eq!(queries::get_adapter_count(&mut host), Some(value));
        assert_eq!((host.type_calls, host.adapter_calls), (0, i as u32 + 1));
        assert!(host.logs.is_empty());
    }
    // Deterministic transitions distinguish context absence from backend zero.
    host.context = false;
    assert_eq!(queries::get_adapter_count(&mut host), None);
    host.context = true;
    host.adapter_count = 0;
    assert_eq!(queries::get_adapter_count(&mut host), Some(0));
    assert_eq!((host.type_calls, host.adapter_calls, host.logs.len()), (0, 7, 1));
}

#[test]
fn diagnostic_names_match_c() {
    for entry in [Entry::DeviceType, Entry::AdapterCount] {
        assert_eq!(entry.c_name().to_bytes(), entry.name().as_bytes());
    }
}
