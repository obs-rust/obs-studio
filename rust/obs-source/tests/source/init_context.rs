//! Tier 1 mirrors test_source_init_context.c::characterize_results.
use obs_source::init_context::initialize;
use std::cell::Cell;

#[test]
fn characterize_results() {
    let expected = [
        c"void destroy(ptr source)",
        c"void remove(ptr source)",
        c"void update(ptr source)",
        c"void save(ptr source)",
        c"void load(ptr source)",
        c"void activate(ptr source)",
        c"void deactivate(ptr source)",
        c"void show(ptr source)",
        c"void hide(ptr source)",
        c"void mute(ptr source, bool muted)",
        c"void push_to_mute_changed(ptr source, bool enabled)",
        c"void push_to_mute_delay(ptr source, int delay)",
        c"void push_to_talk_changed(ptr source, bool enabled)",
        c"void push_to_talk_delay(ptr source, int delay)",
        c"void enable(ptr source, bool enabled)",
        c"void rename(ptr source, string new_name, string prev_name)",
        c"void volume(ptr source, in out float volume)",
        c"void update_properties(ptr source)",
        c"void update_flags(ptr source, int flags)",
        c"void audio_sync(ptr source, int out int offset)",
        c"void audio_balance(ptr source, in out float balance)",
        c"void audio_mixers(ptr source, in out int mixers)",
        c"void audio_monitoring(ptr source, int type)",
        c"void monitor(ptr source, bool monitor)",
        c"void audio_activate(ptr source)",
        c"void audio_deactivate(ptr source)",
        c"void filter_add(ptr source, ptr filter)",
        c"void filter_remove(ptr source, ptr filter)",
        c"void reorder_filters(ptr source)",
        c"void transition_start(ptr source)",
        c"void transition_video_stop(ptr source)",
        c"void transition_stop(ptr source)",
        c"void media_play(ptr source)",
        c"void media_pause(ptr source)",
        c"void media_restart(ptr source)",
        c"void media_stopped(ptr source)",
        c"void media_next(ptr source)",
        c"void media_previous(ptr source)",
        c"void media_started(ptr source)",
        c"void media_ended(ptr source)",
    ];
    for populated in [false, true] {
        for private in [false, true] {
            for init_result in [false, true] {
                for registration_result in [false, true] {
                    let calls = Cell::new(0);
                    let signal_token = Cell::new(false);
                    let inputs = (
                        populated.then_some(1),
                        populated.then_some("source"),
                        populated.then_some("uuid"),
                        populated.then_some(2),
                        private,
                    );
                    let result = initialize(
                        inputs,
                        |received| {
                            assert_eq!(calls.replace(1), 0);
                            assert_eq!(
                                received,
                                (
                                    populated.then_some(1),
                                    populated.then_some("source"),
                                    populated.then_some("uuid"),
                                    populated.then_some(2),
                                    private
                                )
                            );
                            signal_token.set(true);
                            init_result
                        },
                        |signals| {
                            assert_eq!(calls.replace(2), 1);
                            assert!(signal_token.get());
                            assert_eq!(signals, expected);
                            registration_result
                        },
                    );
                    assert_eq!(result, init_result && registration_result);
                    assert_eq!(calls.get(), if init_result { 2 } else { 1 });
                    assert!(signal_token.get());
                }
            }
        }
    }
}
