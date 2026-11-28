use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Duplicate,
    Invalid,
    Stopped,
}

enum Kind {
    Fd(i32),
    Timer,
}

struct Entry<H> {
    kind: Kind,
    handler: H,
}

pub struct Registry<H> {
    next: u64,
    entries: BTreeMap<u64, Entry<H>>,
    descriptors: BTreeMap<i32, u64>,
    stopped: bool,
}

impl<H: Clone + PartialEq> Registry<H> {
    pub fn new() -> Self {
        Self {
            next: 0,
            entries: BTreeMap::new(),
            descriptors: BTreeMap::new(),
            stopped: false,
        }
    }

    fn insert(&mut self, kind: Kind, handler: H) -> Result<u64, Error> {
        if self.stopped {
            return Err(Error::Stopped);
        }
        self.next = self.next.checked_add(1).ok_or(Error::Stopped)?;
        let token = self.next;
        if let Kind::Fd(fd) = kind {
            self.descriptors.insert(fd, token);
        }
        self.entries.insert(token, Entry { kind, handler });
        Ok(token)
    }

    pub fn register_fd(&mut self, fd: i32, handler: H) -> Result<u64, Error> {
        if fd < 0 {
            return Err(Error::Invalid);
        }
        if self.descriptors.contains_key(&fd) {
            return Err(Error::Duplicate);
        }
        self.insert(Kind::Fd(fd), handler)
    }

    pub fn register_timer(&mut self, interval: u64, handler: H) -> Result<u64, Error> {
        if interval == 0 {
            return Err(Error::Invalid);
        }
        if self
            .entries
            .values()
            .any(|entry| matches!(entry.kind, Kind::Timer) && entry.handler == handler)
        {
            return Err(Error::Duplicate);
        }
        self.insert(Kind::Timer, handler)
    }

    pub fn unregister_fd_handler(&mut self, handler: &H) -> Vec<(u64, H)> {
        let descriptors: Vec<_> = self
            .descriptors
            .iter()
            .filter_map(|(&fd, &token)| {
                self.entries
                    .get(&token)
                    .filter(|entry| &entry.handler == handler)
                    .map(|_| (fd, token))
            })
            .collect();
        descriptors
            .into_iter()
            .filter_map(|(fd, token)| {
                self.descriptors.remove(&fd);
                self.entries
                    .remove(&token)
                    .map(|entry| (token, entry.handler))
            })
            .collect()
    }

    pub fn unregister_timer(&mut self, handler: &H) -> Option<(u64, H)> {
        let (&token, _) = self
            .entries
            .iter()
            .find(|(_, entry)| matches!(entry.kind, Kind::Timer) && &entry.handler == handler)?;
        self.entries
            .remove(&token)
            .map(|entry| (token, entry.handler))
    }

    pub fn dispatch(&self, token: u64) -> Option<H> {
        self.entries.get(&token).map(|entry| entry.handler.clone())
    }

    pub fn stop(&mut self) {
        self.stopped = true;
        self.descriptors.clear();
        self.entries.clear();
    }
}

impl<H: Clone + PartialEq> Default for Registry<H> {
    fn default() -> Self {
        Self::new()
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
