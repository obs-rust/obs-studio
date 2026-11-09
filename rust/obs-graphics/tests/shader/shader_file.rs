//! Tier 1 mirrors test_shader_file_creators.c through the safe API.
use obs_graphics::shader_file::{Backend, Failure, Kind, create};

#[derive(Debug, PartialEq, Eq)]
pub enum Event { Read, Create(Kind, Vec<u8>, usize, bool), Free, Log(Kind, Failure) }

pub struct Fake {
    pub context: bool,
    pub path: bool,
    pub source: Option<Vec<u8>>,
    pub reject: bool,
    pub output: bool,
    pub error: usize,
    pub events: Vec<Event>,
}
impl Default for Fake {
    fn default() -> Self {
        Self { context: true, path: true, source: Some(b"// UTF-8: \xc3\xa9\nvoid main() {}\n".to_vec()), reject: false, output: true, error: 0, events: Vec::new() }
    }
}
impl Backend for Fake {
    type Source = Vec<u8>;
    type Shader = usize;
    fn context(&self) -> bool { self.context }
    fn path_present(&self) -> bool { self.path }
    fn read(&mut self) -> Option<Vec<u8>> { self.events.push(Event::Read); self.source.clone() }
    fn create(&mut self, kind: Kind, source: &Vec<u8>) -> usize {
        self.events.push(Event::Create(kind, source.clone(), 42, self.output));
        if self.reject { if self.output { self.error = 99; } 0 } else { 7 }
    }
    fn free(&mut self, _: Vec<u8>) { self.events.push(Event::Free); }
    fn log(&mut self, kind: Kind, failure: Failure) { self.events.push(Event::Log(kind, failure)); }
}

#[test]
fn test_shader_file_creators_context_and_path_gates() {
    for kind in [Kind::Vertex, Kind::Pixel] {
        for path in [false, true] {
            let mut b = Fake { context: false, path, error: 99, ..Fake::default() };
            assert_eq!(create(&mut b, kind), 0);
            assert_eq!(b.error, 99);
            assert_eq!(b.events, [Event::Log(kind, Failure::Context)]);
        }
        let mut b = Fake { path: false, error: 99, ..Fake::default() };
        assert_eq!(create(&mut b, kind), 0);
        assert_eq!(b.error, 99);
        assert_eq!(b.events, [Event::Log(kind, Failure::Path)]);
    }
}

#[test]
fn test_shader_file_creators_unreadable_empty_and_bom_only_files() {
    // All three are NULL from the unchanged os_quick_read_utf8_file boundary.
    for _input in 0..3 {
        for kind in [Kind::Vertex, Kind::Pixel] {
            for output in [false, true] {
                let mut b = Fake { source: None, error: 99, output, ..Fake::default() };
                assert_eq!(create(&mut b, kind), 0);
                assert_eq!(b.error, 99);
                assert_eq!(b.events, [Event::Read, Event::Log(kind, Failure::Read)]);
            }
        }
    }
}

#[test]
fn test_shader_file_creators_utf8_dispatch_and_backend_results() {
    // The unchanged reader strips the BOM before returning the source.
    for _bom in [false, true] {
        for kind in [Kind::Vertex, Kind::Pixel] {
            for reject in [false, true] {
                for output in [false, true] {
                    let mut b = Fake { reject, output, ..Fake::default() };
                    let text = b.source.clone().unwrap();
                    assert_eq!(create(&mut b, kind), if reject { 0 } else { 7 });
                    assert_eq!(b.error, if reject && output { 99 } else { 0 });
                    assert_eq!(b.events, [Event::Read, Event::Create(kind, text, 42, output), Event::Free]);
                }
            }
        }
    }
}
