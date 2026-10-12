#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include "media-time.h"

/* Upstream obsproject/obs-studio#11183 (reported by nlsmart): with a 24 hour
 * soundtrack in a Media Source, the elapsed time in the media controls stood
 * still while the file played. The label was computed back from the slider
 * position, which has 1024 steps, so it only moved every 84 seconds. */

#define SLIDER_MAX 1024 /* frontend/forms/source-toolbar/media-controls.ui */
#define SECOND 1000LL
#define HOUR (3600LL * SECOND)
#define DAY (24LL * HOUR)

static void expect_str(const char *what, const char *got, const char *expected)
{
	if (strcmp(got, expected) != 0) {
		fprintf(stderr, "%s\n  got:      \"%s\"\n  expected: \"%s\"\n", what, got, expected);
		fail();
	}
}

static void expect_labels(int64_t time_ms, int64_t duration_ms, bool seeking, int slider_value, bool countdown,
			  const char *elapsed, const char *duration)
{
	struct media_time_labels labels;
	char what[160];

	memset(&labels, 0, sizeof(labels));
	media_time_get_labels(&labels, time_ms, duration_ms, seeking, slider_value, SLIDER_MAX, countdown);

	snprintf(what, sizeof(what), "elapsed label: time %lld ms, duration %lld ms, seeking %d, slider %d",
		 (long long)time_ms, (long long)duration_ms, (int)seeking, slider_value);
	expect_str(what, labels.elapsed, elapsed);
	snprintf(what, sizeof(what), "duration label: time %lld ms, duration %lld ms, seeking %d, countdown %d",
		 (long long)time_ms, (long long)duration_ms, (int)seeking, (int)countdown);
	expect_str(what, labels.duration, duration);
}

static void expect_format(int64_t time_ms, const char *expected)
{
	char buf[MEDIA_TIME_LABEL_SIZE];
	char what[64];

	media_time_format(buf, sizeof(buf), time_ms);
	snprintf(what, sizeof(what), "media_time_format(%lld)", (long long)time_ms);
	expect_str(what, buf, expected);
}

/* The reporter's case: a 24 hour file playing. Every second of playback
 * must show up in the elapsed label. */
static void test_24h_label_changes_every_second(void **state)
{
	char prev[MEDIA_TIME_LABEL_SIZE] = "";
	int64_t start = 10 * HOUR;
	int steps = 0;
	int unchanged = 0;
	int wrong = 0;

	(void)state;

	/* Two minutes of playback, sampled mid-second like the media timer. */
	for (int64_t t = start; t < start + 120 * SECOND; t += SECOND) {
		struct media_time_labels labels;
		char expected[MEDIA_TIME_LABEL_SIZE];

		media_time_get_labels(&labels, t + 400, DAY, false, 0, SLIDER_MAX, false);
		media_time_format(expected, sizeof(expected), t);

		if (strcmp(labels.elapsed, expected) != 0) {
			if (!wrong)
				fprintf(stderr, "24 h file at %lld ms: elapsed \"%s\", expected \"%s\"\n",
					(long long)(t + 400), labels.elapsed, expected);
			wrong++;
		}
		if (steps && strcmp(labels.elapsed, prev) == 0)
			unchanged++;
		snprintf(prev, sizeof(prev), "%s", labels.elapsed);
		steps++;
	}

	if (wrong || unchanged) {
		fprintf(stderr, "24 h file, %d one-second steps: %d wrong labels, label unchanged after %d steps\n",
			steps, wrong, unchanged);
		fail();
	}

	expect_labels(10 * 60 * SECOND, DAY, false, 0, false, "00:10:00", "24:00:00");
}

/* An hour-long file only moved every 3.5 s. */
static void test_1h_label_is_exact(void **state)
{
	(void)state;
	expect_labels(20 * 60 * SECOND + 34 * SECOND + 567, HOUR, false, 0, false, "00:20:34", "01:00:00");
	expect_labels(59 * 60 * SECOND + 59 * SECOND, HOUR, false, 0, false, "00:59:59", "01:00:00");
	expect_labels(SECOND, HOUR, false, 0, false, "00:00:01", "01:00:00");
}

/* The countdown (click on the duration label) counts down each second. */
static void test_countdown_follows_media_time(void **state)
{
	(void)state;
	expect_labels(10 * HOUR, DAY, false, 0, true, "10:00:00", "-14:00:00");
	expect_labels(10 * HOUR + SECOND, DAY, false, 0, true, "10:00:01", "-13:59:59");
	expect_labels(10 * HOUR + 2 * SECOND, DAY, false, 0, true, "10:00:02", "-13:59:58");
}

/* While the user drags the slider, the label previews where the drag
 * would seek to, not where the media is. */
static void test_seeking_previews_slider(void **state)
{
	(void)state;
	expect_labels(5 * SECOND, DAY, true, 512, false, "12:00:00", "24:00:00");
	expect_labels(5 * SECOND, DAY, true, 1, false, "00:01:24", "24:00:00");
	expect_labels(5 * SECOND, DAY, true, SLIDER_MAX, true, "24:00:00", "-00:00:00");
	expect_labels(5 * SECOND, DAY, true, 0, false, "00:00:00", "24:00:00");
}

/* The slider maps to the exact media time, for the seek and the label. */
static void test_slider_maps_exactly(void **state)
{
	(void)state;
	/* 333 / 1024 of 24 h is 28096875 ms exactly. */
	assert_int_equal(media_time_from_slider(333, SLIDER_MAX, DAY), 28096875);
	assert_int_equal(media_time_from_slider(1, SLIDER_MAX, DAY), 84375);
	assert_int_equal(media_time_from_slider(SLIDER_MAX, SLIDER_MAX, DAY), DAY);
	assert_int_equal(media_time_from_slider(0, SLIDER_MAX, DAY), 0);

	assert_int_equal(media_time_to_slider(DAY / 2, DAY, SLIDER_MAX), 512);
	assert_int_equal(media_time_to_slider(28096875, DAY, SLIDER_MAX), 333);
	assert_int_equal(media_time_to_slider(0, DAY, SLIDER_MAX), 0);
	assert_int_equal(media_time_to_slider(DAY, DAY, SLIDER_MAX), SLIDER_MAX);
}

/* A source with no duration yet (or a live stream) shows zeros. */
static void test_zero_duration(void **state)
{
	(void)state;
	assert_int_equal(media_time_to_slider(5 * SECOND, 0, SLIDER_MAX), 0);
	assert_int_equal(media_time_from_slider(512, SLIDER_MAX, 0), 0);
	expect_labels(0, 0, false, 0, false, "00:00:00", "00:00:00");
	expect_labels(0, 0, false, 0, true, "00:00:00", "-00:00:00");
	expect_labels(0, 0, true, 512, false, "00:00:00", "00:00:00");
}

/* The media time may run past the duration at the end of a file. */
static void test_time_past_end_is_clamped(void **state)
{
	(void)state;
	assert_int_equal(media_time_to_slider(HOUR + 500, HOUR, SLIDER_MAX), SLIDER_MAX);
	expect_labels(HOUR + 2 * SECOND, HOUR, false, 0, false, "01:00:00", "01:00:00");
	expect_labels(HOUR + 2 * SECOND, HOUR, false, 0, true, "01:00:00", "-00:00:00");
}

static void test_hours_formatting(void **state)
{
	(void)state;
	expect_format(0, "00:00:00");
	expect_format(999, "00:00:00");
	expect_format(59 * SECOND, "00:00:59");
	expect_format(61 * SECOND, "00:01:01");
	expect_format(HOUR + 61 * SECOND, "01:01:01");
	expect_format(DAY - 1, "23:59:59");
	expect_format(DAY + HOUR, "25:00:00");
	expect_format(100 * HOUR, "100:00:00");
	expect_format(-500, "00:00:00");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_24h_label_changes_every_second),
		cmocka_unit_test(test_1h_label_is_exact),
		cmocka_unit_test(test_countdown_follows_media_time),
		cmocka_unit_test(test_seeking_previews_slider),
		cmocka_unit_test(test_slider_maps_exactly),
		cmocka_unit_test(test_zero_duration),
		cmocka_unit_test(test_time_past_end_is_clamped),
		cmocka_unit_test(test_hours_formatting),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
