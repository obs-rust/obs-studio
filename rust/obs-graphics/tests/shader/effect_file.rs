use obs_graphics::effect_file::{Diagnostic, Model, create_from_file};

#[derive(Default)]
struct Fixture {
    context: bool,
    paths: Vec<Vec<u8>>,
    file: Vec<u8>,
    read_ok: bool,
    create_ok: bool,
    events: String,
    logs: Vec<Diagnostic>,
    error: usize,
    with_error: bool,
}
impl Model for Fixture {
    type File = ();
    type Effect = usize;
    type Text = ();
    fn context_present(&mut self) -> bool {
        self.context
    }
    fn first(&mut self) -> Option<usize> {
        (!self.paths.is_empty()).then_some(0)
    }
    fn next(&mut self, i: usize) -> Option<usize> {
        (i + 1 < self.paths.len()).then_some(i + 1)
    }
    fn file_path(&self, _: ()) -> &[u8] {
        &self.file
    }
    fn effect_path(&self, i: usize) -> &[u8] {
        &self.paths[i]
    }
    fn read(&mut self, _: ()) -> Option<()> {
        self.events.push('R');
        self.read_ok.then_some(())
    }
    fn create(&mut self, _: (), _: ()) -> Option<usize> {
        self.events.push('C');
        if self.with_error {
            self.error = usize::from(!self.create_ok);
        }
        self.create_ok.then_some(7)
    }
    fn free(&mut self, _: ()) {
        self.events.push('F');
    }
    fn log(&mut self, d: Diagnostic, _: Option<()>) {
        self.events.push('L');
        self.logs.push(d);
    }
}
#[test]
fn test_effect_file_validation() {
    let mut f = Fixture {
        error: 9,
        ..Fixture::default()
    };
    assert_eq!(create_from_file(&mut f, None), None);
    assert_eq!(f.logs, [Diagnostic::Context]);
    assert_eq!(f.events, "L");
    assert_eq!(f.error, 9);
    f.context = true;
    f.logs.clear();
    f.events.clear();
    assert_eq!(create_from_file(&mut f, None), None);
    assert_eq!(f.logs, [Diagnostic::File]);
    assert_eq!(f.events, "L");
    assert_eq!(f.error, 9);
}
#[test]
fn test_effect_file_cache() {
    let mut f = Fixture {
        context: true,
        paths: vec![
            b"CASE.effect".to_vec(),
            b"case.effect".to_vec(),
            b"case.effect".to_vec(),
        ],
        file: b"case.effect".to_vec(),
        error: 9,
        ..Fixture::default()
    };
    assert_eq!(create_from_file(&mut f, Some(())), Some(1));
    f.file = b"CASE.effect".to_vec();
    assert_eq!(create_from_file(&mut f, Some(())), Some(0));
    assert_eq!(f.error, 9);
    assert!(f.events.is_empty());
}
#[test]
fn test_effect_file_reads_and_constructor() {
    for read_ok in [false, true] {
        for create_ok in [false, true] {
            for with_error in [false, true] {
                let mut f = Fixture {
                    context: true,
                    read_ok,
                    create_ok,
                    with_error,
                    error: 9,
                    ..Fixture::default()
                };
                assert_eq!(
                    create_from_file(&mut f, Some(())),
                    (read_ok && create_ok).then_some(7)
                );
                assert_eq!(f.events, if read_ok { "RCF" } else { "RL" });
                assert_eq!(
                    f.error,
                    if read_ok && with_error {
                        usize::from(!create_ok)
                    } else {
                        9
                    }
                );
                assert_eq!(
                    f.logs,
                    if read_ok {
                        vec![]
                    } else {
                        vec![Diagnostic::Read]
                    }
                );
            }
        }
    }
}
#[test]
fn missing_and_non_utf8_paths() {
    let mut f = Fixture {
        context: true,
        paths: vec![vec![0xff], vec![0xc3, 0xa9]],
        file: vec![0xff],
        ..Fixture::default()
    };
    assert_eq!(create_from_file(&mut f, Some(())), Some(0));
    f.file = vec![];
    assert_eq!(create_from_file(&mut f, Some(())), None);
    assert_eq!(f.events, "RL");
}
