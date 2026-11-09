/* Source-only characterization of the hidden C function. Dependency mocks
 * check control flow and forwarding; this is not a libobs ABI/swap test. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs-internal.h"

static struct obs_source source;
static obs_data_t *expected_settings;
static obs_data_t *expected_hotkeys;
static const char *expected_name;
static const char *expected_uuid;
static bool expected_private;
static bool init_result;
static bool registration_result;
static unsigned calls;
static int signal_token;
static const char *expected_signals[] = {
	"void destroy(ptr source)",
	"void remove(ptr source)",
	"void update(ptr source)",
	"void save(ptr source)",
	"void load(ptr source)",
	"void activate(ptr source)",
	"void deactivate(ptr source)",
	"void show(ptr source)",
	"void hide(ptr source)",
	"void mute(ptr source, bool muted)",
	"void push_to_mute_changed(ptr source, bool enabled)",
	"void push_to_mute_delay(ptr source, int delay)",
	"void push_to_talk_changed(ptr source, bool enabled)",
	"void push_to_talk_delay(ptr source, int delay)",
	"void enable(ptr source, bool enabled)",
	"void rename(ptr source, string new_name, string prev_name)",
	"void volume(ptr source, in out float volume)",
	"void update_properties(ptr source)",
	"void update_flags(ptr source, int flags)",
	"void audio_sync(ptr source, int out int offset)",
	"void audio_balance(ptr source, in out float balance)",
	"void audio_mixers(ptr source, in out int mixers)",
	"void audio_monitoring(ptr source, int type)",
	"void monitor(ptr source, bool monitor)",
	"void audio_activate(ptr source)",
	"void audio_deactivate(ptr source)",
	"void filter_add(ptr source, ptr filter)",
	"void filter_remove(ptr source, ptr filter)",
	"void reorder_filters(ptr source)",
	"void transition_start(ptr source)",
	"void transition_video_stop(ptr source)",
	"void transition_stop(ptr source)",
	"void media_play(ptr source)",
	"void media_pause(ptr source)",
	"void media_restart(ptr source)",
	"void media_stopped(ptr source)",
	"void media_next(ptr source)",
	"void media_previous(ptr source)",
	"void media_started(ptr source)",
	"void media_ended(ptr source)",
	NULL,
};

static bool characterize_context_init(struct obs_context_data *context, enum obs_obj_type type, obs_data_t *settings,
				      const char *name, const char *uuid, obs_data_t *hotkeys, bool private)
{
	assert_int_equal(calls++, 0);
	assert_ptr_equal(context, &source.context);
	assert_int_equal(type, OBS_OBJ_TYPE_SOURCE);
	assert_ptr_equal(settings, expected_settings);
	assert_ptr_equal(name, expected_name);
	assert_ptr_equal(uuid, expected_uuid);
	assert_ptr_equal(hotkeys, expected_hotkeys);
	assert_int_equal(private, expected_private);
	context->signals = (signal_handler_t *)&signal_token;
	return init_result;
}

static bool characterize_add_array(signal_handler_t *handler, const char **signals)
{
	assert_int_equal(calls++, 1);
	assert_ptr_equal(handler, &signal_token);
	for (size_t i = 0; i < sizeof(expected_signals) / sizeof(*expected_signals); ++i) {
		if (expected_signals[i]) {
			assert_non_null(signals[i]);
			assert_string_equal(signals[i], expected_signals[i]);
		} else {
			assert_null(signals[i]);
		}
	}
	return registration_result;
}

#define obs_context_data_init characterize_context_init
#define signal_handler_add_array characterize_add_array
#include "source-init-context-characterization.inc"
#undef obs_context_data_init
#undef signal_handler_add_array

static void characterize_results(void **state)
{
	(void)state;
	int settings_token, hotkeys_token;
	for (unsigned inputs = 0; inputs < 4; ++inputs) {
		expected_settings = inputs & 1 ? (obs_data_t *)&settings_token : NULL;
		expected_hotkeys = inputs & 1 ? (obs_data_t *)&hotkeys_token : NULL;
		expected_name = inputs & 1 ? "source" : NULL;
		expected_uuid = inputs & 1 ? "uuid" : NULL;
		expected_private = (inputs & 2) != 0;
		for (unsigned results = 0; results < 4; ++results) {
			init_result = (results & 1) != 0;
			registration_result = (results & 2) != 0;
			calls = 0;
			source.context.signals = NULL;
			assert_int_equal(obs_source_init_context(&source, expected_settings, expected_name,
								 expected_uuid, expected_hotkeys, expected_private),
					 init_result && registration_result);
			assert_int_equal(calls, init_result ? 2 : 1);
			/* Characterized: a failed init is not cleaned up by this wrapper. */
			assert_ptr_equal(source.context.signals, &signal_token);
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(characterize_results)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
