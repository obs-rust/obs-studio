/* obs_wait_for_destroy_queue() must wait for the video thread whenever it
 * runs, also when audio was never reset. tick_sources() holds a reference
 * on every source while it ticks it; a caller that waits for the destroy
 * queue and then checks a weak reference raced that tick (the master push
 * failure of test_scene_item_transition_serialization_lifetime). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>
#include "util/platform.h"
#include "util/threading.h"
#include "obs.h"

static os_event_t *tick_entered;
static volatile long hold_next_tick;

static const char *fixture_name(void *data)
{
	(void)data;
	return "destroy queue sync fixture";
}
static void *fixture_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}
static void fixture_destroy(void *data)
{
	(void)data;
}
static uint32_t fixture_size(void *data)
{
	(void)data;
	return 0;
}
static void fixture_tick(void *data, float seconds)
{
	(void)data;
	(void)seconds;
	if (os_atomic_set_long(&hold_next_tick, 0)) {
		os_event_signal(tick_entered);
		/* Keep tick_sources()' reference while the test releases its own. */
		os_sleep_ms(500);
	}
}
static int setup(void **state)
{
	(void)state;
	if (os_event_init(&tick_entered, OS_EVENT_TYPE_AUTO) != 0)
		return -1;
	if (!obs_startup("en-US", NULL, NULL))
		return -1;
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
	obs_add_data_path(TEST_LIBOBS_DATA_PATH);
#pragma GCC diagnostic pop
	struct obs_video_info video = {.graphics_module = GRAPHICS_MODULE_PATH,
				       .fps_num = 30,
				       .fps_den = 1,
				       .base_width = 64,
				       .base_height = 64,
				       .output_width = 64,
				       .output_height = 64,
				       .output_format = VIDEO_FORMAT_NV12,
				       .colorspace = VIDEO_CS_709,
				       .range = VIDEO_RANGE_PARTIAL,
				       .scale_type = OBS_SCALE_BICUBIC};
	/* Video only: no obs_reset_audio(), like most core-video fixtures. */
	if (obs_reset_video(&video) != OBS_VIDEO_SUCCESS) {
		obs_shutdown();
		return -1;
	}
	struct obs_source_info info = {.id = "destroy_queue_sync_fixture",
				       .type = OBS_SOURCE_TYPE_INPUT,
				       .output_flags = OBS_SOURCE_VIDEO,
				       .get_name = fixture_name,
				       .create = fixture_create,
				       .destroy = fixture_destroy,
				       .get_width = fixture_size,
				       .get_height = fixture_size,
				       .video_tick = fixture_tick};
	obs_register_source(&info);
	return 0;
}
static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	os_event_destroy(tick_entered);
	return 0;
}
static void wait_covers_a_tick_in_progress(void **state)
{
	(void)state;
	obs_source_t *source = obs_source_create_private("destroy_queue_sync_fixture", "Ticked", NULL);
	assert_non_null(source);
	obs_weak_source_t *weak = obs_source_get_weak_source(source);

	os_atomic_set_long(&hold_next_tick, 1);
	assert_int_equal(os_event_timedwait(tick_entered, 5000), 0);
	obs_source_release(source);
	assert_false(obs_weak_source_expired(weak)); /* the tick still holds it */

	while (obs_wait_for_destroy_queue())
		;
	assert_true(obs_weak_source_expired(weak));
	obs_weak_source_release(weak);
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(wait_covers_a_tick_in_progress)};
	return cmocka_run_group_tests(tests, setup, teardown);
}
