/* Hybrid MP4 with automatic file splitting by time never splits
 * (obsproject/obs-studio#14051). mp4-output.c's should_split() compared the
 * distance to the split time with llabs(): the file split only when a
 * keyframe of the primary video track landed within 1 ms of the split time,
 * on either side. With keyframes every 2 s at 30 fps, or with the encoder's
 * own keyframe interval, that almost never happens, and once the first
 * keyframe after the split time has gone by the runtime only grows, so the
 * recording stays in one file. The FFmpeg muxer's split (MKV, MP4) splits on
 * the first keyframe at or after the split time.
 *
 * The test builds mp4-output.c into the binary and asks should_split() about
 * keyframes before, at and after the split time. No libobs start-up. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <string.h>

#include "mp4-output.c.h"

/* mp4-mux.c and mp4-output.c name things through the module's locale. */
const char *obs_module_text(const char *val)
{
	return val;
}

#define SEC 1000000LL

static struct mp4_output split_every(int64_t seconds)
{
	struct mp4_output out;
	memset(&out, 0, sizeof(out));
	out.split_file_enabled = true;
	out.start_time = 5 * SEC; /* first packet's dts_usec */
	out.max_time = seconds * SEC;
	return out;
}

static bool split_at(struct mp4_output *out, int64_t runtime_usec, bool keyframe)
{
	struct encoder_packet pkt;
	memset(&pkt, 0, sizeof(pkt));
	pkt.type = OBS_ENCODER_VIDEO;
	pkt.track_idx = 0;
	pkt.keyframe = keyframe;
	pkt.size = 1000;
	pkt.dts_usec = out->start_time + runtime_usec;
	return should_split(out, &pkt);
}

/* A keyframe that comes after the split time starts the next file. */
static void keyframe_after_split_time_splits(void **state)
{
	(void)state;
	struct mp4_output out = split_every(60);

	assert_false(split_at(&out, 56 * SEC, true));
	assert_true(split_at(&out, 63 * SEC, true));
	/* 30 fps, 2 s keyframe interval, split time not on a frame boundary */
	out.max_time = 61 * SEC + 500000;
	assert_true(split_at(&out, 62 * SEC, true));
}

/* Each later keyframe still splits until the muxer starts a new file (which
 * resets start_time), so a missed keyframe cannot keep the file open. */
static void later_keyframes_still_split(void **state)
{
	(void)state;
	struct mp4_output out = split_every(60);

	assert_true(split_at(&out, 70 * SEC, true));
	assert_true(split_at(&out, 600 * SEC, true));
}

/* Unchanged: exactly at the split time, up to 1 ms early (timestamp error),
 * never before, never on a non-keyframe or another track. */
static void split_time_window(void **state)
{
	(void)state;
	struct mp4_output out = split_every(60);

	assert_true(split_at(&out, 60 * SEC, true));
	assert_true(split_at(&out, 60 * SEC - 999, true));
	assert_false(split_at(&out, 60 * SEC - 1000, true));
	assert_false(split_at(&out, 30 * SEC, true));
	assert_false(split_at(&out, 63 * SEC, false));

	struct encoder_packet audio;
	memset(&audio, 0, sizeof(audio));
	audio.type = OBS_ENCODER_AUDIO;
	audio.keyframe = true;
	audio.dts_usec = out.start_time + 63 * SEC;
	assert_false(should_split(&out, &audio));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(keyframe_after_split_time_splits),
		cmocka_unit_test(later_keyframes_still_split),
		cmocka_unit_test(split_time_window),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
