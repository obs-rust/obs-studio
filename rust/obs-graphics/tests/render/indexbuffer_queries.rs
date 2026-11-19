//! Tier 1 mirrors every assertion in test_graphics_indexbuffer_queries.c.
use obs_graphics::indexbuffer_queries::{Entry, Host, Rejection, get_data, get_type};

#[derive(Default)]
struct Fake {
    context: bool,
    expected_buffer: usize,
    result: Option<usize>,
    index_type: u32,
    calls: [usize; 2],
    logs: Vec<String>,
}

impl Host for Fake {
    type Buffer = usize;
    type Data = usize;
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, entry: Entry, reason: Rejection) {
        self.logs
            .push(format!("{}: {}", entry.name(), reason.message()));
    }
    fn data(&mut self, buffer: usize) -> Option<usize> {
        assert!(self.context, "no projection without context");
        assert_eq!(buffer, self.expected_buffer);
        self.calls[0] += 1;
        self.result
    }
    fn index_type(&mut self, buffer: usize) -> u32 {
        assert!(self.context, "no projection without context");
        assert_eq!(buffer, self.expected_buffer);
        self.calls[1] += 1;
        self.index_type
    }
}

fn diagnostic(host: &Fake, name: &str, reason: &str, count: usize) {
    assert_eq!(host.logs.last().unwrap(), &format!("{name}: {reason}"));
    assert_eq!(host.logs.len(), count);
    assert_eq!(host.calls, [0, 0]);
}

#[test]
fn test_graphics_indexbuffer_queries_context_before_pointers() {
    let mut host = Fake::default();
    for (i, buffer) in [None, Some(0)].into_iter().enumerate() {
        assert_eq!(get_data(&mut host, buffer), None);
        diagnostic(
            &host,
            "gs_indexbuffer_get_data",
            "called while not in a graphics context",
            i * 2 + 1,
        );
        assert_eq!(get_type(&mut host, buffer), 0);
        diagnostic(
            &host,
            "gs_indexbuffer_get_type",
            "called while not in a graphics context",
            i * 2 + 2,
        );
    }
}

#[test]
fn test_graphics_indexbuffer_queries_pointer_guards() {
    let mut host = Fake {
        context: true,
        ..Fake::default()
    };
    assert_eq!(get_data(&mut host, None), None);
    diagnostic(
        &host,
        "gs_indexbuffer_get_data",
        "Null 'indexbuffer' parameter",
        1,
    );
    assert_eq!(get_type(&mut host, None), 0);
    diagnostic(
        &host,
        "gs_indexbuffer_get_type",
        "Null 'indexbuffer' parameter",
        2,
    );
}

#[test]
fn test_graphics_indexbuffer_queries_backend_results_and_buffer_routing() {
    let mut host = Fake {
        context: true,
        ..Fake::default()
    };
    for buffer in 0..2 {
        host.expected_buffer = buffer;
        for data in [None, Some(0), Some(1)] {
            host.result = data;
            let before = host.calls;
            assert_eq!(get_data(&mut host, Some(buffer)), data);
            assert_eq!(host.calls, [before[0] + 1, before[1]]);
            assert!(host.logs.is_empty());
        }
        // GS_UNSIGNED_SHORT and GS_UNSIGNED_LONG; Tier 3 checks real C values.
        for index_type in [0, 1] {
            host.index_type = index_type;
            let before = host.calls;
            assert_eq!(get_type(&mut host, Some(buffer)), index_type);
            assert_eq!(host.calls, [before[0], before[1] + 1]);
            assert!(host.logs.is_empty());
        }
    }
    assert_eq!(host.calls, [6, 4]);
}
