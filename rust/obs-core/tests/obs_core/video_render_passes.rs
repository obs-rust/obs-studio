//! Tier 1 mirrors test_video_render_passes.c, including all adapter arguments.
use obs_core::video_render_passes::{self as render, Operation};

pub(super) fn run(convert: bool, index: usize, passes: usize, width: u32, height: u32, name: usize) -> String {
    use Operation::*;
    let mut trace = String::new();
    let mut loop_index = 0;
    let mut pass_index = 0;
    let texture = 10 + index;
    let mut call = |op, a, b| {
        let (code, result) = match op {
            MixTexture => { assert_eq!((a, b), (index, 0)); ('M', texture) }
            DefaultEffect => { assert_eq!((a,b),(0,0)); ('E', 20) }
            ImageParameter => { assert_eq!((a,b),(20,0)); ('P', 30) }
            BindTexture => { assert_eq!((a,b),(30,texture)); ('I', 0) }
            PushBlend => { assert_eq!((a,b),(0,0)); ('[',0) }
            DisableBlend => { assert_eq!((a,b),(0,0)); ('B',0) }
            EnableSrgb => { assert_eq!((a,b),(0,0)); ('+',0) }
            EffectLoop => { assert_eq!((a,b),(20,0)); let more=loop_index<passes; loop_index+=1; ('L',usize::from(more)) }
            Sprite => { assert_eq!((a,b),(texture,0)); ('S',0) }
            DisableSrgb => { assert_eq!((a,b),(0,0)); ('-',0) }
            PopBlend => { assert_eq!((a,b),(0,0)); (']',0) }
            Technique => { assert_eq!((a,b),(20,name)); ('T',40) }
            Width => { assert_eq!((a,b),(texture,0)); ('W',width as usize) }
            Height => { assert_eq!((a,b),(texture,0)); ('H',height as usize) }
            Target => { assert_eq!((a,b),(texture,0)); ('R',0) }
            Size => { assert_eq!((a,b),(width as usize,height as usize)); ('Z',0) }
            Begin => { assert_eq!((a,b),(40,0)); ('A',passes) }
            BeginPass => { assert_eq!((a,b),(40,pass_index)); pass_index+=1; ('b',0) }
            Triangle => { assert_eq!((a,b),(0,0)); ('D',0) }
            EndPass => { assert_eq!((a,b),(40,0)); ('e',0) }
            End => { assert_eq!((a,b),(40,0)); ('F',0) }
        };
        trace.push(code);
        result
    };
    if convert { render::render_convert_plane(20,texture,name,&mut call); assert_eq!(pass_index,passes); }
    else { render::draw_mix_texture(index,&mut call); assert_eq!(loop_index,passes+1); }
    trace
}

#[test]
fn test_video_render_passes_c_mix_passes() {
    for index in 0..2 {
        for count in 0..3 {
            let expected=["MEPI[B+L-]","MEPI[B+LSL-]","MEPI[B+LSLSL-]"];
            assert_eq!(run(false,index,count,0,0,0),expected[count]);
        }
    }
}

#[test]
fn test_video_render_passes_c_conversion_passes() {
    for (plane,(width,height)) in [(0,0),(1920,1080),(u32::MAX,1)].into_iter().enumerate() {
        for count in 0..3 {
            let expected=["TWHRZAF","TWHRZAbDeF","TWHRZAbDebDeF"];
            assert_eq!(run(true,plane%2,count,width,height,100+plane),expected[count]);
        }
    }
}
