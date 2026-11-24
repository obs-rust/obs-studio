//! Sidechain lookup and attachment state shared by OBS update and video tick.

#[derive(Debug, PartialEq, Eq)]
pub struct Lookup {
    pub name: String,
    generation: u64,
}

pub struct Route<T> {
    name: String,
    generation: u64,
    last_lookup: Option<u64>,
    attached: Option<T>,
}

impl<T> Default for Route<T> {
    fn default() -> Self {
        Self { name: String::new(), generation: 0, last_lookup: None, attached: None }
    }
}

impl<T> Route<T> {
    pub fn select(&mut self, _name: &str) -> Option<T> {
        todo!()
    }

    pub fn lookup(&mut self, _now_ns: u64) -> Option<Lookup> {
        todo!()
    }

    pub fn attach(&mut self, _lookup: &Lookup, _source: T) -> Result<(), T> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_lookup_is_retried_after_three_seconds() {
        let mut route = Route::<u8>::default();
        assert_eq!(route.select("voice"), None);
        let lookup = route.lookup(10).unwrap();
        assert_eq!(lookup.name, "voice");
        assert_eq!(route.lookup(3_000_000_010), None);
        let next = route.lookup(3_000_000_011).unwrap();
        assert_eq!(route.attach(&next, 4), Ok(()));
        assert_eq!(route.lookup(7_000_000_000), None);
        assert_eq!(route.select("voice"), None);
        assert_eq!(route.select("none"), Some(4));
        assert_eq!(route.lookup(8_000_000_000), None);
    }

    #[test]
    fn delayed_result_cannot_attach_after_selection_leaves_and_returns() {
        let mut route = Route::<u8>::default();
        route.select("voice");
        let old = route.lookup(100).unwrap();
        route.select("effects");
        route.select("voice");
        let current = route.lookup(102).unwrap();
        assert_eq!(route.attach(&old, 1), Err(1));
        assert_eq!(route.attach(&current, 2), Ok(()));
        assert_eq!(route.select("effects"), Some(2));
    }
}
