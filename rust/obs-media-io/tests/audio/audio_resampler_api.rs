//! Tier 1: wrapper decisions, using a scripted kernel host. Actual FFmpeg
//! sample fidelity remains covered by unchanged test_audio_resampler_api.c.
use obs_media_io::audio_resampler::{Event, Host, Info, Resampler};

#[derive(Default)]
struct Kernel {
    calls: Vec<String>,
    result: i32,
    delay: i64,
    init_error: i32,
    comp_error: i32,
    allocation_fails: bool,
    matrix_error: i32,
}
impl Host for Kernel {
    type Input = ();
    fn alloc(&mut self, dst: Info, src: Info) -> bool {
        self.calls.push(format!("alloc {dst:?} {src:?}"));
        !self.allocation_fails
    }
    fn matrix(&mut self, row: &[f64; 8]) -> i32 {
        self.calls.push(format!("matrix {row:?}"));
        self.matrix_error
    }
    fn init(&mut self) -> i32 { self.init_error }
    fn compensate(&mut self, delta: i32, distance: i32) -> i32 {
        self.calls.push(format!("comp {delta} {distance}"));
        self.comp_error
    }
    fn delay(&mut self, base: i64) -> i64 {
        if base == 1_000_000_000 { self.delay * 1_000_000_000 / 48000 } else { self.delay }
    }
    fn rescale(&mut self, v: i64, n: i64, d: i64) -> i64 { (v * n + d - 1) / d }
    fn resize(&mut self, channels: u32, frames: i32, format: i32) {
        self.calls.push(format!("resize {channels} {frames} {format}"));
    }
    fn convert(&mut self, capacity: i32, _: (), frames: i32) -> i32 {
        self.calls.push(format!("convert {capacity} {frames}"));
        self.result
    }
    fn log(&mut self, event: Event) { self.calls.push(format!("log {event:?}")); }
}
fn info(rate: u32, format: i32, speakers: i32) -> Info { Info { rate, format, speakers } }
fn stereo(format: i32) -> Info { info(48000, format, 2) }

#[test]
fn invalid_sample_rate() {
    // FFmpeg owns validation; both directions must propagate its error.
    for (src, dst) in [(0,48000),(48000,0)] {
        assert!(Resampler::create(Kernel { init_error: -22, ..Kernel::default() }, info(dst,4,2), info(src,4,2)).is_none());
    }
}
#[test]
fn unknown_format_is_s16() {
    for (src,dst) in [(0,4),(4,0)] {
        let mut rs=Resampler::create(Kernel { result:5, ..Kernel::default() }, info(48000,dst,1), info(48000,src,1)).unwrap();
        let mut offset=u64::MAX;
        assert_eq!(rs.resample((),5,&mut offset),Some(5));
        assert_eq!(offset,0);
        assert!(rs.host.calls.contains(&format!("resize 1 5 {}", if dst==0 {1} else {3})));
        assert_eq!(rs.planes,1);
    }
}
#[test]
fn planar_to_packed_and_buffer_growth() {
    let mut rs=Resampler::create(Kernel::default(),stereo(4),stereo(8)).unwrap();
    for frames in [16,1024,32,1024] {
        rs.host.result=frames;
        rs.set_error(0);
        let mut offset=u64::MAX;
        assert_eq!(rs.resample((),u32::try_from(frames).unwrap(),&mut offset),Some(u32::try_from(frames).unwrap()));
        assert_eq!(offset,0);
        assert_eq!(rs.planes,1);
    }
    assert_eq!(rs.host.calls.iter().filter(|s| s.starts_with("resize")).collect::<Vec<_>>(),vec!["resize 2 16 3","resize 2 1024 3"]);
}
#[test]
fn packed_to_planar() {
    let mut rs=Resampler::create(Kernel {result:3,..Kernel::default()},stereo(8),stereo(4)).unwrap();
    let mut offset=u64::MAX;
    assert_eq!(rs.resample((),3,&mut offset),Some(3));
    assert_eq!(offset,0);
    assert_eq!(rs.planes,2);
    assert!(rs.host.calls.contains(&"resize 2 3 8".into()));
}
#[test]
fn mono_upmix() {
    let mut rs=Resampler::create(Kernel {result:5,..Kernel::default()},stereo(8),info(48000,4,1)).unwrap();
    assert!(rs.host.calls.contains(&"matrix [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]".into()));
    let mut offset=u64::MAX;
    assert_eq!(rs.resample((),5,&mut offset),Some(5));
    assert_eq!(offset,0);
    assert_eq!(rs.planes,2);
}
#[test]
fn rate_conversion_delay() {
    let mut rs=Resampler::create(Kernel::default(),info(24000,4,1),info(48000,4,1)).unwrap();
    let mut total=0;
    for call in 0..100 {
        rs.host.delay=if call==0 {0} else {32};
        rs.host.result=if call==0 {224} else {240};
        let mut offset=u64::MAX;
        let frames=rs.resample((),480,&mut offset).unwrap();
        assert!(frames>0 && frames<=240);
        if call==0 {assert_eq!(offset,0)} else {assert!(offset>0 && offset<2_000_000)}
        total+=frames;
    }
    assert!((24000-64..24000).contains(&total));
    assert!(rs.host.calls.contains(&"resize 1 256 3".into()));
}
#[test]
fn errors_and_compensation_bookkeeping() {
    assert!(Resampler::create(Kernel {allocation_fails:true,..Kernel::default()},stereo(4),stereo(4)).is_none());
    let mut rs=Resampler::create(Kernel {result:-5,..Kernel::default()},stereo(4),stereo(4)).unwrap();
    rs.set_error(10_000_000);
    let mut offset=123;
    rs.host.comp_error=-22;
    assert_eq!(rs.resample((),480,&mut offset),None);
    assert_eq!(offset,123);
    rs.host.comp_error=0;
    assert_eq!(rs.resample((),480,&mut offset),None);
    assert_eq!(offset,0);
    assert_eq!(rs.compensation.output_samples,0);
    rs.host.result=480;
    assert_eq!(rs.resample((),480,&mut offset),Some(480));
    assert_eq!(rs.compensation.input_samples,480);
    assert_eq!(rs.compensation.output_samples,480);
    assert!(rs.host.calls.contains(&"resize 2 482 3".into()));
}
