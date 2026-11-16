//! Safe counterparts of test_cubetexture_access.c, including backend ownership.
use obs_graphics::cubetexture_access::{Host, Operation, access};
#[derive(Default)]
struct Fixture {
    context: bool,
    calls: [u32; 3],
    seen: Option<usize>,
    size: u32,
    color: u32,
    token: u8,
    logs: Vec<(&'static str, &'static str)>,
}
impl Host<usize> for Fixture {
    fn has_context(&self) -> bool {
        self.context
    }
    fn debug(&mut self, function: &'static str, reason: &'static str) {
        self.logs.push((function, reason));
    }
    fn dispatch(&mut self, operation: Operation, texture: usize) -> u32 {
        self.calls[operation as usize] += 1;
        self.seen = Some(texture);
        match operation {
            Operation::Size => self.size,
            Operation::ColorFormat => self.color,
            Operation::Destroy => {
                assert_eq!(self.token, 0x5a);
                self.token = 0xa5;
                0
            }
        }
    }
}
#[test]
fn test_cubetexture_access_rejected_inputs() {
    for active in [false, true] {
        for token in [None, Some(0x1234)] {
            if active && token.is_some() {
                continue;
            }
            for operation in [Operation::Size, Operation::Destroy, Operation::ColorFormat] {
                let mut host = Fixture {
                    context: active,
                    token: 0x5a,
                    ..Fixture::default()
                };
                assert_eq!(access(&mut host, token, operation), 0);
                if active && operation == Operation::Destroy {
                    assert!(host.logs.is_empty());
                } else {
                    assert_eq!(
                        host.logs,
                        [(
                            operation.function(),
                            if active {
                                "Null 'cubetex' parameter"
                            } else {
                                "called while not in a graphics context"
                            }
                        )]
                    );
                }
                assert_eq!(host.calls, [0; 3]);
                assert_eq!(host.seen, None);
                assert_eq!(host.token, 0x5a);
            }
        }
    }
}
#[test]
fn test_cubetexture_access_backend_results() {
    let mut host = Fixture {
        context: true,
        token: 0x5a,
        ..Fixture::default()
    };
    for (i, size) in [0, 1, 1920, u32::MAX].into_iter().enumerate() {
        host.size = size;
        host.color = [0, 3, 2, 9][i];
        assert_eq!(access(&mut host, Some(0x1234), Operation::Size), size);
        assert_eq!(host.calls, [(i + 1) as u32, 0, i as u32]);
        assert_eq!(host.token, 0x5a);
        assert_eq!(
            access(&mut host, Some(0x1234), Operation::ColorFormat),
            host.color
        );
        assert_eq!(host.calls, [(i + 1) as u32, 0, (i + 1) as u32]);
        assert_eq!(host.seen, Some(0x1234));
        assert!(host.logs.is_empty());
    }
    access(&mut host, Some(0x1234), Operation::Destroy);
    assert_eq!(host.calls, [4, 1, 4]);
    assert_eq!(host.token, 0xa5);
    assert!(host.logs.is_empty());
}
