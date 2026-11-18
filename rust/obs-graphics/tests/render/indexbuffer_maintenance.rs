//! Tier 1 mirrors all assertions of test_graphics_indexbuffer_maintenance.c.
use obs_graphics::indexbuffer_maintenance::{self as ib, Entry, Host, Rejection};

#[derive(Default)]
struct Fixture {
    context: bool,
    calls: Vec<(Entry, usize, Option<usize>)>,
    logs: Vec<String>,
    count: usize,
}

impl Fixture {
    fn record(&mut self, entry: Entry, buffer: usize, data: Option<usize>) {
        assert!(self.context, "context before field access");
        self.calls.push((entry, buffer, data));
    }
    fn diagnostic(&self, name: &str, reason: &str, logs: usize) {
        assert_eq!(self.logs.last().unwrap(), &format!("{name}: {reason}"));
        assert_eq!(self.logs.len(), logs);
        assert!(self.calls.is_empty());
    }
    fn forwarded(&self, calls: usize, entry: Entry, data: Option<usize>) {
        assert_eq!(self.calls.len(), calls);
        assert_eq!(self.calls.last(), Some(&(entry, 0x10, data)));
        assert!(self.logs.is_empty());
    }
}

impl Host for Fixture {
    type Buffer = usize;
    type Data = usize;
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, entry: Entry, reason: Rejection) {
        self.logs.push(format!(
            "{}: {}",
            entry.name(),
            reason.message().to_str().unwrap()
        ));
    }
    fn destroy(&mut self, buffer: usize) {
        self.record(Entry::Destroy, buffer, None);
    }
    fn flush(&mut self, buffer: usize) {
        self.record(Entry::Flush, buffer, None);
    }
    fn direct(&mut self, buffer: usize, data: usize) {
        self.record(Entry::Direct, buffer, Some(data));
    }
    fn count(&mut self, buffer: usize) -> usize {
        self.record(Entry::Count, buffer, None);
        self.count
    }
}

#[test]
fn test_graphics_indexbuffer_maintenance_context_before_pointers() {
    let mut host = Fixture::default();
    let reason = "called while not in a graphics context";
    for nonnull in 0..2 {
        let buffer = (nonnull != 0).then_some(0x10);
        let data = (nonnull != 0).then_some(0x20);
        ib::destroy(&mut host, buffer);
        host.diagnostic("gs_indexbuffer_destroy", reason, nonnull * 4 + 1);
        ib::flush(&mut host, buffer);
        host.diagnostic("gs_indexbuffer_flush", reason, nonnull * 4 + 2);
        ib::flush_direct(&mut host, buffer, data);
        host.diagnostic("gs_indexbuffer_flush_direct", reason, nonnull * 4 + 3);
        assert_eq!(ib::get_num_indices(&mut host, buffer), 0);
        host.diagnostic("gs_indexbuffer_get_num_indices", reason, nonnull * 4 + 4);
    }
}

#[test]
fn test_graphics_indexbuffer_maintenance_pointer_guards() {
    let mut host = Fixture {
        context: true,
        ..Fixture::default()
    };
    ib::destroy(&mut host, None);
    assert!(host.logs.is_empty());
    assert!(host.calls.is_empty());
    ib::flush(&mut host, None);
    host.diagnostic("gs_indexbuffer_flush", "Null 'indexbuffer' parameter", 1);
    ib::flush_direct(&mut host, None, None);
    host.diagnostic(
        "gs_indexbuffer_flush_direct",
        "Null 'indexbuffer' parameter",
        2,
    );
    ib::flush_direct(&mut host, None, Some(0x20));
    host.diagnostic(
        "gs_indexbuffer_flush_direct",
        "Null 'indexbuffer' parameter",
        3,
    );
    ib::flush_direct(&mut host, Some(0x10), None);
    host.diagnostic("gs_indexbuffer_flush_direct", "Null 'data' parameter", 4);
    assert_eq!(ib::get_num_indices(&mut host, None), 0);
    host.diagnostic(
        "gs_indexbuffer_get_num_indices",
        "Null 'indexbuffer' parameter",
        5,
    );
}

#[test]
fn test_graphics_indexbuffer_maintenance_backend_forwarding() {
    let mut host = Fixture {
        context: true,
        ..Fixture::default()
    };
    ib::destroy(&mut host, Some(0x10));
    host.forwarded(1, Entry::Destroy, None);
    ib::flush(&mut host, Some(0x10));
    host.forwarded(2, Entry::Flush, None);
    ib::flush_direct(&mut host, Some(0x10), Some(0x20));
    host.forwarded(3, Entry::Direct, Some(0x20));
    for (i, count) in [0, 1, u32::MAX as usize, usize::MAX / 2 + 1, usize::MAX]
        .into_iter()
        .enumerate()
    {
        host.count = count;
        assert_eq!(ib::get_num_indices(&mut host, Some(0x10)), count);
        host.forwarded(4 + i, Entry::Count, None);
    }
}

#[test]
fn diagnostic_names_match_c() {
    for entry in [Entry::Destroy, Entry::Flush, Entry::Direct, Entry::Count] {
        assert_eq!(entry.c_name().to_str().unwrap(), entry.name());
    }
}
