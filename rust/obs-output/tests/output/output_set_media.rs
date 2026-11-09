//! Mirrors test_output_set_media.c: supported_media and null_output.
use obs_output::output_set_media::{Diagnostic, Media, set_media};

#[test]
fn supported_media() {
    for flags in 0..4 {
        for encoded in [false, true] {
            let (mut video, mut audio) = (1, 2);
            for clear in [false, true] {
                let (new_video, new_audio) = if clear { (0, 0) } else { (3, 4) };
                let before = (video, audio);
                let mut logs = Vec::new();
                set_media(Some(Media { flags: flags | if encoded { 8 } else { 0 }, video: &mut video, audio: &mut audio }), new_video, new_audio, |message| logs.push(message));
                assert_eq!(video, if !encoded && flags & 1 != 0 { new_video } else { before.0 });
                assert_eq!(audio, if !encoded && flags & 2 != 0 { new_audio } else { before.1 });
                assert_eq!(logs, if encoded { vec![Diagnostic::EncodedOutput] } else { vec![] });
            }
        }
    }
}

#[test]
fn null_output() {
    let mut logs = Vec::new();
    set_media::<usize>(None, 0, 0, |message| logs.push(message));
    assert_eq!(logs, [Diagnostic::NullOutput]);
}
