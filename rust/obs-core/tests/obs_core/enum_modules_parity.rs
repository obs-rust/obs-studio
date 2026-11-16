//! Original-C parity, including live successor mutation. No domain exclusions
//! beyond C's requirements: live nodes/core, acyclic lists and a non-NULL
//! callback for populated lists. No own threads; fixtures use oracle TLS.
use obs_c_oracle::module_lists::*;
use obs_core::ffi::enum_modules::{Callback, obs_enum_modules};
use proptest::prelude::*;
use std::cell::Cell;
use std::ffi::c_void;
use std::ptr;

type Enum = unsafe extern "C" fn(Callback, *mut c_void);
unsafe extern "C" {
    fn oracle_obs_enum_modules(callback: Callback, param: *mut c_void);
    fn oracle_enum_modules_set_next(module: *mut c_void, next: *mut c_void);
    fn obs_module_lists_seam_module_next(module: *const c_void) -> *mut c_void;
}

struct World {
    core: *mut c_void,
    nodes: Vec<*mut c_void>,
}
impl World {
    fn new(present: bool, count: usize, disabled: usize) -> Self {
        // SAFETY: oracle owns the real-header core and copies these strings.
        let core = if present {
            unsafe { oracle_module_lists_core_new(c"".as_ptr()) }
        } else {
            ptr::null_mut()
        };
        // SAFETY: install this thread's live fixture, or no core.
        unsafe { oracle_module_lists_set_core(core) };
        let mut world = Self {
            core,
            nodes: Vec::new(),
        };
        if present {
            for _ in 0..count {
                world.add(true);
            }
            for _ in 0..disabled {
                world.add(false);
            }
        }
        world
    }
    fn add(&mut self, loaded: bool) -> *mut c_void {
        // SAFETY: live core; copied strings. The oracle prepends opened nodes.
        let node = unsafe {
            oracle_module_lists_module_new(
                self.core,
                c"stub".as_ptr(),
                c"".as_ptr(),
                c"stub".as_ptr(),
                loaded,
            )
        };
        self.nodes.push(node);
        node
    }
    fn index(&self, node: *mut c_void) -> usize {
        self.nodes.iter().position(|&n| n == node).unwrap()
    }
}
impl Drop for World {
    fn drop(&mut self) {
        // SAFETY: traversal has returned; disconnect TLS before freeing nodes
        // and the core. The core helper does not walk or free module nodes.
        unsafe {
            oracle_module_lists_set_core(ptr::null_mut());
            for &node in &self.nodes {
                oracle_module_lists_module_free(node);
            }
            if !self.core.is_null() {
                oracle_module_lists_core_free(self.core);
            }
        }
    }
}
struct Visits {
    world: World,
    enumerate: Enum,
    mode: u8,
    outer: Vec<usize>,
    nested: Vec<usize>,
    expected_param: *mut c_void,
    exact_param: bool,
}
unsafe extern "C" fn collect_nested(param: *mut c_void, node: *mut c_void) {
    // SAFETY: enumeration receives this live Visits as its exact parameter.
    let visits = unsafe { &mut *param.cast::<Visits>() };
    visits.exact_param &= param == visits.expected_param;
    visits.nested.push(visits.world.index(node));
}
unsafe extern "C" fn collect(param: *mut c_void, node: *mut c_void) {
    // SAFETY: caller passes a unique live Visits for each callback duration.
    let visits = unsafe { &mut *param.cast::<Visits>() };
    visits.exact_param &= param == visits.expected_param;
    visits.outer.push(visits.world.index(node));
    if visits.outer.len() == 1 {
        match visits.mode {
            1 => {
                visits.world.add(true);
                let enumerate = visits.enumerate;
                // End the mutable Visits borrow before reentrant callbacks.
                // SAFETY: same live fixture and parameter, fresh callback borrow.
                unsafe { enumerate(Some(collect_nested), param) };
            }
            2 => {
                // SAFETY: all nodes remain allocated; truncate current live link.
                unsafe { oracle_enum_modules_set_next(node, ptr::null_mut()) };
            }
            3 => {
                // SAFETY: node is live; skip one successor without freeing it.
                unsafe {
                    let next = obs_module_lists_seam_module_next(node);
                    if !next.is_null() {
                        oracle_enum_modules_set_next(node, obs_module_lists_seam_module_next(next));
                    }
                }
            }
            _ => {}
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    outer: Vec<usize>,
    nested: Vec<usize>,
    after: Vec<usize>,
    links: Vec<Option<usize>>,
    exact_param: bool,
}
fn run(enumerate: Enum, present: bool, count: usize, disabled: usize, mode: u8) -> Outcome {
    let mut visits = Visits {
        world: World::new(present, count, disabled),
        enumerate,
        mode,
        outer: Vec::new(),
        nested: Vec::new(),
        expected_param: ptr::null_mut(),
        exact_param: true,
    };
    let param = ptr::from_mut(&mut visits).cast();
    visits.expected_param = param;
    // SAFETY: live core/nodes and exact parameter remain valid for both calls.
    unsafe {
        enumerate(Some(collect), param);
        enumerate(Some(collect_nested), param);
    }
    let after = std::mem::take(&mut visits.nested);
    // The first nested segment (mode 1) precedes the later complete traversal.
    let nested_len = if mode == 1 && count > 0 && present {
        count + 1
    } else {
        0
    };
    let nested = after[..nested_len].to_vec();
    let after = after[nested_len..].to_vec();
    let links = visits
        .world
        .nodes
        .iter()
        .map(|&node| {
            // SAFETY: every node is still live; access uses the real C seam.
            let next = unsafe { obs_module_lists_seam_module_next(node) };
            (!next.is_null()).then(|| visits.world.index(next))
        })
        .collect();
    Outcome {
        outer: std::mem::take(&mut visits.outer),
        nested,
        after,
        links,
        exact_param: visits.exact_param,
    }
}
fn check(present: bool, count: usize, disabled: usize, mode: u8) {
    let c = run(oracle_obs_enum_modules, present, count, disabled, mode);
    let rust = run(obs_enum_modules, present, count, disabled, mode);
    assert!(rust.exact_param);
    assert_eq!(rust, c);
}

#[test]
fn characterized_edges_and_post_callback_successor() {
    for present in [false, true] {
        for count in [0, 1, 2, 3, 16] {
            for mode in 0..4 {
                check(present, count, 2, mode);
            }
        }
    }
}
thread_local! { static NULL_PARAM_CALLS: Cell<usize> = const { Cell::new(0) }; }
unsafe extern "C" fn null_param(param: *mut c_void, node: *mut c_void) {
    if param.is_null() && !node.is_null() {
        NULL_PARAM_CALLS.set(NULL_PARAM_CALLS.get() + 1);
    }
}
#[test]
fn null_callback_on_no_core_empty_disabled_and_null_parameter() {
    for enumerate in [oracle_obs_enum_modules as Enum, obs_enum_modules as Enum] {
        for present in [false, true] {
            let _world = World::new(present, 0, 2);
            // SAFETY: NULL callback is valid on no-core and empty loaded lists.
            unsafe { enumerate(None, ptr::null_mut()) };
        }
        let _world = World::new(true, 3, 2);
        NULL_PARAM_CALLS.set(0);
        // SAFETY: callback accepts a NULL parameter and live module pointers.
        unsafe { enumerate(Some(null_param), ptr::null_mut()) };
        assert_eq!(NULL_PARAM_CALLS.get(), 3);
    }
}
proptest! {
    #![proptest_config(ProptestConfig { cases: obs_c_oracle::parity::cases(128), failure_persistence: None, ..ProptestConfig::default() })]
    #[test]
    fn live_traversal_matches_original_c(present in any::<bool>(), count in 0usize..17, disabled in 0usize..5, mode in 0u8..4) {
        check(present, count, disabled, mode);
    }
}
