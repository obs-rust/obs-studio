//! Tier 1 mirrors test_output_supported_codecs.c without FFI or unsafe code.
use obs_output::output_supported_codecs::{Codecs, audio, video};

#[test]
fn borrowed_codecs() {
    let values: [Option<&[u8]>; 4] = [None, Some(b""), Some(b"h264;\xff"), Some(b"aac;opus")];
    for v in values {
        for a in values {
            let fields = Some(Codecs { video: v, audio: a });
            for _ in 0..2 {
                assert_eq!(video(fields, |_| panic!("valid output must not log")), v);
                assert_eq!(audio(fields, |_| panic!("valid output must not log")), a);
            }
        }
    }
}

#[test]
fn null_output_logs_each_getter_name() {
    let mut logs = Vec::new();
    assert_eq!(video::<Option<&[u8]>>(None, |name| logs.push(name)), None);
    assert_eq!(audio::<Option<&[u8]>>(None, |name| logs.push(name)), None);
    assert_eq!(
        logs,
        [
            c"obs_output_get_supported_video_codecs",
            c"obs_output_get_supported_audio_codecs"
        ]
    );
}
