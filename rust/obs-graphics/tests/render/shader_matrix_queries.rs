//! Tier 1 mirrors test_shader_matrix_queries.c with token shaders and parameters.
use obs_graphics::shader_matrix_queries::{self as queries, Entry, Host, Rejection};

const LOG_DEBUG: i32 = 400;

#[derive(Default)]
struct Fixture {
    context: bool,
    callbacks: bool,
    expected_shader: Option<usize>,
    viewproj_result: Option<usize>,
    world_result: Option<usize>,
    viewproj_calls: u32,
    world_calls: u32,
    logs: Vec<(i32, String)>,
    events: String,
}
impl Fixture {
    fn new() -> Self {
        Self {
            callbacks: true,
            ..Self::default()
        }
    }
    fn message(&self) -> &str {
        self.logs.last().map_or("", |(_, message)| message)
    }
}
impl Host for Fixture {
    type Shader = usize;
    type Param = usize;
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, entry: Entry, reason: Rejection) {
        let reason = reason.message().to_str().unwrap();
        self.logs
            .push((LOG_DEBUG, format!("{}: {reason}", entry.name())));
        self.events.push('L');
    }
    fn viewproj(&mut self, shader: usize) -> Option<usize> {
        assert!(self.context && self.callbacks);
        assert_eq!(Some(shader), self.expected_shader);
        self.viewproj_calls += 1;
        self.events.push('V');
        self.viewproj_result
    }
    fn world(&mut self, shader: usize) -> Option<usize> {
        assert!(self.context && self.callbacks);
        assert_eq!(Some(shader), self.expected_shader);
        self.world_calls += 1;
        self.events.push('W');
        self.world_result
    }
}

#[test]
fn test_shader_matrix_queries_no_context() {
    let mut host = Fixture::new();
    // Context rejection precedes shader rejection, even without callbacks.
    for i in 0..2 {
        let shader = if i == 0 { Some(10) } else { None };
        assert_eq!(queries::get_viewproj_matrix(&mut host, shader), None);
        assert_eq!(
            host.message(),
            "gs_shader_get_viewproj_matrix: called while not in a graphics context"
        );
        assert_eq!(host.logs.len(), 2 * i + 1);
        assert_eq!(queries::get_world_matrix(&mut host, shader), None);
        assert_eq!(
            host.message(),
            "gs_shader_get_world_matrix: called while not in a graphics context"
        );
        assert_eq!(host.logs.len(), 2 * i + 2);
        assert_eq!((host.viewproj_calls, host.world_calls), (0, 0));
        host.callbacks = false;
    }
    assert!(host.logs.iter().all(|(level, _)| *level == LOG_DEBUG));
    assert_eq!(host.events, "LLLL");
}

#[test]
fn test_shader_matrix_queries_null_shader_in_context() {
    let mut host = Fixture {
        context: true,
        ..Fixture::new()
    };
    for i in 0..2 {
        assert_eq!(queries::get_viewproj_matrix(&mut host, None), None);
        assert_eq!(
            host.message(),
            "gs_shader_get_viewproj_matrix: Null 'shader' parameter"
        );
        assert_eq!(host.logs.len(), 2 * i + 1);
        assert_eq!(queries::get_world_matrix(&mut host, None), None);
        assert_eq!(
            host.message(),
            "gs_shader_get_world_matrix: Null 'shader' parameter"
        );
        assert_eq!(host.logs.len(), 2 * i + 2);
        assert_eq!((host.viewproj_calls, host.world_calls), (0, 0));
        host.callbacks = false;
    }
    assert_eq!(host.events, "LLLL");
}

#[test]
fn test_shader_matrix_queries_forward_shader_and_borrowed_results() {
    let mut host = Fixture {
        context: true,
        viewproj_result: Some(20),
        world_result: Some(21),
        ..Fixture::new()
    };
    for i in 0..2u32 {
        let shader = 10 + i as usize;
        host.expected_shader = Some(shader);
        assert_eq!(
            queries::get_viewproj_matrix(&mut host, Some(shader)),
            Some(20)
        );
        assert_eq!((host.viewproj_calls, host.world_calls), (i + 1, i));
        assert_eq!(queries::get_world_matrix(&mut host, Some(shader)), Some(21));
        assert_eq!((host.viewproj_calls, host.world_calls), (i + 1, i + 1));
        assert!(host.logs.is_empty());
    }
    assert_eq!(host.events, "VWVW");
}

#[test]
fn test_shader_matrix_queries_callback_null_results_are_returned_silently() {
    let mut host = Fixture {
        context: true,
        expected_shader: Some(10),
        ..Fixture::new()
    };
    assert_eq!(queries::get_world_matrix(&mut host, Some(10)), None);
    assert_eq!((host.viewproj_calls, host.world_calls), (0, 1));
    assert_eq!(queries::get_viewproj_matrix(&mut host, Some(10)), None);
    assert_eq!((host.viewproj_calls, host.world_calls), (1, 1));
    assert!(host.logs.is_empty());
    assert_eq!(host.events, "WV");
}

#[test]
fn diagnostic_names_match_c() {
    for entry in [Entry::ViewProj, Entry::World] {
        assert_eq!(entry.c_name().to_bytes(), entry.name().as_bytes());
    }
}
