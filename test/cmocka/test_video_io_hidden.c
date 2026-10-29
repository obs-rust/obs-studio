/* Characterization of the non-EXPORT functions in libobs/media-io/video-io.c.
 *
 * libobs hides them (-fvisibility=hidden), so this binary compiles video-io.c
 * itself. Those definitions take precedence over libobs's. This pins the C
 * file. It does not exercise a Rust swap.
 *
 * video_output_close on a frame-rate-divisor child frees the parent's cache
 * and mutexes (the child is a shallow copy). Tests use
 * video_output_free_frame_rate_divisor for the child and close the root.
 * The inc/dec texture helpers have no NULL check. They are not called with
 * NULL.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <string.h>

#include <obs.h>
#include <media-io/video-frame.h>
#include <media-io/video-io.h>
#include <util/platform.h>
#include <util/threading.h>

#define GATE_MAX 4

static int obs_up;

struct gate {
	os_sem_t *hits_sem;
	os_event_t *release;
	int hits;
	uint64_t ts[GATE_MAX];
	uint8_t byte0[GATE_MAX];
};

static void on_frame(void *param, struct video_data *frame)
{
	struct gate *g = param;
	int i = g->hits++;

	if (i < GATE_MAX) {
		g->ts[i] = frame->timestamp;
		g->byte0[i] = frame->data[0] ? frame->data[0][0] : 0;
	}
	os_sem_post(g->hits_sem);
	os_event_wait(g->release);
}

static struct video_output_info rgba_info(const char *name)
{
	struct video_output_info info;

	memset(&info, 0, sizeof(info));
	info.name = name;
	info.format = VIDEO_FORMAT_RGBA;
	info.fps_num = 30;
	info.fps_den = 1;
	info.width = 4;
	info.height = 4;
	info.cache_size = 2;
	info.colorspace = VIDEO_CS_709;
	info.range = VIDEO_RANGE_FULL;
	return info;
}

static void gate_init(struct gate *g)
{
	memset(g, 0, sizeof(*g));
	assert_int_equal(os_sem_init(&g->hits_sem, 0), 0);
	assert_int_equal(os_event_init(&g->release, OS_EVENT_TYPE_MANUAL), 0);
	os_event_signal(g->release);
}

static void finish(video_t *video, struct gate *g)
{
	os_event_signal(g->release);
	video_output_close(video);
	os_sem_destroy(g->hits_sem);
	os_event_destroy(g->release);
}

static void null_divisor(void **state)
{
	UNUSED_PARAMETER(state);
	assert_null(video_output_create_with_frame_rate_divisor(NULL, 2));
	video_output_free_frame_rate_divisor(NULL);
}

static void texture_encoder_refs_skip_the_reset(void **state)
{
	video_t *video = NULL;
	struct video_output_info info = rgba_info("gpu");
	struct gate g;

	UNUSED_PARAMETER(state);
	assert_true(obs_up);
	gate_init(&g);
	assert_int_equal(video_output_open(&video, &info), VIDEO_OUTPUT_SUCCESS);

	video_output_inc_texture_frames(video);
	video_output_inc_texture_encoders(video);
	/* First encoder ref while raw output is idle clears the counters. */
	assert_int_equal(video_output_get_total_frames(video), 0);
	video_output_inc_texture_frames(video);
	video_output_inc_texture_skipped_frames(video);
	assert_int_equal(video_output_get_total_frames(video), 1);
	assert_int_equal(video_output_get_skipped_frames(video), 1);

	assert_true(video_output_connect(video, NULL, on_frame, &g));
	/* gpu_refs != 0, so connect leaves the counters alone. */
	assert_int_equal(video_output_get_total_frames(video), 1);
	assert_true(video_output_disconnect2(video, on_frame, &g));
	assert_int_equal(video_output_get_total_frames(video), 1);

	video_output_dec_texture_encoders(video);
	assert_int_equal(video_output_get_skipped_frames(video), 1);
	assert_int_equal(video_output_get_total_frames(video), 1);
	finish(video, &g);
}

static void frame_rate_divisor_view(void **state)
{
	video_t *video = NULL;
	struct video_output_info info = rgba_info("child");
	struct gate g;
	struct video_frame frame;
	video_t *child;
	const struct video_output_info *got;
	uint64_t frame_time;

	UNUSED_PARAMETER(state);
	assert_true(obs_up);
	gate_init(&g);
	assert_int_equal(video_output_open(&video, &info), VIDEO_OUTPUT_SUCCESS);
	frame_time = video_output_get_frame_time(video);

	assert_null(video_output_create_with_frame_rate_divisor(video, 0));
	assert_null(video_output_create_with_frame_rate_divisor(video, 1));
	child = video_output_create_with_frame_rate_divisor(video, 2);
	assert_non_null(child);

	got = video_output_get_info(child);
	assert_int_equal(got->fps_num, 30);
	/* fps_den is multiplied on the copy. frame_time is not recomputed,
	 * and get_frame_rate reads the root. Characterized, not endorsed. */
	assert_int_equal(got->fps_den, 2);
	assert_true(video_output_get_frame_rate(child) == 30.0);
	assert_true(video_output_get_frame_time(child) == frame_time);
	assert_int_equal(video_output_get_width(child), 4);
	assert_int_equal(video_output_get_format(child), VIDEO_FORMAT_RGBA);

	assert_true(video_output_connect(child, NULL, on_frame, &g));
	assert_true(video_output_active(child));
	assert_true(video_output_lock_frame(child, &frame, 1, 77));
	frame.data[0][0] = 0x55;
	video_output_unlock_frame(child);
	assert_int_equal(os_sem_wait(g.hits_sem), 0);
	assert_int_equal(g.byte0[0], 0x55);

	video_output_stop(child);
	assert_true(video_output_stopped(video));
	/* free(root) is a no-op: the parent pointer is null. */
	video_output_free_frame_rate_divisor(video);
	video_output_free_frame_rate_divisor(child);
	finish(video, &g);
}

/* A grouped encoder starts on the group's start frame, but video-io decides
 * which frames it delivers with a per-input counter that starts at 0 when the
 * input connects. A frame queued before the group start consumes that first
 * slot, so an encoder with a frame-rate divisor above 1 is one phase ahead and
 * never receives the start frame; the group then never starts. Clearing the
 * counter makes the next frame the input receives the start frame again. */
static void push_frame(video_t *video, struct video_frame *frame, uint64_t ts)
{
	/* The frame cache is empty until the video thread starts, and
	 * lock_frame only fails while every slot is still queued. */
	bool locked = false;
	for (int i = 0; i < 500 && !locked; i++) {
		locked = video_output_lock_frame(video, frame, 1, ts);
		if (!locked)
			os_sleep_ms(10);
	}
	assert_true(locked);
	frame->data[0][0] = 0x5a;
	video_output_unlock_frame(video);
}

static void group_start_aligns_the_divisor_counter(void **state)
{
	video_t *video = NULL;
	struct video_output_info info = rgba_info("align");
	struct gate g;
	struct gate other;
	struct video_frame frame;
	uint64_t frame_time;
	const uint64_t start = 1000;

	UNUSED_PARAMETER(state);
	assert_true(obs_up);
	gate_init(&g);
	gate_init(&other);
	assert_int_equal(video_output_open(&video, &info), VIDEO_OUTPUT_SUCCESS);
	frame_time = video_output_get_frame_time(video);
	assert_true(frame_time > 0);

	assert_true(video_output_connect2(video, NULL, 2, on_frame, &g));

	/* Delivered, and it advances the input's divisor counter to 1. */
	push_frame(video, &frame, start);
	assert_int_equal(os_sem_wait(g.hits_sem), 0);
	assert_int_equal(g.hits, 1);
	assert_int_equal(g.ts[0], start);

	/* The group's start frame is two frames on. Without the alignment the
	 * counter is 1 here, so that frame is skipped. */
	assert_true(video_output_align_frame_rate_divisor(video, start + 2 * frame_time, on_frame, &g));
	assert_false(video_output_align_frame_rate_divisor(video, start + 2 * frame_time, on_frame, &other));

	/* Rendered before the phase: dropped, and the counter is not moved. */
	push_frame(video, &frame, start + frame_time);
	/* The phase frame itself: delivered whatever the counter was. */
	push_frame(video, &frame, start + 2 * frame_time);

	assert_int_equal(os_sem_wait(g.hits_sem), 0);
	assert_int_equal(g.hits, 2);
	/* The first frame at or after the phase start, not the one before it.
	 * The cadence after it depends on when the frame the video thread is
	 * already holding is consumed, which the real-libobs case in
	 * test_encoder_group_reconfig_walk covers end to end. */
	assert_int_equal(g.ts[1], start + 2 * frame_time);

	assert_true(video_output_disconnect2(video, on_frame, &g));
	finish(video, &g);
	os_sem_destroy(other.hits_sem);
	os_event_destroy(other.release);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	obs_up = obs_startup("en-US", NULL, NULL);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_divisor),
		cmocka_unit_test(texture_encoder_refs_skip_the_reset),
		cmocka_unit_test(frame_rate_divisor_view),
		cmocka_unit_test(group_start_aligns_the_divisor_counter),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
