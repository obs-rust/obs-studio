use obs_output::encoder_audio_getters::{sample_rate, frame_size, mixer_index};
use core::ffi::CStr;

#[test]
fn mirrors_c_sample_rate_values_logs_and_lazy_media() {
    let mut logs = Vec::new();
    assert_eq!(sample_rate(None, || panic!("unexpected media"), |l,f| logs.push((l,f))), 0);
    assert_eq!(logs, [(400, c"obs_encoder_get_sample_rate: Null 'encoder' parameter")]);
    for kind in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
        for media in [false, true] {
            for stored in [0, 1, 1024, u32::MAX] {
                let fields = (kind, media, stored);
                let mut calls = 0;
                logs.clear();
                assert_eq!(sample_rate(Some(fields), || { calls += 1; 48000 }, |l,f| logs.push((l,f))),
                           if kind == 0 && media { if stored != 0 { stored } else { 48000 } } else { 0 });
                assert_eq!(calls, u32::from(kind == 0 && media && stored == 0));
                assert_eq!(logs, if kind == 0 { vec![] } else { vec![(200, c"obs_encoder_get_sample_rate: encoder '%s' is not an audio encoder")] });
                assert_eq!(fields, (kind, media, stored));
            }
        }
    }
}
#[test]
fn mirrors_c_full_width_sizes_current_type_and_logs() {
    type Getter = fn(Option<(i32,usize)>, fn(i32,&'static CStr)) -> usize;
    let getters: [Getter; 2] = [|f,l| frame_size(f,l), |f,l| mixer_index(f,l)];
    for getter in getters {
        for kind in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
            for value in [0, 1, 1024, u32::MAX as usize, usize::MAX] {
                assert_eq!(getter(Some((kind,value)), |_,_| {}), if kind == 0 { value } else { 0 });
            }
        }
        assert_eq!(getter(None, |_,_| {}), 0);
    }
    let mut logs = Vec::new();
    assert_eq!(frame_size(None, |l,f| logs.push((l,f))), 0);
    assert_eq!(logs, [(400,c"obs_encoder_get_frame_size: Null 'encoder' parameter")]);
    logs.clear();
    assert_eq!(mixer_index(None, |l,f| logs.push((l,f))), 0);
    assert_eq!(logs, [(400,c"obs_encoder_get_mixer_index: Null 'encoder' parameter")]);
    logs.clear();
    assert_eq!(frame_size(Some((1,usize::MAX)), |l,f| logs.push((l,f))), 0);
    assert_eq!(logs, [(200,c"obs_encoder_get_frame_size: encoder '%s' is not an audio encoder")]);
    logs.clear();
    assert_eq!(mixer_index(Some((1,usize::MAX)), |l,f| logs.push((l,f))), 0);
    assert_eq!(logs, [(200,c"obs_encoder_get_mixer_index: encoder '%s' is not an audio encoder")]);
}
