use obs_output::encoder_reroute::{Environment, create};

#[derive(Clone, Debug, PartialEq)]
struct Info {
    id: &'static str,
    kind: i32,
    codec: &'static str,
    caps: u32,
    callback: usize,
}
struct Model {
    encoder: bool,
    id: Option<&'static str>,
    allowed: bool,
    original: Info,
    active: Info,
    candidate: Info,
    logs: Vec<bool>,
    calls: usize,
    fail: bool,
}
impl Model {
    fn new() -> Self {
        let original = Info { id: "original", kind: 0, codec: "h264", caps: 0, callback: 1 };
        Self { encoder: true, id: Some("replacement"), allowed: true, active: original.clone(), original,
            candidate: Info { id: "replacement", kind: 0, codec: "H264", caps: 1, callback: 2 },
            logs: vec![], calls: 0, fail: false }
    }
}
impl Environment for Model {
    type Candidate = Info;
    type Output = Option<usize>;
    fn encoder_present(&self) -> bool { self.encoder }
    fn id_present(&self) -> bool { self.id.is_some() }
    fn null_parameter(&mut self, encoder: bool) { self.logs.push(encoder); }
    fn allowed(&self) -> bool { self.allowed }
    fn lookup(&self) -> Option<Info> { (self.id == Some("replacement")).then(|| self.candidate.clone()) }
    fn kinds(&self, candidate: &Info) -> (i32, i32) { (self.original.kind, candidate.kind) }
    fn codecs_equal(&self, candidate: &Info) -> bool { self.original.codec.eq_ignore_ascii_case(candidate.codec) }
    fn install_create(&mut self, candidate: Info) -> Option<usize> {
        self.active = candidate;
        self.calls += 1;
        assert_eq!(self.active, self.candidate); // whole table before callback
        assert_eq!(self.original.id, "original");
        assert_eq!(self.original.codec, "h264");
        // Current settings/encoder tokens are forwarded by the boundary;
        // the safe model checks the callback result is propagated unchanged.
        (!self.fail).then_some(73)
    }
}
#[test]
fn original_create_invalid_selection_and_admission() {
    for case in 0..7 {
        let mut env = Model::new();
        match case { 0 => env.encoder = false, 1 => env.id = None, 2 => env.id = Some(""),
            3 => env.id = Some("missing"), 4 => env.candidate.kind = 1,
            5 => env.candidate.codec = "hevc", _ => env.allowed = false }
        let before = env.active.clone();
        assert_eq!(create(&mut env), None);
        assert_eq!(env.active, before);
        assert_eq!(env.calls, 0);
        assert_eq!(env.logs, match case { 0 => vec![true], 1 => vec![false], _ => vec![] });
    }
}
#[test]
fn run_reroute_successful_and_failed_replacement() {
    for fail in [false, true] {
        let mut env = Model::new();
        env.fail = fail;
        let original = env.original.clone();
        assert_eq!(create(&mut env), Some((!fail).then_some(73)));
        assert_eq!(env.calls, 1);
        assert_eq!(env.active, env.candidate);
        assert_eq!(env.original, original);
        env.allowed = false; // same rule before/after create and on another thread
        let before = env.active.clone();
        assert_eq!(create(&mut env), None);
        assert_eq!(env.active, before);
        assert_eq!(env.calls, 1);
    }
}
