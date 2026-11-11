//! Mirrors test_shader_param_introspection.c, using opaque safe tokens.
use obs_graphics::shader_param_introspection::{
    Host, num_params, param_by_idx, param_by_name, param_info,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Info {
    kind: i32,
    name: Option<usize>,
}

struct Fixture {
    context: bool,
    logs: Vec<(&'static str, &'static str)>,
    order: String,
    count: i32,
    result: Option<usize>,
    index: u32,
    name: usize,
    writes: u32,
    backend: Info,
    output: Info,
}

impl Fixture {
    fn new(context: bool) -> Self {
        Self {
            context,
            logs: Vec::new(),
            order: String::new(),
            count: 0,
            result: Some(3),
            index: 0,
            name: 4,
            writes: 0,
            backend: Info {
                kind: 0,
                name: None,
            },
            output: Info {
                kind: 12,
                name: Some(5),
            },
        }
    }
    fn diagnostic(&mut self, function: &'static str, reason: &'static str) {
        assert_eq!(self.logs, [(function, reason)]);
        assert!(self.order.is_empty());
        self.logs.clear();
    }
}

impl Host for Fixture {
    type Handle = usize;
    type Name = usize;
    type Info = usize;
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, function: &'static str, reason: &'static str) {
        self.logs.push((function, reason));
    }
    fn count(&mut self, shader: usize) -> i32 {
        assert!(self.context);
        assert_eq!(shader, 1);
        self.order.push('C');
        self.count
    }
    fn by_index(&mut self, shader: usize, index: u32) -> Option<usize> {
        assert!(self.context);
        assert_eq!((shader, index), (1, self.index));
        self.order.push('I');
        self.result
    }
    fn by_name(&mut self, shader: usize, name: usize) -> Option<usize> {
        assert!(self.context);
        assert_eq!((shader, name), (1, self.name));
        self.order.push('N');
        self.result
    }
    fn info(&mut self, param: usize, info: usize) {
        assert!(self.context);
        assert_eq!((param, info), (2, 6));
        self.order.push('O');
        if self.writes & 1 != 0 {
            self.output.kind = self.backend.kind;
        }
        if self.writes & 2 != 0 {
            self.output.name = self.backend.name;
        }
    }
}

#[test]
fn test_shader_param_introspection_context_guards() {
    let mut h = Fixture::new(false);
    let original = h.output;
    let reason = "called while not in a graphics context";
    for valid in [false, true] {
        let shader = valid.then_some(1);
        assert_eq!(num_params(&mut h, shader), 0);
        h.diagnostic("gs_shader_get_num_params", reason);
        assert_eq!(param_by_idx(&mut h, shader, u32::MAX), None);
        h.diagnostic("gs_shader_get_param_by_idx", reason);
        for named in [false, true] {
            assert_eq!(param_by_name(&mut h, shader, named.then_some(4)), None);
            h.diagnostic("gs_shader_get_param_by_name", reason);
            param_info(&mut h, valid.then_some(2), named.then_some(6));
            h.diagnostic("gs_shader_get_param_info", reason);
            assert_eq!(h.output, original);
        }
    }
}

#[test]
fn test_shader_param_introspection_pointer_guards() {
    let mut h = Fixture::new(true);
    h.output.kind = 6;
    let original = h.output;
    assert_eq!(num_params(&mut h, None), 0);
    h.diagnostic("gs_shader_get_num_params", "Null 'shader' parameter");
    assert_eq!(param_by_idx(&mut h, None, 0), None);
    h.diagnostic("gs_shader_get_param_by_idx", "Null 'shader' parameter");
    for name in [None, Some(4)] {
        assert_eq!(param_by_name(&mut h, None, name), None);
        h.diagnostic("gs_shader_get_param_by_name", "Null 'shader' parameter");
    }
    assert_eq!(param_by_name(&mut h, Some(1), None), None);
    h.diagnostic("gs_shader_get_param_by_name", "Null 'name' parameter");
    for info in [None, Some(6)] {
        param_info(&mut h, None, info);
        h.diagnostic("gs_shader_get_param_info", "Null 'param' parameter");
        assert_eq!(h.output, original);
    }
    param_info(&mut h, Some(2), None);
    h.diagnostic("gs_shader_get_param_info", "Null 'info' parameter");
}

#[test]
fn test_shader_param_introspection_dispatch_results() {
    let mut h = Fixture::new(true);
    for (i, count) in [0, 1, -1, i32::MIN, i32::MAX].into_iter().enumerate() {
        h.count = count;
        assert_eq!(num_params(&mut h, Some(1)), count);
        assert_eq!(h.order.len(), i + 1);
    }
    assert_eq!(h.order, "CCCCC");
    for result in [None, Some(3)] {
        h.result = result;
        for index in [0, 1, u32::MAX] {
            h.index = index;
            let before = h.order.len();
            assert_eq!(param_by_idx(&mut h, Some(1), index), result);
            assert_eq!(h.order.len(), before + 1);
        }
        // Distinct tokens for the empty name and parameter[7].value.
        for name in [4, 7] {
            h.name = name;
            let before = h.order.len();
            assert_eq!(param_by_name(&mut h, Some(1), Some(name)), result);
            assert_eq!(h.order.len(), before + 1);
        }
    }
    assert_eq!(h.order, "CCCCCIIINNIIINN");
    assert!(h.logs.is_empty());
}

#[test]
fn test_shader_param_introspection_info_out_writes() {
    let mut h = Fixture::new(true);
    for writes in 0..4 {
        for name in [None, Some(7)] {
            h.output = Info {
                kind: 3,
                name: Some(5),
            };
            h.backend = Info { kind: 0, name };
            h.writes = writes;
            let before = h.order.len();
            param_info(&mut h, Some(2), Some(6));
            assert_eq!(h.order.len(), before + 1);
            assert_eq!(h.output.kind, if writes & 1 != 0 { 0 } else { 3 });
            assert_eq!(h.output.name, if writes & 2 != 0 { name } else { Some(5) });
        }
    }
    for kind in 1..=12 {
        h.output = Info {
            kind: 0,
            name: None,
        };
        h.backend = Info {
            kind,
            name: Some(7),
        };
        h.writes = 3;
        let before = h.order.len();
        param_info(&mut h, Some(2), Some(6));
        assert_eq!(h.order.len(), before + 1);
        assert_eq!(h.output, h.backend);
    }
    assert!(h.logs.is_empty());
}
