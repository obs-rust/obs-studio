#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Duplicate,
    Invalid,
    Stopped,
}

pub struct Registry<H>(std::marker::PhantomData<H>);

impl<H: Clone + PartialEq> Registry<H> {
    pub fn new() -> Self {
        todo!()
    }
    pub fn register_fd(&mut self, _fd: i32, _handler: H) -> Result<u64, Error> {
        todo!()
    }
    pub fn register_timer(&mut self, _interval: u64, _handler: H) -> Result<u64, Error> {
        todo!()
    }
    pub fn unregister_fd_handler(&mut self, _handler: &H) -> Vec<(u64, H)> {
        todo!()
    }
    pub fn unregister_timer(&mut self, _handler: &H) -> Option<(u64, H)> {
        todo!()
    }
    pub fn dispatch(&self, _token: u64) -> Option<H> {
        todo!()
    }
    pub fn stop(&mut self) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, Registry};
    use std::rc::Rc;

    #[test]
    fn multiple_descriptors_duplicate_and_removal() {
        let mut run = Registry::new();
        let a = run.register_fd(1, 8).unwrap();
        let b = run.register_fd(2, 8).unwrap();
        assert_eq!(run.register_fd(1, 7), Err(Error::Duplicate));
        assert_eq!(run.register_fd(-1, 8), Err(Error::Invalid));
        assert_eq!(run.unregister_fd_handler(&8), [(a, 8), (b, 8)]);
        assert!(run.unregister_fd_handler(&8).is_empty());
    }

    #[test]
    fn duplicate_timers_and_unknown_removal() {
        let mut run = Registry::new();
        assert_eq!(run.register_timer(0, 8), Err(Error::Invalid));
        let a = run.register_timer(10, 8).unwrap();
        assert_eq!(run.register_timer(20, 8), Err(Error::Duplicate));
        assert_eq!(run.unregister_timer(&9), None);
        assert_eq!(run.unregister_timer(&8), Some((a, 8)));
        assert_eq!(run.dispatch(a), None);
    }

    #[test]
    fn reentrant_removal_retains_handler_and_reused_fd_rejects_stale_callback() {
        let mut run = Registry::new();
        let handler = Rc::new(8);
        let weak = Rc::downgrade(&handler);
        let old = run.register_fd(1, handler).unwrap();
        let invoking = run.dispatch(old).unwrap();
        let removed = run.unregister_fd_handler(&invoking);
        assert_eq!(removed[0].0, old);
        assert_eq!(weak.strong_count(), 2);
        drop(removed);
        assert_eq!(weak.strong_count(), 1);
        drop(invoking);
        assert!(weak.upgrade().is_none());
        let new = run.register_fd(1, Rc::new(9)).unwrap();
        assert_eq!(run.dispatch(old), None);
        assert_eq!(run.dispatch(new), Some(Rc::new(9)));
    }

    #[test]
    fn shutdown_invalidates_pending_callbacks() {
        let mut run = Registry::new();
        let fd = run.register_fd(1, 8).unwrap();
        let timer = run.register_timer(10, 8).unwrap();
        run.stop();
        assert_eq!(run.dispatch(fd), None);
        assert_eq!(run.dispatch(timer), None);
        assert_eq!(run.register_fd(1, 9), Err(Error::Stopped));
    }
}
