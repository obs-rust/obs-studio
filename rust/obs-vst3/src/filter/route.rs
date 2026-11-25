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
        Self {
            name: String::new(),
            generation: 0,
            last_lookup: None,
            attached: None,
        }
    }
}

impl<T> Route<T> {
    pub fn select(&mut self, name: &str) -> Option<T> {
        let name = if name == "none" { "" } else { name };
        if self.name == name {
            return None;
        }
        self.name = name.to_owned();
        self.generation = self.generation.wrapping_add(1);
        self.last_lookup = None;
        self.attached.take()
    }

    pub fn detach(&mut self) -> Option<T> {
        self.generation = self.generation.wrapping_add(1);
        self.attached.take()
    }

    pub fn lookup(&mut self, now_ns: u64) -> Option<Lookup> {
        const RETRY_NS: u64 = 3_000_000_000;
        if self.name.is_empty()
            || self.attached.is_some()
            || self
                .last_lookup
                .is_some_and(|last| now_ns.wrapping_sub(last) <= RETRY_NS)
        {
            return None;
        }
        self.last_lookup = Some(now_ns);
        Some(Lookup {
            name: self.name.clone(),
            generation: self.generation,
        })
    }

    pub fn attach(&mut self, lookup: &Lookup, source: T) -> Result<(), T> {
        if self.generation != lookup.generation || self.attached.is_some() {
            return Err(source);
        }
        self.attached = Some(source);
        Ok(())
    }
}
