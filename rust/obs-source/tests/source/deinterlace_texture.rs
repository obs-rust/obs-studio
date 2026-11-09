//! 1:1 assertions from test_deinterlace_texture_allocation.c.
use obs_source::deinterlace_texture::{allocate, Allocator, Input, Plane};
use obs_source::conversion_plan::{video_format as vf, gs_color_format as gs};

struct Recording { events: Vec<(usize, Plane, u32, u32)>, render: Option<(i32,i32)>, textures: [usize;4], renderer: usize, fail: i32, fail_render: bool }
impl Allocator for Recording {
    fn render(&mut self, format: i32, zs: i32) {
        assert!(self.events.is_empty()); assert!(self.render.is_none());
        self.render = Some((format,zs)); self.renderer = usize::from(!self.fail_render);
    }
    fn texture(&mut self, index: usize, plane: Plane, levels: u32, flags: u32) {
        assert_eq!(index,self.events.len());
        self.events.push((index,plane,levels,flags));
        self.textures[index] = if i32::try_from(index).unwrap() == self.fail {0} else {index+1};
    }
}
fn check(gpu: bool, channels: i32, format: i32, trc: u8, expected: i32, fail: i32, fail_render: bool) {
    let planes = std::array::from_fn(|c| Plane { width: 641-u32::try_from(c).unwrap()*101, height:479-u32::try_from(c).unwrap()*71, format:if c%2==0 {gs::R8G8} else {gs::R8} });
    let input = Input {gpu,channels,format,trc,width:1920,height:1080,planes};
    let mut r = Recording { events:vec![],render:None,textures:[99;4],renderer:99,fail,fail_render };
    allocate(input,&mut r);
    let count = if gpu {usize::try_from(channels.max(0)).unwrap()} else {1};
    assert_eq!(r.events.len(),count);
    assert_eq!(r.render,if gpu {Some((expected,0))} else {None});
    assert_eq!(r.renderer,if gpu {usize::from(!fail_render)} else {99});
    for (c,handle) in r.textures.iter().enumerate() {
        if c>=count {assert_eq!(*handle,99);continue;}
        let (_,plane,levels,flags)=r.events[c];
        assert_eq!(plane,if gpu {planes[c]} else {Plane{width:1920,height:1080,format:expected}});
        assert_eq!(levels,1); assert_eq!(flags,2);
        assert_eq!(*handle,if i32::try_from(c).unwrap()==fail {0} else {c+1});
    }
}
#[test]
fn format_selection_c_characterization() {
    let formats = [(vf::RGBA,gs::RGBA),(vf::BGRA,gs::BGRA),(vf::I40A,gs::BGRA),(vf::I42A,gs::BGRA),(vf::YUVA,gs::BGRA),(vf::AYUV,gs::BGRA),(vf::I010,gs::RGBA16F),(vf::P010,gs::RGBA16F),(vf::I210,gs::RGBA16F),(vf::I412,gs::RGBA16F),(vf::YA2L,gs::RGBA16F),(vf::P216,gs::RGBA16F),(vf::P416,gs::RGBA16F),(vf::V210,gs::RGBA16F),(vf::R10L,gs::RGBA16F),(vf::I420,gs::BGRX),(vf::NONE,gs::BGRX)];
    for (format,expected) in formats {for trc in 0..4 {for gpu in [false,true] {check(gpu,3,format,trc,if trc>=2 {gs::RGBA16F} else {expected},-1,false);}}}
}
#[test]
fn channels_and_failures_c_characterization() {
    for channels in 0..=4 {for gpu in [false,true] {for fail in -1..4 {for render in [false,true] {check(gpu,channels,vf::RGBA,0,gs::RGBA,fail,render);}}}}
}
#[test]
fn negative_gpu_channels_allocate_only_render() {check(true,-1,vf::RGBA,0,gs::RGBA,-1,false);}
