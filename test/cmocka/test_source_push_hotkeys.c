/* Characterize the actual registered source callbacks, before their Rust port.
 * Duplicate presses/releases each append an action: characterized, not endorsed.
 * The source never enters the mix, so no audio thread consumes its queue. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs-internal.h>
#include <util/platform.h>

static const char *push_name(void *unused)
{
	(void)unused;
	return "push hotkey characterization";
}

static void *push_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void push_destroy(void *unused)
{
	(void)unused;
}

static int setup(void **state)
{
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_audio_info audio = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	assert_true(obs_reset_audio(&audio));
	struct obs_source_info info = {
		.id = "test_source_push_hotkeys",
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = push_name,
		.create = push_create,
		.destroy = push_destroy,
	};
	obs_register_source(&info);
	obs_hotkey_enable_callback_rerouting(true);
	obs_source_t *source = obs_source_create(info.id, "push callbacks", NULL, NULL);
	assert_non_null(source);
	obs_source_remove(source);
	*state = source;
	return 0;
}

static int teardown(void **state)
{
	obs_source_release(*state);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	return 0;
}

static void routed_actions(void **state)
{
	obs_source_t *source = *state;
	/* Release before press, repeated presses, interleaving and repeated releases.
	 * The gates stay disabled: callbacks still enqueue every event. */
	const bool talk[] = {false, true, false, false, true, false, true, true};
	const bool pressed[] = {false, true, true, true, true, false, false, false};
	bool mute_flag = false;
	bool talk_flag = false;
	assert_false(source->user_push_to_mute_pressed);
	assert_false(source->user_push_to_talk_pressed);
	assert_false(source->push_to_mute_enabled);
	assert_false(source->push_to_talk_enabled);
	pthread_mutex_lock(&source->audio_actions_mutex);
	size_t initial_count = source->audio_actions.num;
	pthread_mutex_unlock(&source->audio_actions_mutex);
	assert_int_equal(initial_count, 0);

	uint64_t timestamps[8];
	for (size_t i = 0; i < 8; i++) {
		uint64_t before = os_gettime_ns();
		obs_hotkey_trigger_routed_callback(talk[i] ? source->push_to_talk_key : source->push_to_mute_key,
						   pressed[i]);
		uint64_t after = os_gettime_ns();
		if (talk[i])
			talk_flag = pressed[i];
		else
			mute_flag = pressed[i];
		assert_int_equal(source->user_push_to_mute_pressed, mute_flag);
		assert_int_equal(source->user_push_to_talk_pressed, talk_flag);
		struct audio_action actions[8];
		pthread_mutex_lock(&source->audio_actions_mutex);
		size_t count = source->audio_actions.num;
		for (size_t j = 0; j < count && j < 8; j++)
			actions[j] = source->audio_actions.array[j];
		pthread_mutex_unlock(&source->audio_actions_mutex);
		assert_int_equal(count, i + 1);
		for (size_t j = 0; j <= i; j++) {
			const struct audio_action *action = &actions[j];
			assert_int_equal(action->type, talk[j] ? AUDIO_ACTION_PTT : AUDIO_ACTION_PTM);
			assert_int_equal(action->set, pressed[j]);
			if (j == i) {
				assert_true(action->timestamp >= before);
				assert_true(action->timestamp <= after);
				timestamps[j] = action->timestamp;
			} else {
				assert_true(action->timestamp == timestamps[j]);
			}
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(routed_actions, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
