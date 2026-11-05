/* A graphics stall can overflow the async queue without seeking the source.
 * Resetting its pacing clock anchors delayed audio to the leftover backlog
 * instead of the ongoing source timeline (#3368). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs-internal.h>

#define MS 1000000ULL
#define SOURCE_ID "test_async_overflow_clock"

static const char *source_name(void *unused)
{
    (void)unused;
    return SOURCE_ID;
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
    (void)settings;
    return source;
}

static void source_destroy(void *data)
{
    (void)data;
}

static int setup(void **state)
{
    if (!obs_startup("en-US", NULL, NULL))
        return -1;
    struct obs_source_info info = {
        .id = SOURCE_ID,
        .type = OBS_SOURCE_TYPE_INPUT,
        .output_flags = OBS_SOURCE_ASYNC_VIDEO,
        .get_name = source_name,
        .create = source_create,
        .destroy = source_destroy,
    };
    obs_register_source(&info);
    *state = obs_source_create_private(SOURCE_ID, "overflow clock", NULL);
    return *state ? 0 : -1;
}

static int teardown(void **state)
{
    obs_source_release(*state);
    obs_wait_for_destroy_queue();
    obs_shutdown();
    return 0;
}

static void feed(obs_source_t *source, uint64_t timestamp)
{
    uint8_t pixels[4] = {0, 0, 0, 255};
    struct obs_source_frame frame = {
        .data = {pixels},
        .linesize = {sizeof(pixels)},
        .width = 1,
        .height = 1,
        .format = VIDEO_FORMAT_BGRA,
        .timestamp = timestamp,
    };
    obs_source_output_video(source, &frame);
}

static void overflow_keeps_the_running_source_clock(void **state)
{
    obs_source_t *source = *state;
    source->last_frame_ts = 3000 * MS;
    source->last_sys_timestamp = 10000 * MS;
    for (uint64_t i = 1; i <= 30; i++)
        feed(source, (3000 + i * 33) * MS);
    assert_int_equal(source->async_frames.num, 30);
    assert_int_equal(source->last_frame_ts, 3000 * MS);

    /* The 31st frame hits the bounded cache's overflow path. The queue is
     * emptied, but no source timestamp jumped and elapsed canvas time must
     * still advance the existing clock when the next frame arrives. */
    feed(source, 4023 * MS);
    assert_int_equal(source->async_frames.num, 0);
    assert_int_equal(source->async_cache.num, 0);
    assert_int_equal(source->last_frame_ts, 3000 * MS);
    assert_int_equal(source->last_sys_timestamp, 10000 * MS);
    feed(source, 4056 * MS);
    assert_int_equal(source->async_frames.num, 1);
    assert_int_equal(source->last_frame_ts, 3000 * MS);
}

static void explicit_stop_resets_the_source_clock(void **state)
{
    obs_source_t *source = *state;
    source->last_frame_ts = 3000 * MS;
    feed(source, 3033 * MS);
    obs_source_output_video(source, NULL);
    assert_int_equal(source->async_frames.num, 0);
    assert_int_equal(source->last_frame_ts, 0);
}

int main(void)
{
    const struct CMUnitTest tests[] = {
        cmocka_unit_test_setup_teardown(overflow_keeps_the_running_source_clock, setup, teardown),
        cmocka_unit_test_setup_teardown(explicit_stop_resets_the_source_clock, setup, teardown),
    };
    return cmocka_run_group_tests(tests, NULL, NULL);
}
