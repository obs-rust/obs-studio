/* Characterize the real registered callbacks before their Rust port.
 * Direct pair callbacks pin return values; routed calls also pin pair latches. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs-internal.h>

static const char *source_name(void *unused)
{
	(void)unused;
	return "mute monitor characterization";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void source_destroy(void *unused)
{
	(void)unused;
}

struct fixture {
	obs_source_t *source;
	obs_hotkey_t *keys[4];
	unsigned mute_signals;
	unsigned monitor_signals;
};

static bool find_keys(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	(void)id;
	struct fixture *f = data;
	const char *names[] = {"libobs.mute", "libobs.unmute", "libobs.monitor-on", "libobs.monitor-off"};
	for (size_t i = 0; i < 4; i++)
		if (strcmp(obs_hotkey_get_name(key), names[i]) == 0)
			f->keys[i] = key;
	return true;
}

static void count_signal(void *data, calldata_t *cd)
{
	(void)cd;
	(*(unsigned *)data)++;
}

static int setup(void **state)
{
	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_audio_info audio = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	assert_true(obs_reset_audio(&audio));
	struct obs_source_info info = {
		.id = "test_source_mute_monitor_hotkeys",
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
	};
	obs_register_source(&info);
	struct fixture *f = bzalloc(sizeof(*f));
	f->source = obs_source_create(info.id, "mute monitor callbacks", NULL, NULL);
	assert_non_null(f->source);
	/* Remove from the mix so no audio thread consumes the action queue. */
	obs_source_remove(f->source);
	obs_hotkey_enable_callback_rerouting(true);
	obs_enum_hotkeys(find_keys, f);
	for (size_t i = 0; i < 4; i++)
		assert_non_null(f->keys[i]);
	signal_handler_t *signals = obs_source_get_signal_handler(f->source);
	signal_handler_connect(signals, "mute", count_signal, &f->mute_signals);
	signal_handler_connect(signals, "monitor", count_signal, &f->monitor_signals);
	*state = f;
	return 0;
}

static int teardown(void **state)
{
	struct fixture *f = *state;
	obs_source_release(f->source);
	obs_wait_for_destroy_queue();
	obs_shutdown();
	bfree(f);
	return 0;
}

static size_t action_count(obs_source_t *source)
{
	pthread_mutex_lock(&source->audio_actions_mutex);
	size_t count = source->audio_actions.num;
	pthread_mutex_unlock(&source->audio_actions_mutex);
	return count;
}

static void callback_matrix(void **state)
{
	struct fixture *f = *state;
	for (size_t key = 0; key < 4; key++) {
		const bool monitor = key >= 2;
		const bool target = key % 2 == 0;
		obs_hotkey_pair_t *pair = f->keys[key]->data;
		assert_int_equal(pair->pair_id, monitor ? f->source->monitor_on_off_key : f->source->mute_unmute_key);
		for (unsigned initial = 0; initial < 2; initial++) {
			for (unsigned other = 0; other < 2; other++) {
				for (unsigned pressed = 0; pressed < 2; pressed++) {
					obs_source_set_muted(f->source, monitor ? other : initial);
					obs_source_set_monitoring_enabled(f->source, monitor ? initial : other);
					f->mute_signals = f->monitor_signals = 0;
					size_t before = action_count(f->source);
					bool changed = pressed && initial != target;
					bool result = pair->func[key % 2](pair->data[key % 2], pair->pair_id,
									  f->keys[key], pressed);
					assert_int_equal(result, changed);
					bool expected = changed ? target : initial;
					assert_int_equal(obs_source_muted(f->source), monitor ? other : expected);
					assert_int_equal(obs_source_get_monitoring_enabled(f->source),
							 monitor ? expected : other);
					assert_int_equal(f->mute_signals, !monitor && changed);
					assert_int_equal(f->monitor_signals, monitor && changed);
					assert_int_equal(action_count(f->source), before + (!monitor && changed));
					if (!monitor && changed) {
						pthread_mutex_lock(&f->source->audio_actions_mutex);
						struct audio_action action = f->source->audio_actions.array[before];
						pthread_mutex_unlock(&f->source->audio_actions_mutex);
						assert_int_equal(action.type, AUDIO_ACTION_MUTE);
						assert_int_equal(action.set, target);
					}
				}
			}
		}
	}
}

static void routed_pairs(void **state)
{
	struct fixture *f = *state;
	for (size_t base = 0; base < 4; base += 2) {
		obs_hotkey_pair_t *pair = f->keys[base]->data;
		/* Release before press and redundant off do not latch. */
		if (base == 0)
			obs_source_set_muted(f->source, false);
		else
			obs_source_set_monitoring_enabled(f->source, false);
		obs_hotkey_trigger_routed_callback(f->keys[base]->id, false);
		obs_hotkey_trigger_routed_callback(f->keys[base + 1]->id, true);
		assert_false(pair->pressed0);
		assert_false(pair->pressed1);
		obs_hotkey_trigger_routed_callback(f->keys[base]->id, true);
		assert_true(pair->pressed0);
		/* Repeated press retains the latch; opposite half waits for release. */
		obs_hotkey_trigger_routed_callback(f->keys[base]->id, true);
		obs_hotkey_trigger_routed_callback(f->keys[base + 1]->id, true);
		assert_true(pair->pressed0);
		assert_false(pair->pressed1);
		obs_hotkey_trigger_routed_callback(f->keys[base]->id, false);
		assert_false(pair->pressed0);
		assert_true(base == 0 ? obs_source_muted(f->source) : obs_source_get_monitoring_enabled(f->source));
		obs_hotkey_trigger_routed_callback(f->keys[base + 1]->id, true);
		assert_true(pair->pressed1);
		obs_hotkey_trigger_routed_callback(f->keys[base + 1]->id, false);
		assert_false(pair->pressed1);
		assert_false(base == 0 ? obs_source_muted(f->source) : obs_source_get_monitoring_enabled(f->source));
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(callback_matrix, setup, teardown),
		cmocka_unit_test_setup_teardown(routed_pairs, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
