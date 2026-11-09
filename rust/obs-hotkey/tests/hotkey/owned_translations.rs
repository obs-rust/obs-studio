//! Safe mirrors of test_hotkey_owned_translations.c's stored-description assertions.
use obs_hotkey::owned_translations::{Storage, set_audio, set_sceneitem};

#[derive(Default)]
struct Slots([Option<Vec<u8>>; 8]);
struct Store<'s, 'a>(&'s mut Slots, std::marker::PhantomData<&'a [u8]>);
impl<'a> Storage for Store<'_, 'a> {
    type Input = Option<&'a [u8]>;
    type Owned = Option<Vec<u8>>;
    fn release(&mut self, slot: usize) {
        self.0.0[slot] = None;
    }
    fn duplicate(&mut self, value: Self::Input) -> Self::Owned {
        value.map(<[u8]>::to_vec)
    }
    fn write(&mut self, slot: usize, value: Self::Owned) {
        self.0.0[slot] = value;
    }
}
impl Slots {
    fn audio(&mut self, values: [Option<&[u8]>; 6]) {
        set_audio(&mut Store(self, std::marker::PhantomData), values);
    }
    fn scene(&mut self, values: [Option<&[u8]>; 2]) {
        set_sceneitem(&mut Store(self, std::marker::PhantomData), values);
    }
    fn audio_snapshot(&self) -> [Option<Vec<u8>>; 6] {
        std::array::from_fn(|i| self.0[i].clone())
    }
    fn scene_snapshot(&self) -> [Option<Vec<u8>>; 2] {
        std::array::from_fn(|i| self.0[i + 6].clone())
    }
    fn registered_scene(&self) -> [Option<Vec<u8>>; 2] {
        // Scene registration uses these same already-ported dstr operations.
        // Keep the C test's formatted-description assertions as well as the
        // raw stored-template assertions for the setter itself.
        std::array::from_fn(|i| {
            let mut description = obs_util::dstr::Dstr::new();
            description.copy(self.0[i + 6].as_deref().unwrap_or_default());
            description.replace(b"%1", b"item");
            (description.capacity() != 0).then(|| description.as_bytes().to_vec())
        })
    }
}
fn audio(values: [&str; 6]) -> [Option<Vec<u8>>; 6] {
    values.map(|v| Some(v.as_bytes().to_vec()))
}

#[test]
fn mirrors_c_audio_copies_and_replaces() {
    let mut slots = Slots::default();
    let mut labels = audio([
        "Mute",
        "Unmute",
        "Push mute",
        "Push talk",
        "Monitor on",
        "Monitor off",
    ]);
    slots.audio(labels.each_ref().map(|v| v.as_deref()));
    for label in labels.iter_mut().flatten() {
        label.fill(b'x');
    }
    let first = audio([
        "Mute",
        "Unmute",
        "Push mute",
        "Push talk",
        "Monitor on",
        "Monitor off",
    ]);
    let old = slots.audio_snapshot();
    assert_eq!(old, first);
    let second = audio(["M2", "U2", "PM2", "PT2", "ON2", "OFF2"]);
    slots.audio(second.each_ref().map(|v| v.as_deref()));
    assert_eq!(slots.audio_snapshot(), second);
    assert_eq!(old, first);
}

#[test]
fn mirrors_c_scene_copies_replaces_and_formats() {
    let mut slots = Slots::default();
    let mut show = b"Show %1 / %1".to_vec();
    let mut hide = b"Hide %1".to_vec();
    slots.scene([Some(&show), Some(&hide)]);
    show.fill(b'x');
    hide.fill(b'y');
    // These setters store templates; substitution belongs to scene registration.
    assert_eq!(
        slots.scene_snapshot(),
        [Some(b"Show %1 / %1".to_vec()), Some(b"Hide %1".to_vec())]
    );
    assert_eq!(
        slots.registered_scene(),
        [
            Some(b"Show item / item".to_vec()),
            Some(b"Hide item".to_vec())
        ]
    );
    slots.scene([Some(b"Visible %1"), Some(b"Hidden %1")]);
    assert_eq!(
        slots.scene_snapshot(),
        [Some(b"Visible %1".to_vec()), Some(b"Hidden %1".to_vec())]
    );
    assert_eq!(
        slots.registered_scene(),
        [
            Some(b"Visible item".to_vec()),
            Some(b"Hidden item".to_vec())
        ]
    );
}

#[test]
fn mirrors_c_null_empty_and_group_isolation() {
    let mut slots = Slots::default();
    let restored = audio(["M", "U", "PM", "PT", "ON", "OFF"]);
    slots.audio(restored.each_ref().map(|v| v.as_deref()));
    slots.scene([Some(b"Show %1"), Some(b"Hide %1")]);
    let mixed = [
        None,
        Some(b"".as_slice()),
        None,
        Some(b"".as_slice()),
        None,
        Some(b"".as_slice()),
    ];
    slots.audio(mixed);
    assert_eq!(slots.audio_snapshot(), mixed.map(|v| v.map(<[u8]>::to_vec)));
    assert_eq!(
        slots.scene_snapshot(),
        [Some(b"Show %1".to_vec()), Some(b"Hide %1".to_vec())]
    );
    assert_eq!(
        slots.registered_scene(),
        [Some(b"Show item".to_vec()), Some(b"Hide item".to_vec())]
    );
    slots.scene([None, Some(b"")]);
    assert_eq!(slots.scene_snapshot(), [None, Some(vec![])]);
    assert_eq!(slots.registered_scene(), [None, None]);
    assert_eq!(slots.audio_snapshot(), mixed.map(|v| v.map(<[u8]>::to_vec)));
    slots.audio([None; 6]);
    assert_eq!(slots.audio_snapshot(), [const { None }; 6]);
    slots.audio([Some(b""); 6]);
    assert_eq!(
        slots.audio_snapshot(),
        std::array::from_fn(|_| Some(vec![]))
    );
    slots.audio(restored.each_ref().map(|v| v.as_deref()));
    assert_eq!(slots.audio_snapshot(), restored);
    slots.scene([Some(b"Restored %1"), Some(b"Hidden %1")]);
    assert_eq!(
        slots.scene_snapshot(),
        [Some(b"Restored %1".to_vec()), Some(b"Hidden %1".to_vec())]
    );
    assert_eq!(
        slots.registered_scene(),
        [
            Some(b"Restored item".to_vec()),
            Some(b"Hidden item".to_vec())
        ]
    );
    assert_eq!(slots.audio_snapshot(), restored);
}

#[test]
fn releases_then_duplicates_then_writes_in_c_slot_order() {
    struct Trace(Vec<(&'static str, usize)>);
    impl Storage for Trace {
        type Input = usize;
        type Owned = usize;
        fn release(&mut self, slot: usize) {
            self.0.push(("free", slot));
        }
        fn duplicate(&mut self, value: usize) -> usize {
            self.0.push(("copy", value));
            value
        }
        fn write(&mut self, slot: usize, value: usize) {
            assert_eq!(slot, value);
            self.0.push(("write", slot));
        }
    }
    let mut trace = Trace(Vec::new());
    set_audio(&mut trace, [0, 1, 2, 3, 4, 5]);
    set_sceneitem(&mut trace, [6, 7]);
    let expected: Vec<_> = (0..8)
        .flat_map(|i| [("free", i), ("copy", i), ("write", i)])
        .collect();
    assert_eq!(trace.0, expected);
}
