use obs_graphics::vertexbuffer_access::{Backend, Error, flush, get_data};

#[derive(Default)]
struct Device {
    calls: Vec<(bool, usize)>,
    result: Option<usize>,
}

impl Backend<usize, Option<usize>> for Device {
    fn flush(&mut self, buffer: usize) {
        self.calls.push((false, buffer));
    }

    fn get_data(&mut self, buffer: usize) -> Option<usize> {
        self.calls.push((true, buffer));
        self.result
    }
}

#[test]
fn context_precedes_buffer_validation() {
    for buffer in [None, Some(7)] {
        assert_eq!(
            flush::<_, Option<usize>, Device>(None, buffer),
            Err(Error::NoContext)
        );
        assert_eq!(
            get_data::<_, _, Device>(None, buffer),
            Err(Error::NoContext)
        );
    }
    let mut device = Device::default();
    assert_eq!(flush(Some(&mut device), None), Err(Error::NullBuffer));
    assert_eq!(get_data(Some(&mut device), None), Err(Error::NullBuffer));
    assert!(device.calls.is_empty());
}

#[test]
fn test_graphics_immediate_render_flush_and_get_data() {
    for handle in [1, 17, usize::MAX] {
        for result in [None, Some(42)] {
            let mut device = Device {
                result,
                ..Device::default()
            };
            assert_eq!(get_data(Some(&mut device), Some(handle)), Ok(result));
            assert_eq!(flush(Some(&mut device), Some(handle)), Ok(()));
            assert_eq!(device.calls, [(true, handle), (false, handle)]);
        }
    }
}
