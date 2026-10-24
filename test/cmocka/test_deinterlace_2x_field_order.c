/*
 * Double-rate deinterlacing must output the fields of an interlaced source in
 * the order they were captured, for both field orders (upstream
 * obsproject/obs-studio#6061: Yadif 2x on bottom-field-first video jumps back
 * in time on every other output frame), and must not skip fields when the
 * field rate differs from the canvas rate (upstream obsproject/obs-studio#3402:
 * 50i into a 60 fps canvas drops about a quarter of the fields).
 *
 * A test async source outputs interlaced BGRA frames (60i or 50i) into a
 * 60, 50 or 30 fps canvas. Every field carries its capture index t (0, 1,
 * 2, ... in time order) in the red and blue channels of its rows, and its
 * parity in green (top rows dark, bottom rows bright). Each output frame is
 * classified by the field it kept: green tells the parity (the interpolated
 * rows of a flat field take the kept field's green), the kept row gives t.
 * The kept fields must never go back in time. When the canvas is at least as
 * fast as the field rate every field must be shown (buffered sources: none
 * for more frames than the rates call for); when it is slower, at most one
 * field may be dropped between two shown ones. Buffered and unbuffered
 * sources are timed by different code, so both are covered.
 *
 * The first frame (obs-rust/obs-studio#1965): the first_frame cases measure
 * a source that never showed a frame, and the after_pause cases a source
 * that pauses for longer than the clock-jump threshold before the measured
 * frames. Both count the first frame's fields too.
 *
 * A buffered source that falls behind (obs-rust/obs-studio#2009): the
 * behind cases. Its buffered walk skips overdue frames; it left the "pair"
 * flag on a frame it had just returned to the frame cache, and the next
 * frame to reuse that entry was shown as the previous frame of a pair, so
 * its fields were skipped.
 *
 * Needs core video, so it runs under xvfb-run and skips without a display.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#define W 64
#define H 64
#define FRAMES 36

/* Field t is drawn as R = R_BASE + STEP * (t / 8), B = R_BASE + STEP * (t % 8). */
#define R_BASE 32
#define STEP 24
#define G_TOP 64
#define G_BOTTOM 192
#define TOLERANCE 6

#define MAX_SEEN 1024
#define STARTUP_FIELDS 4
/* The pause before the measured frames of the first_frame and after_pause
 * cases: past the 70 ms after which the gap from the frame before is a pause,
 * not a frame duration (OBS_SOURCE_DEINTERLACE_PACING_TS_JUMP_THRESHOLD). */
#define PAUSE_MS 150
/* Neutral frames pushed before a case is recorded, and output frames to wait
 * for, so the first measured frame doesn't pay for a cold render path
 * (shader compile, first texture upload; obs-rust/obs-studio#456). */
#define WARMUP_FRAMES 6
#define WARMUP_TICKS 4

static bool obs_up;

struct watch {
	pthread_mutex_t mutex;
	bool recording;
	size_t ticks; /* every output frame, recorded or not */
	size_t count;
	int seen[MAX_SEEN]; /* field index, or -1 for an unrecognised frame */
};

static int snap(int v)
{
	int i = (v - R_BASE + STEP / 2) / STEP;
	if (v < R_BASE - TOLERANCE || i < 0)
		return -1;
	int d = v - (R_BASE + STEP * i);
	return (d >= -TOLERANCE && d <= TOLERANCE) ? i : -1;
}

static int parity_of(int g)
{
	if (g >= G_TOP - TOLERANCE * 2 && g <= G_TOP + TOLERANCE * 2)
		return 0;
	if (g >= G_BOTTOM - TOLERANCE * 2 && g <= G_BOTTOM + TOLERANCE * 2)
		return 1;
	return -1;
}

/* BGRA output: returns the index of the field this frame kept, or -1. */
static int classify(const struct video_data *frame)
{
	const uint8_t *row0 = frame->data[0] + (H / 2) * frame->linesize[0] + (W / 2) * 4;
	const uint8_t *row1 = row0 + frame->linesize[0];
	int p0 = parity_of(row0[1]), p1 = parity_of(row1[1]);
	if (p0 < 0 || p0 != p1)
		return -1;

	const uint8_t *kept = p0 == 0 ? row0 : row1;
	int hi = snap(kept[2]), lo = snap(kept[0]);
	if (hi < 0 || lo < 0 || lo > 7)
		return -1;
	return hi * 8 + lo;
}

static void raw_video(void *param, struct video_data *frame)
{
	struct watch *w = param;
	int field = classify(frame);

	pthread_mutex_lock(&w->mutex);
	w->ticks++;
	if (w->recording && w->count < MAX_SEEN)
		w->seen[w->count++] = field;
	pthread_mutex_unlock(&w->mutex);
}

static const char *interlaced_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "interlaced test source";
}

static void *interlaced_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void interlaced_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static struct obs_source_info interlaced_source = {
	.id = "interlaced_test_source",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_ASYNC_VIDEO,
	.get_name = interlaced_name,
	.create = interlaced_create,
	.destroy = interlaced_destroy,
};

static void fill_rows(uint8_t *pixels, int parity, int t)
{
	for (int y = parity; y < H; y += 2) {
		uint8_t *row = pixels + y * W * 4;
		for (int x = 0; x < W; x++) {
			row[x * 4 + 0] = (uint8_t)(R_BASE + STEP * (t % 8));
			row[x * 4 + 1] = parity ? G_BOTTOM : G_TOP;
			row[x * 4 + 2] = (uint8_t)(R_BASE + STEP * (t / 8));
			row[x * 4 + 3] = 255;
		}
	}
}

/* Pushes FRAMES interlaced frames at fps, timestamped like a capture card. */
static void play(obs_source_t *source, bool top_first, uint32_t fps)
{
	static uint8_t pixels[W * H * 4];
	const uint64_t period = 1000000000ULL / fps;
	const uint64_t start = os_gettime_ns();

	for (int n = 0; n < FRAMES; n++) {
		/* the first field of frame n was captured at 2n, the second at 2n + 1 */
		fill_rows(pixels, top_first ? 0 : 1, 2 * n);
		fill_rows(pixels, top_first ? 1 : 0, 2 * n + 1);

		uint64_t ts = start + n * period;
		struct obs_source_frame frame = {
			.data = {pixels},
			.linesize = {W * 4},
			.width = W,
			.height = H,
			.format = VIDEO_FORMAT_BGRA,
			.full_range = true,
			.timestamp = ts,
		};
		os_sleepto_ns(ts);
		obs_source_output_video(source, &frame);
	}
	os_sleep_ms(300);
}

/* Pushes WARMUP_FRAMES unrecognisable (black) frames at fps, then waits until
 * the canvas has rendered WARMUP_TICKS more frames (at most 2 s). */
static void warm_up(obs_source_t *source, uint32_t fps, struct watch *w)
{
	static uint8_t black[W * H * 4];
	const uint64_t period = 1000000000ULL / fps;
	const uint64_t start = os_gettime_ns();

	pthread_mutex_lock(&w->mutex);
	const size_t ticks = w->ticks;
	pthread_mutex_unlock(&w->mutex);

	for (int n = 0; n < WARMUP_FRAMES; n++) {
		uint64_t ts = start + n * period;
		struct obs_source_frame frame = {
			.data = {black},
			.linesize = {W * 4},
			.width = W,
			.height = H,
			.format = VIDEO_FORMAT_BGRA,
			.full_range = true,
			.timestamp = ts,
		};
		os_sleepto_ns(ts);
		obs_source_output_video(source, &frame);
	}
	for (int waited = 0; waited < 200; waited++) {
		pthread_mutex_lock(&w->mutex);
		const bool warm = w->ticks >= ticks + WARMUP_TICKS;
		pthread_mutex_unlock(&w->mutex);
		if (warm)
			break;
		os_sleep_ms(10);
	}
}

static const char *mode_name(enum obs_deinterlace_mode mode)
{
	switch (mode) {
	case OBS_DEINTERLACE_MODE_YADIF_2X:
		return "yadif 2x";
	case OBS_DEINTERLACE_MODE_LINEAR_2X:
		return "linear 2x";
	case OBS_DEINTERLACE_MODE_RETRO:
		return "discard 2x (retro)";
	default:
		return "?";
	}
}

/* What comes before the measured frames. */
enum start {
	START_WARM,  /* the source's own warm-up frames, then at once */
	START_NEW,   /* nothing: a source that never showed a frame */
	START_PAUSE, /* the source's own warm-up frames, then a pause */
};

struct rates {
	uint32_t source_fps; /* frames per second; the field rate is twice this */
	uint32_t canvas_fps;
};

static bool reset_canvas(uint32_t fps)
{
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = fps,
		.fps_den = 1,
		.base_width = W,
		.base_height = H,
		.output_width = W,
		.output_height = H,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_POINT,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS)
		fprintf(stderr, "obs_reset_video at %u fps failed: %d\n", fps, ret);
	return ret == OBS_VIDEO_SUCCESS;
}

static obs_source_t *create_interlaced(enum obs_deinterlace_mode mode, bool top_first, bool unbuffered)
{
	obs_source_t *source = obs_source_create("interlaced_test_source", "interlaced", NULL, NULL);
	/* Capture cards often run unbuffered (DirectShow's "Auto"). */
	obs_source_set_async_unbuffered(source, unbuffered);
	obs_source_set_deinterlace_mode(source, mode);
	obs_source_set_deinterlace_field_order(source, top_first ? OBS_DEINTERLACE_FIELD_ORDER_TOP
								 : OBS_DEINTERLACE_FIELD_ORDER_BOTTOM);
	return source;
}

static void check_field_order(enum obs_deinterlace_mode mode, bool top_first, bool unbuffered, const struct rates *r,
			      enum start start)
{
	const uint32_t field_rate = 2 * r->source_fps;
	char what[128];
	snprintf(what, sizeof(what), "%s, %s%s%s", mode_name(mode),
		 top_first ? "top field first" : "bottom field first", unbuffered ? ", unbuffered" : "",
		 start == START_NEW     ? ", first frame"
		 : start == START_PAUSE ? ", after a pause"
					: "");

	if (!reset_canvas(r->canvas_fps))
		fail();

	obs_source_t *source = create_interlaced(mode, top_first, unbuffered);
	obs_scene_t *scene = obs_scene_create("scene");

	static struct watch w;
	memset(&w, 0, sizeof(w));
	pthread_mutex_init(&w.mutex, NULL);
	obs_add_raw_video_callback(NULL, raw_video, &w);
	obs_set_output_source(0, obs_scene_get_source(scene));

	if (start == START_NEW) {
		/* Another source, deinterlaced the same way, warms the render
		 * path up and goes. */
		obs_source_t *warm = create_interlaced(mode, top_first, unbuffered);
		obs_sceneitem_t *item = obs_scene_add(scene, warm);
		warm_up(warm, r->source_fps, &w);
		obs_sceneitem_remove(item);
		obs_source_remove(warm);
		obs_source_release(warm);
		obs_scene_add(scene, source);
		os_sleep_ms(PAUSE_MS);
	} else {
		obs_scene_add(scene, source);
		warm_up(source, r->source_fps, &w);
		if (start == START_PAUSE)
			os_sleep_ms(PAUSE_MS);
	}

	pthread_mutex_lock(&w.mutex);
	w.recording = true;
	pthread_mutex_unlock(&w.mutex);

	play(source, top_first, r->source_fps);

	pthread_mutex_lock(&w.mutex);
	w.recording = false;
	pthread_mutex_unlock(&w.mutex);

	/* Let the output drain to black so the next case starts clean. */
	obs_set_output_source(0, NULL);
	os_sleep_ms(250);
	obs_remove_raw_video_callback(raw_video, &w);
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_source_release(source);
	pthread_mutex_destroy(&w.mutex);

	/* Collapse repeats: the fields in the order they were shown. Count the
	 * fields skipped between two shown ones, except in the first two
	 * frames (fields 0-3) after the warm-up: its last frame and the first
	 * measured one are stamped back to back, so the first has no frame
	 * duration to time its fields by (and Yadif needs a previous frame).
	 * After a pause or for a new source, count every field from field 0,
	 * the first one shown included (#1965); only a buffered new source's
	 * first frame may be held longer than the cadence, as the buffered
	 * clock starts from it. Also count how many output frames each field
	 * is held for (the last one is held until the case ends). */
	const int startup = start == START_WARM ? STARTUP_FIELDS : 0;
	const int unheld = start == START_NEW && !unbuffered ? 2 : startup;
	int shown[MAX_SEEN];
	size_t num = 0, unknown = 0, backwards = 0, skipped = 0, max_gap = 0, held = 0, max_held = 0;
	size_t holds[MAX_SEEN];
	size_t num_holds = 0;
	for (size_t i = 0; i < w.count; i++) {
		if (w.seen[i] < 0) {
			unknown++;
			continue;
		}
		if (num && shown[num - 1] == w.seen[i]) {
			held++;
			continue;
		}
		if (num && shown[num - 1] >= unheld) {
			holds[num_holds++] = held;
			if (held > max_held)
				max_held = held;
		}
		held = 1;
		if (num && w.seen[i] < shown[num - 1])
			backwards++;
		const int last = num ? shown[num - 1] : -1;
		if ((num || !startup) && w.seen[i] > last + 1 && w.seen[i] > startup) {
			int from = last < startup - 1 ? startup - 1 : last;
			size_t gap = (size_t)(w.seen[i] - from - 1);
			skipped += gap;
			if (gap > max_gap)
				max_gap = gap;
		}
		shown[num++] = w.seen[i];
	}

	fprintf(stderr,
		"%s, %ui into %u fps: %zu output frames, %zu unrecognised, %zu fields shown, %zu skipped "
		"(at most %zu in a row), held for up to %zu frames, %zu steps back in time:",
		what, field_rate, r->canvas_fps, w.count, unknown, num, skipped, max_gap, max_held, backwards);
	for (size_t i = 0; i < num; i++)
		fprintf(stderr, " %d", shown[i]);
	fprintf(stderr, "\n");

	/* A canvas at least as fast as the field rate can show every field,
	 * each for at most canvas / field rate frames, rounded up (50i into 60
	 * fps: 5 fields once, 1 twice, like 50p); a slower one must drop some,
	 * but never two in a row. An unbuffered source shows a frame as soon
	 * as it arrives, so one that arrives just after a tick repeats the
	 * field before it: only buffered sources are held to the cadence. */
	const size_t allowed_gap = r->canvas_fps >= field_rate ? 0 : 1;
	const size_t allowed_held = unbuffered ? SIZE_MAX : (r->canvas_fps + field_rate - 1) / field_rate;
	const size_t expected = r->canvas_fps >= field_rate ? 2 * FRAMES : 2 * FRAMES * r->canvas_fps / field_rate;

	if (num < expected * 3 / 4) {
		fprintf(stderr, "%s: only %zu of %d fields were shown (expected about %zu)\n", what, num, 2 * FRAMES,
			expected);
		fail();
	}
	if (backwards) {
		fprintf(stderr, "%s: the output went back in time %zu times\n", what, backwards);
		fail();
	}
	if (max_gap > allowed_gap) {
		fprintf(stderr, "%s, %ui into %u fps: %zu fields skipped, up to %zu in a row (allowed %zu)\n", what,
			field_rate, r->canvas_fps, skipped, max_gap, allowed_gap);
		fail();
	}
	/* One field may be held a frame longer than the cadence allows: a single
	 * late tick under load (#456). A second one, or a longer hold, is a
	 * cadence bug. */
	size_t over = 0;
	for (size_t i = 0; i < num_holds; i++)
		if (holds[i] > allowed_held)
			over++;
	if (allowed_held != SIZE_MAX && (max_held > allowed_held + 1 || over > 1)) {
		fprintf(stderr,
			"%s, %ui into %u fps: a field was held for %zu frames (allowed %zu, once %zu); %zu over\n",
			what, field_rate, r->canvas_fps, max_held, allowed_held, allowed_held + 1, over);
		fail();
	}
}

/* A buffered source that falls behind (#2009). Its buffered walk skips the
 * frames that are overdue at a tick, up to the last one due; with three
 * frames queued and all of them overdue, it erased the first, returned it to
 * the frame cache, and then set its prev_frame flag. The next frame to reuse
 * that cache entry was queued with the flag on, so when it was picked with a
 * frame behind it, it was taken as the previous frame of a pair: the frame
 * behind it was shown in its place, and its own fields never were.
 *
 * A source that never showed a frame shows frame 0, then right after a tick
 * gets frames 1, 2 and 3 at once, stamped 4, 8 and 12 ms after frame 0: all
 * three are overdue at the next tick (it comes at least 16 ms after frame 0
 * was picked, and the source's clock stood still with nothing queued). The
 * walk drops frame 1 and shows 2, then 3. Right after that tick, frames 4 to
 * 9 come at once, at the source's rate after frame 3; the first of them
 * reuses frame 1's cache entry. From frame 4's first field on, every field
 * must be shown in order (no two in a row dropped at a slower canvas). Frame
 * 4 must not be due at the tick after frame 3's, so the frame duration is
 * longer than the canvas interval plus 2 ms. */
#define BURST_FRAMES 6

static void wait_ticks(struct watch *w, size_t n)
{
	pthread_mutex_lock(&w->mutex);
	const size_t ticks = w->ticks;
	pthread_mutex_unlock(&w->mutex);
	for (int waited = 0; waited < 2000; waited++) {
		pthread_mutex_lock(&w->mutex);
		const bool done = w->ticks >= ticks + n;
		pthread_mutex_unlock(&w->mutex);
		if (done)
			break;
		os_sleep_ms(1);
	}
}

static void output_frame(obs_source_t *source, int n, uint64_t ts)
{
	static uint8_t pixels[W * H * 4];
	fill_rows(pixels, 0, 2 * n);
	fill_rows(pixels, 1, 2 * n + 1);
	struct obs_source_frame frame = {
		.data = {pixels},
		.linesize = {W * 4},
		.width = W,
		.height = H,
		.format = VIDEO_FORMAT_BGRA,
		.full_range = true,
		.timestamp = ts,
	};
	obs_source_output_video(source, &frame);
}

static void check_burst(enum obs_deinterlace_mode mode, const struct rates *r)
{
	if (!reset_canvas(r->canvas_fps))
		fail();

	obs_scene_t *scene = obs_scene_create("scene");
	static struct watch w;
	memset(&w, 0, sizeof(w));
	pthread_mutex_init(&w.mutex, NULL);
	obs_add_raw_video_callback(NULL, raw_video, &w);
	obs_set_output_source(0, obs_scene_get_source(scene));

	/* Another source, deinterlaced the same way, warms the render path
	 * up and goes, so the measured one starts with an empty cache. */
	obs_source_t *warm = create_interlaced(mode, true, false);
	obs_sceneitem_t *item = obs_scene_add(scene, warm);
	warm_up(warm, r->source_fps, &w);
	obs_sceneitem_remove(item);
	obs_source_remove(warm);
	obs_source_release(warm);

	obs_source_t *source = create_interlaced(mode, true, false);
	obs_scene_add(scene, source);
	wait_ticks(&w, 2);

	pthread_mutex_lock(&w.mutex);
	w.recording = true;
	pthread_mutex_unlock(&w.mutex);

	const uint64_t period = 1000000000ULL / r->source_fps;
	const uint64_t t0 = os_gettime_ns();
	output_frame(source, 0, t0);
	wait_ticks(&w, 3);
	for (int n = 1; n <= 3; n++)
		output_frame(source, n, t0 + n * 4000000ULL);
	wait_ticks(&w, 1);
	for (int n = 4; n < 4 + BURST_FRAMES; n++)
		output_frame(source, n, t0 + 12000000ULL + (uint64_t)(n - 3) * period);
	os_sleep_ms((unsigned)(BURST_FRAMES * period / 1000000 + 300));

	pthread_mutex_lock(&w.mutex);
	w.recording = false;
	pthread_mutex_unlock(&w.mutex);

	obs_set_output_source(0, NULL);
	os_sleep_ms(250);
	obs_remove_raw_video_callback(raw_video, &w);
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_source_release(source);
	pthread_mutex_destroy(&w.mutex);

	/* The fields shown, in order. From frame 4's first field on, a canvas
	 * at least as fast as the field rate shows every field, and a slower
	 * one never drops two in a row. Frames 1 to 3 are overdue when they
	 * come, so they are not counted. */
	const int first = 2 * 4, end = 2 * (4 + BURST_FRAMES);
	const int allowed_gap = r->canvas_fps >= 2 * r->source_fps ? 0 : 1;
	size_t backwards = 0;
	int last = -1, max_gap = 0, last_counted = first - 1;
	fprintf(stderr, "%s, behind, %ui into %u fps: fields shown:", mode_name(mode), 2 * r->source_fps,
		r->canvas_fps);
	for (size_t i = 0; i < w.count; i++) {
		const int field = w.seen[i];
		if (field < 0 || field == last)
			continue;
		fprintf(stderr, " %d", field);
		if (field < last)
			backwards++;
		if (field >= first && field < end) {
			const int gap = field - last_counted - 1;
			if (gap > max_gap)
				max_gap = gap;
			last_counted = field;
		}
		last = field;
	}
	fprintf(stderr, "\n");

	if (backwards) {
		fprintf(stderr, "%s, behind: the output went back in time %zu times\n", mode_name(mode), backwards);
		fail();
	}
	/* Yadif 2x shows the last frame's second field only with a frame
	 * after it, so the last field need not be shown. */
	if (last_counted < end - 2) {
		fprintf(stderr, "%s, behind, %ui into %u fps: the fields stop at %d (expected %d or %d)\n",
			mode_name(mode), 2 * r->source_fps, r->canvas_fps, last_counted, end - 2, end - 1);
		fail();
	}
	if (max_gap > allowed_gap) {
		fprintf(stderr,
			"%s, behind, %ui into %u fps: up to %d fields skipped in a row from field %d (allowed %d)\n",
			mode_name(mode), 2 * r->source_fps, r->canvas_fps, max_gap, first, allowed_gap);
		fail();
	}
}

struct burst_case {
	enum obs_deinterlace_mode mode;
	struct rates rates;
};

static void run_burst_case(void **state)
{
	const struct burst_case *c = *state;
	if (!obs_up)
		skip();
	check_burst(c->mode, &c->rates);
}

#define BURST_CASES(rate_name, source_fps, canvas_fps)                                                              \
	{"behind_yadif_2x_" #rate_name, run_burst_case, NULL, NULL,                                                 \
	 &(struct burst_case){OBS_DEINTERLACE_MODE_YADIF_2X, {source_fps, canvas_fps}}},                            \
		{"behind_linear_2x_" #rate_name, run_burst_case, NULL, NULL,                                        \
		 &(struct burst_case){OBS_DEINTERLACE_MODE_LINEAR_2X, {source_fps, canvas_fps}}}

struct field_case {
	enum obs_deinterlace_mode mode;
	bool top_first;
	bool unbuffered;
	struct rates rates;
	enum start start;
};

static void run_case(void **state)
{
	const struct field_case *c = *state;
	if (!obs_up)
		skip();
	check_field_order(c->mode, c->top_first, c->unbuffered, &c->rates, c->start);
}

/* source_fps: frames per second, the field rate is twice that */
#define FIELD_CASE(name, mode, top_first, source_fps, canvas_fps) \
	{#name, run_case, NULL, NULL, &(struct field_case){mode, top_first, false, {source_fps, canvas_fps}, START_WARM}}

/* Unbuffered sources time their frames differently; the fields are picked
 * the same way for every mode, so Yadif 2x covers them. */
#define UNBUFFERED_CASES(rate_name, source_fps, canvas_fps)                                                         \
	{"yadif_2x_top_first_unbuffered_" #rate_name, run_case, NULL, NULL,                                         \
	 &(struct field_case){OBS_DEINTERLACE_MODE_YADIF_2X, true, true, {source_fps, canvas_fps}, START_WARM}},    \
		{"yadif_2x_bottom_first_unbuffered_" #rate_name, run_case, NULL, NULL,                              \
		 &(struct field_case){OBS_DEINTERLACE_MODE_YADIF_2X, false, true, {source_fps, canvas_fps}, START_WARM}}

/* The first frame (#1965), with Linear 2x: Yadif 2x has no previous frame
 * to interpolate a new source's first frame from. A new source, buffered and
 * unbuffered; and an unbuffered source after a pause. */
#define FIRST_FRAME_CASES(rate_name, source_fps, canvas_fps)                                                         \
	{"first_frame_linear_2x_" #rate_name, run_case, NULL, NULL,                                                  \
	 &(struct field_case){OBS_DEINTERLACE_MODE_LINEAR_2X, true, false, {source_fps, canvas_fps}, START_NEW}},    \
		{"first_frame_linear_2x_unbuffered_" #rate_name, run_case, NULL, NULL,                               \
		 &(struct field_case){OBS_DEINTERLACE_MODE_LINEAR_2X, true, true, {source_fps, canvas_fps}, START_NEW}}, \
		{"after_pause_linear_2x_unbuffered_" #rate_name, run_case, NULL, NULL,                               \
		 &(struct field_case){OBS_DEINTERLACE_MODE_LINEAR_2X, true, true, {source_fps, canvas_fps}, START_PAUSE}}

/* A buffered source after a pause, with Yadif 2x and Linear 2x: its warm-up
 * frames come before the pause, so Yadif 2x has a previous frame. Its clock
 * stands still while nothing is queued, so it does not fall behind (#2009). */
#define BUFFERED_PAUSE_CASES(rate_name, source_fps, canvas_fps)                                                  \
	{"after_pause_yadif_2x_" #rate_name, run_case, NULL, NULL,                                               \
	 &(struct field_case){OBS_DEINTERLACE_MODE_YADIF_2X, true, false, {source_fps, canvas_fps}, START_PAUSE}}, \
		{"after_pause_linear_2x_" #rate_name, run_case, NULL, NULL,                                      \
		 &(struct field_case){OBS_DEINTERLACE_MODE_LINEAR_2X, true, false, {source_fps, canvas_fps},     \
				      START_PAUSE}}

#define FIELD_CASES(rate_name, source_fps, canvas_fps)                                                                         \
	FIELD_CASE(yadif_2x_top_first_##rate_name, OBS_DEINTERLACE_MODE_YADIF_2X, true, source_fps, canvas_fps),               \
		FIELD_CASE(yadif_2x_bottom_first_##rate_name, OBS_DEINTERLACE_MODE_YADIF_2X, false, source_fps, canvas_fps),   \
		FIELD_CASE(linear_2x_top_first_##rate_name, OBS_DEINTERLACE_MODE_LINEAR_2X, true, source_fps, canvas_fps),     \
		FIELD_CASE(linear_2x_bottom_first_##rate_name, OBS_DEINTERLACE_MODE_LINEAR_2X, false, source_fps, canvas_fps), \
		FIELD_CASE(discard_2x_top_first_##rate_name, OBS_DEINTERLACE_MODE_RETRO, true, source_fps, canvas_fps),        \
		FIELD_CASE(discard_2x_bottom_first_##rate_name, OBS_DEINTERLACE_MODE_RETRO, false, source_fps, canvas_fps)

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	/* obs_startup() needs a display on Linux (run under xvfb-run); the
	 * cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	if (!reset_canvas(60)) {
		fprintf(stderr, "skipped: no video\n");
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	obs_register_source(&interlaced_source);
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		/* matched rates: the canvas shows each field once */
		FIELD_CASES(60i_into_60, 30, 60),
		FIELD_CASES(50i_into_50, 25, 50),
		/* canvas faster than the field rate: every field, some repeated */
		FIELD_CASES(50i_into_60, 25, 60),
		/* canvas slower than the field rate: some fields dropped, never two in a row */
		FIELD_CASES(50i_into_30, 25, 30),
		FIELD_CASES(60i_into_50, 30, 50),
		UNBUFFERED_CASES(60i_into_60, 30, 60),
		UNBUFFERED_CASES(50i_into_50, 25, 50),
		UNBUFFERED_CASES(50i_into_60, 25, 60),
		UNBUFFERED_CASES(50i_into_30, 25, 30),
		UNBUFFERED_CASES(60i_into_50, 30, 50),
		FIRST_FRAME_CASES(60i_into_60, 30, 60),
		FIRST_FRAME_CASES(50i_into_50, 25, 50),
		FIRST_FRAME_CASES(50i_into_60, 25, 60),
		FIRST_FRAME_CASES(50i_into_30, 25, 30),
		FIRST_FRAME_CASES(60i_into_50, 30, 50),
		BUFFERED_PAUSE_CASES(60i_into_60, 30, 60),
		BUFFERED_PAUSE_CASES(50i_into_50, 25, 50),
		BUFFERED_PAUSE_CASES(50i_into_60, 25, 60),
		BUFFERED_PAUSE_CASES(50i_into_30, 25, 30),
		BUFFERED_PAUSE_CASES(60i_into_50, 30, 50),
		BURST_CASES(60i_into_60, 30, 60),
		BURST_CASES(50i_into_50, 25, 50),
		BURST_CASES(50i_into_60, 25, 60),
		BURST_CASES(60i_into_50, 30, 50),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
