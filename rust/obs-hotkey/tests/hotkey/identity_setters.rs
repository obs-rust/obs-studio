//! Tier 1 mirrors set_name_and_description and pair_set_names_and_descriptions.
use obs_hotkey::identity_setters::{Field, Registry, set};

#[derive(Default)]
struct World {
    values: [[Option<Vec<u8>>; 2]; 2],
    events: Vec<&'static str>,
}
impl Registry for World {
    type Input = Option<Vec<u8>>;
    type Slot = (usize, usize);
    type Owned = Option<Vec<u8>>;
    fn find(&mut self, id: usize, field: Field) -> Option<Self::Slot> {
        (id < 2).then_some((id, usize::from(field == Field::Description)))
    }
    fn release(&mut self, &(id, field): &Self::Slot) {
        self.events.push("free");
        self.values[id][field] = None;
    }
    fn duplicate(&mut self, input: Self::Input) -> Self::Owned {
        self.events.push("duplicate");
        input
    }
    fn store(&mut self, (id, field): Self::Slot, value: Self::Owned) {
        self.events.push("store");
        self.values[id][field] = value;
    }
}
#[test]
fn set_name_and_description() {
    let mut world = World::default();
    for field in [Field::Name, Field::Description] {
        for input in [
            Some(b"old".to_vec()),
            Some(b"new".to_vec()),
            None,
            Some(vec![]),
            Some(vec![255]),
        ] {
            let index = usize::from(field == Field::Description);
            let other = world.values[0][1 - index].clone();
            world.events.clear();
            set(&mut world, 0, field, input.clone());
            assert_eq!(world.values[0][index], input);
            assert_eq!(world.values[0][1 - index], other);
            assert_eq!(world.events, ["free", "duplicate", "store"]);
        }
    }
    let before = world.values.clone();
    world.events.clear();
    set(&mut world, 100, Field::Name, Some(b"x".to_vec()));
    set(&mut world, usize::MAX, Field::Description, None);
    assert_eq!(world.values, before);
    assert!(world.events.is_empty());
}
#[test]
fn pair_set_names_and_descriptions() {
    let mut world = World::default();
    for (id, name, desc) in [(0, b"a2", b"A2"), (1, b"b2", b"B2")] {
        set(&mut world, id, Field::Name, Some(name.to_vec()));
        set(&mut world, id, Field::Description, Some(desc.to_vec()));
        assert_eq!(world.values[id], [Some(name.to_vec()), Some(desc.to_vec())]);
    }
}
