//! Mirrors test_output_total_bytes_query.c (#5203) against the safe core.
use core::cell::RefCell;
use obs_output::output_total_bytes::{Host, query};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Callback,
    Active,
    Capturing,
    Dispatch,
}

struct Mock {
    callback: bool,
    active: bool,
    capturing: bool,
    result: u64,
    trace: RefCell<Vec<Step>>,
}

impl Mock {
    fn new(callback: bool, active: bool, capturing: bool, result: u64) -> Self {
        Self {
            callback,
            active,
            capturing,
            result,
            trace: RefCell::new(Vec::new()),
        }
    }
}

impl Host for Mock {
    fn has_callback(&self) -> bool {
        self.trace.borrow_mut().push(Step::Callback);
        self.callback
    }
    fn delay_active(&self) -> bool {
        self.trace.borrow_mut().push(Step::Active);
        self.active
    }
    fn delay_capturing(&self) -> bool {
        self.trace.borrow_mut().push(Step::Capturing);
        self.capturing
    }
    fn dispatch(&self) -> u64 {
        assert!(self.callback, "dispatch without a callback");
        self.trace.borrow_mut().push(Step::Dispatch);
        self.result
    }
}

#[test]
fn mirrors_null_output() {
    let mut logs = 0;
    assert_eq!(query(None::<&Mock>, || logs += 1), 0);
    assert_eq!(logs, 1);
}

#[test]
fn mirrors_callback_delay_matrix() {
    use Step::*;
    let results = [0, 1, 0x1_0000_0000, 0xfedc_ba98_7654_3210, u64::MAX];
    for callback in [false, true] {
        for active in [false, true] {
            for capturing in [false, true] {
                for result in results {
                    let host = Mock::new(callback, active, capturing, result);
                    let got = query(Some(&host), || panic!("unexpected log"));
                    let trace = host.trace.into_inner();
                    let (expected, steps): (u64, &[Step]) = match (callback, active, capturing) {
                        (false, _, _) => (0, &[Callback]),
                        (true, false, _) => (result, &[Callback, Active, Dispatch]),
                        (true, true, false) => (0, &[Callback, Active, Capturing]),
                        (true, true, true) => (result, &[Callback, Active, Capturing, Dispatch]),
                    };
                    assert_eq!(
                        (got, trace.as_slice()),
                        (expected, steps),
                        "callback={callback} active={active} capturing={capturing} result={result:#x}"
                    );
                }
            }
        }
    }
}
