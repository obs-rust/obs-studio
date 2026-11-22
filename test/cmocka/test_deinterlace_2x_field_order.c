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
 * field may be dropped between two shown ones. One field may be skipped in a
 * whole run, or held one frame too long, for a single late tick under load
 * (#456); a second one is a cadence bug. Buffered and unbuffered sources are
 * timed by different code, so both are covered.
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
/* Output frames to keep recording after the last measured field has been
 * shown, and the most to wait for it: the fixed sleep this replaces was 300 ms
 * (#2796). The frames after the last field also record the next one, and a
 * late step back in time, the way the sleep did. */
#define END_TICKS 4
#define END_WAIT_MS 300
/* Unrecognised (black) output frames in a row that mean the output has
 * drained, and the most to wait for them (the old fixed sleep was 250 ms). */
#define DRAIN_TICKS 2
#define DRAIN_WAIT_MS 250

static bool obs_up;

struct watch {
	pthread_mutex_t mutex;
	bool recording;
	size_t ticks; /* every output frame, recorded or not */
	size_t count;
	int seen[MAX_SEEN]; /* field index, or -1 for an unrecognised frame */
	/* The output frame repeats the frame before it: a tick libobs did not
	 * render (#5374). When the graphics thread oversleeps past several
	 * ticks, video_sleep counts the ticks it missed as lagged frames and
	 * video-io delivers the last rendered frame once per missed tick, from
	 * the same cache entry (video_output_cur_frame); every rendered frame
	 * gets an entry of its own. */
	bool repeat[MAX_SEEN];
	const uint8_t *last_data; /* the cache entry of the last output frame */
	int max_field;            /* the highest field recorded, recording or not */
	size_t black_run;         /* unrecognised frames in a row */
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
	const bool repeat = frame->data[0] == w->last_data;
	w->last_data = frame->data[0];
	w->ticks++;
	if (field < 0) {
		w->black_run++;
	} else {
		w->black_run = 0;
		if (field > w->max_field)
			w->max_field = field;
	}
	if (w->recording && w->count < MAX_SEEN) {
		w->repeat[w->count] = repeat;
		w->seen[w->count++] = field;
	}
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

/* Pushes FRAMES interlaced frames at fps, timestamped like a capture card.
 * The caller waits for the last fields to be shown (#2796). */
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

/* Waits until field `last` has been shown and END_TICKS output frames have
 * come since the call, and at most END_WAIT_MS: the fixed sleep this replaces
 * was END_WAIT_MS whether or not the frames were already rendered (#2796). A
 * wait that reaches its limit reports it and the case keeps the old timing. */
static void wait_shown(struct watch *w, int last)
{
	pthread_mutex_lock(&w->mutex);
	const size_t ticks = w->ticks;
	pthread_mutex_unlock(&w->mutex);

	int field = 0;
	for (int waited = 0; waited < END_WAIT_MS; waited++) {
		pthread_mutex_lock(&w->mutex);
		const bool shown = w->max_field >= last && w->ticks >= ticks + END_TICKS;
		field = w->max_field;
		pthread_mutex_unlock(&w->mutex);
		if (shown)
			return;
		os_sleep_ms(1);
	}
	fprintf(stderr, "wait_shown: field %d shown of %d, frames not rendered after %d ms\n", field, last,
		END_WAIT_MS);
}

/* Waits until the output has rendered DRAIN_TICKS unrecognised (black) frames
 * in a row, and at most DRAIN_WAIT_MS (the fixed drain sleep this replaces). */
static void wait_drained(struct watch *w)
{
	size_t run = 0;
	for (int waited = 0; waited < DRAIN_WAIT_MS; waited++) {
		pthread_mutex_lock(&w->mutex);
		const bool drained = w->black_run >= DRAIN_TICKS;
		run = w->black_run;
		pthread_mutex_unlock(&w->mutex);
		if (drained)
			return;
		os_sleep_ms(1);
	}
	fprintf(stderr, "wait_drained: %zu unrecognised frames in a row after %d ms\n", run, DRAIN_WAIT_MS);
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

/* How the fields of a case must be paced. */
struct cadence {
	int startup;         /* the first fields, not checked for skips */
	int unheld;          /* the first fields, not checked for holds */
	size_t allowed_gap;  /* fields a slower canvas drops between two shown */
	size_t allowed_held; /* output frames a field may be shown for (SIZE_MAX: any) */
};

/* What the output showed, counted against a cadence. */
struct field_count {
	int shown[MAX_SEEN]; /* the fields in the order they were shown */
	size_t num;          /* fields shown */
	size_t unknown;      /* unrecognised output frames */
	size_t backwards;    /* steps back in time */
	size_t skipped;      /* fields never shown, in total */
	size_t max_gap;      /* the longest skip */
	/* How many separate skips went beyond the cadence's allowance: a slower
	 * canvas drops fields as a matter of course (50i into 30 fps drops two
	 * in every five), so the count that matters is of the skips that exceed
	 * the allowance, not of the fields skipped in total. */
	size_t over_gaps;
	size_t max_held;  /* the most output frames one field was shown for */
	size_t over_held; /* fields held longer than the cadence allows */
	size_t stalled;   /* output frames that repeat a tick libobs did not render */
	size_t excused;   /* fields skipped for ticks libobs did not render */
};

/* Collapse repeats: the fields in the order they were shown. Count the fields
 * skipped between two shown ones, except in the first `startup` fields: after
 * the warm-up, its last frame and the first measured one are stamped back to
 * back, so the first has no frame duration to time its fields by (and Yadif
 * needs a previous frame). Also count how many output frames each field is
 * held for (the last one is held until the case ends), except the first
 * `unheld` fields. A pure function of the recorded frames, so its rules are
 * tested on fixed sequences.
 *
 * A stall is not a cadence bug (#5374). When the graphics thread misses N
 * ticks, video-io repeats the last rendered frame for them, and the next
 * render shows the field that is due by then, as it must to stay in sync with
 * audio: up to N fields were never due at a rendered tick. So a repeated
 * frame is not a hold, and a skip is excused by up to the number of ticks not
 * rendered since the field before it was shown. Nothing else changes: with
 * no repeated frames the rules are the ones above. */
static void count_fields(const int *seen, const bool *repeat, size_t count, const struct cadence *c,
			 struct field_count *f)
{
	memset(f, 0, sizeof(*f));
	size_t held = 0, stall = 0;
	for (size_t i = 0; i < count; i++) {
		if (repeat[i]) {
			f->stalled++;
			stall++;
			continue;
		}
		if (seen[i] < 0) {
			f->unknown++;
			continue;
		}
		if (f->num && f->shown[f->num - 1] == seen[i]) {
			held++;
			continue;
		}
		if (f->num && f->shown[f->num - 1] >= c->unheld) {
			if (held > c->allowed_held)
				f->over_held++;
			if (held > f->max_held)
				f->max_held = held;
		}
		held = 1;
		if (f->num && seen[i] < f->shown[f->num - 1])
			f->backwards++;
		const int last = f->num ? f->shown[f->num - 1] : -1;
		if ((f->num || !c->startup) && seen[i] > last + 1 && seen[i] > c->startup) {
			int from = last < c->startup - 1 ? c->startup - 1 : last;
			size_t gap = (size_t)(seen[i] - from - 1);
			f->skipped += gap;
			const size_t excused = gap < stall ? gap : stall;
			f->excused += excused;
			gap -= excused;
			if (gap > c->allowed_gap)
				f->over_gaps++;
			if (gap > f->max_gap)
				f->max_gap = gap;
		}
		f->shown[f->num++] = seen[i];
		stall = 0;
	}
}

/* The same late tick can cost a field: the canvas misses the tick that would
 * have shown it, so one skip may go one field past the cadence in a whole
 * run, and never two in a row. A second such skip, or a longer one, is a
 * cadence bug (#456). Fields the slower canvas drops by design
 * (gap == allowed_gap) do not count. */
static bool skips_ok(const struct field_count *f, const struct cadence *c)
{
	return f->max_gap <= c->allowed_gap + 1 && f->over_gaps <= 1;
}

/* One field may be held a frame longer than the cadence allows: a single late
 * tick under load (#456). A second one, or a longer hold, is a cadence bug. */
static bool holds_ok(const struct field_count *f, const struct cadence *c)
{
	return c->allowed_held == SIZE_MAX || (f->max_held <= c->allowed_held + 1 && f->over_held <= 1);
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

	const uint32_t lagged = obs_get_lagged_frames();
	const uint32_t dropped = video_output_get_skipped_frames(obs_get_video());
	pthread_mutex_lock(&w.mutex);
	w.recording = true;
	pthread_mutex_unlock(&w.mutex);

	play(source, top_first, r->source_fps);
	/* Yadif 2x shows the last frame's first field on its own; the second
	 * needs a frame after it. */
	wait_shown(&w, 2 * FRAMES - 2);

	pthread_mutex_lock(&w.mutex);
	w.recording = false;
	pthread_mutex_unlock(&w.mutex);
	const uint32_t lagged_now = obs_get_lagged_frames() - lagged;
	const uint32_t dropped_now = video_output_get_skipped_frames(obs_get_video()) - dropped;

	/* Let the output drain to black so the next case starts clean. */
	obs_set_output_source(0, NULL);
	wait_drained(&w);
	obs_remove_raw_video_callback(raw_video, &w);
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_source_release(source);
	pthread_mutex_destroy(&w.mutex);

	/* A canvas at least as fast as the field rate can show every field,
	 * each for at most canvas / field rate frames, rounded up (50i into 60
	 * fps: 5 fields once, 1 twice, like 50p); a slower one must drop some,
	 * but never two in a row. An unbuffered source shows a frame as soon
	 * as it arrives, so one that arrives just after a tick repeats the
	 * field before it: only buffered sources are held to the cadence.
	 * After a pause or for a new source, count every field from field 0,
	 * the first one shown included (#1965); only a buffered new source's
	 * first frame may be held longer than the cadence, as the buffered
	 * clock starts from it. */
	const int startup = start == START_WARM ? STARTUP_FIELDS : 0;
	const struct cadence cadence = {
		.startup = startup,
		.unheld = start == START_NEW && !unbuffered ? 2 : startup,
		.allowed_gap = r->canvas_fps >= field_rate ? 0 : 1,
		.allowed_held = unbuffered ? SIZE_MAX : (r->canvas_fps + field_rate - 1) / field_rate,
	};
	const size_t expected = r->canvas_fps >= field_rate ? 2 * FRAMES : 2 * FRAMES * r->canvas_fps / field_rate;

	static struct field_count f;
	count_fields(w.seen, w.repeat, w.count, &cadence, &f);

	fprintf(stderr,
		"%s, %ui into %u fps: %zu output frames, %zu unrecognised, %zu fields shown, %zu skipped "
		"(at most %zu in a row), held for up to %zu frames, %zu steps back in time; %zu repeated frames "
		"(%u lagged, %u dropped by video-io) excuse %zu skipped fields:",
		what, field_rate, r->canvas_fps, w.count, f.unknown, f.num, f.skipped, f.max_gap, f.max_held,
		f.backwards, f.stalled, lagged_now, dropped_now, f.excused);
	for (size_t i = 0; i < f.num; i++)
		fprintf(stderr, " %d", f.shown[i]);
	fprintf(stderr, "\n");

	if (f.num < expected * 3 / 4) {
		fprintf(stderr, "%s: only %zu of %d fields were shown (expected about %zu)\n", what, f.num, 2 * FRAMES,
			expected);
		fail();
	}
	if (f.backwards) {
		fprintf(stderr, "%s: the output went back in time %zu times\n", what, f.backwards);
		fail();
	}
	if (!skips_ok(&f, &cadence)) {
		fprintf(stderr,
			"%s, %ui into %u fps: %zu fields skipped, %zu skips past the cadence (at most %zu in a row), "
			"allowed one skip of %zu\n",
			what, field_rate, r->canvas_fps, f.skipped, f.over_gaps, f.max_gap, cadence.allowed_gap + 1);
		fail();
	}
	if (!holds_ok(&f, &cadence)) {
		fprintf(stderr,
			"%s, %ui into %u fps: a field was held for %zu frames (allowed %zu, once %zu); %zu over\n",
			what, field_rate, r->canvas_fps, f.max_held, cadence.allowed_held, cadence.allowed_held + 1,
			f.over_held);
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
	wait_shown(&w, 2 * (4 + BURST_FRAMES) - 2);

	pthread_mutex_lock(&w.mutex);
	w.recording = false;
	pthread_mutex_unlock(&w.mutex);

	obs_set_output_source(0, NULL);
	wait_drained(&w);
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

/* The accounting on fixed sequences (#5374): 50i into 50 fps after a pause,
 * where every field is due at its own tick. A repeated frame (r) is a tick
 * libobs did not render: video-io repeated the frame before it. A stall of N
 * ticks costs up to N fields, and only those; anything past it, or a skip or
 * hold with no stall, is still a cadence bug. These need no video, so they
 * run without a display. */
static const struct cadence matched_rate = {.startup = 0, .unheld = 0, .allowed_gap = 0, .allowed_held = 1};

static void count_sequence(const int *seen, const bool *repeat, size_t count, struct field_count *f)
{
	count_fields(seen, repeat, count, &matched_rate, f);
}

/* A 3-tick stall: field 2 is repeated for the 3 ticks not rendered, the
 * deinterlacer catches up to wall time and fields 3-5 were never due at a
 * rendered tick (the failing run of #5374). */
static void stall_excuses_its_skip(void **state)
{
	UNUSED_PARAMETER(state);
	static const int seen[] = {0, 1, 2, 2, 2, 2, 6, 7, 8};
	static const bool repeat[] = {0, 0, 0, 1, 1, 1, 0, 0, 0};
	static struct field_count f;
	count_sequence(seen, repeat, 9, &f);
	assert_int_equal(f.stalled, 3);
	assert_int_equal(f.skipped, 3);
	assert_int_equal(f.excused, 3);
	assert_true(skips_ok(&f, &matched_rate));
	assert_true(holds_ok(&f, &matched_rate));
}

/* The same output with every tick rendered: a cadence bug. */
static void skip_without_stall_fails(void **state)
{
	UNUSED_PARAMETER(state);
	static const int seen[] = {0, 1, 2, 2, 2, 2, 6, 7, 8};
	static const bool repeat[9] = {0};
	static struct field_count f;
	count_sequence(seen, repeat, 9, &f);
	assert_false(skips_ok(&f, &matched_rate));
	assert_false(holds_ok(&f, &matched_rate));
}

/* A 1-tick stall excuses one field, not three. */
static void skip_past_stall_fails(void **state)
{
	UNUSED_PARAMETER(state);
	static const int seen[] = {0, 1, 2, 2, 6, 7, 8};
	static const bool repeat[] = {0, 0, 0, 1, 0, 0, 0};
	static struct field_count f;
	count_sequence(seen, repeat, 7, &f);
	assert_false(skips_ok(&f, &matched_rate));
}

/* A stall excuses the skip that follows it, not one after a later field. */
static void stall_in_another_window_fails(void **state)
{
	UNUSED_PARAMETER(state);
	static const int seen[] = {0, 1, 1, 2, 6, 7};
	static const bool repeat[] = {0, 0, 1, 0, 0, 0};
	static struct field_count f;
	count_sequence(seen, repeat, 6, &f);
	assert_false(skips_ok(&f, &matched_rate));
}

/* Frames repeated for ticks not rendered are not holds. */
static void stall_repeats_are_not_holds(void **state)
{
	UNUSED_PARAMETER(state);
	static const int seen[] = {0, 1, 2, 2, 2, 3, 4};
	static const bool repeat[] = {0, 0, 0, 1, 1, 0, 0};
	static struct field_count f;
	count_sequence(seen, repeat, 7, &f);
	assert_int_equal(f.max_held, 1);
	assert_true(holds_ok(&f, &matched_rate));
	assert_true(skips_ok(&f, &matched_rate));
}

/* A field rendered at three ticks in a row is held past the cadence. */
static void hold_without_stall_fails(void **state)
{
	UNUSED_PARAMETER(state);
	static const int seen[] = {0, 1, 1, 1, 2, 3};
	static const bool repeat[6] = {0};
	static struct field_count f;
	count_sequence(seen, repeat, 6, &f);
	assert_false(holds_ok(&f, &matched_rate));
}

/* One late tick with no stall reported still costs at most one field, once
 * (#2904); a second one fails. */
static void one_late_tick_is_allowed_once(void **state)
{
	UNUSED_PARAMETER(state);
	static const int once[] = {0, 1, 2, 4, 5, 6};
	static const int twice[] = {0, 1, 2, 4, 5, 7};
	static const bool repeat[6] = {0};
	static struct field_count f;
	count_sequence(once, repeat, 6, &f);
	assert_true(skips_ok(&f, &matched_rate));
	count_sequence(twice, repeat, 6, &f);
	assert_false(skips_ok(&f, &matched_rate));
}

/* Named for the 50i into 50 entry, so CMOCKA_TEST_FILTER runs them there. */
#define ACCOUNTING_CASE(name) {"accounting_" #name "_50i_into_50", name, NULL, NULL, NULL}

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
		/* the accounting; the names end in a registered rate, so they run */
		ACCOUNTING_CASE(stall_excuses_its_skip),
		ACCOUNTING_CASE(skip_without_stall_fails),
		ACCOUNTING_CASE(skip_past_stall_fails),
		ACCOUNTING_CASE(stall_in_another_window_fails),
		ACCOUNTING_CASE(stall_repeats_are_not_holds),
		ACCOUNTING_CASE(hold_without_stall_fails),
		ACCOUNTING_CASE(one_late_tick_is_allowed_once),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
