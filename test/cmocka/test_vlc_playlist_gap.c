/*
 * VLC Video Source must play every playlist item to its end
 * (upstream obsproject/obs-studio#9345, duplicate #3712, #11984).
 *
 * Users report that about a second is cut at every playlist track boundary:
 * the end of a track is missing from recordings and streams.
 *
 * The test plays generated WAV files through the real vlc_source module and
 * measures the program mix (what outputs and recordings receive). Every file
 * is a tone whose peak level rises linearly over the file, so the level of a
 * block of the mix tells which file it came from and how far into that file
 * it is. The test checks how much of each file reaches the mix and which part
 * of a file is the last (first) one heard.
 *
 * The playback cases need core video (run under xvfb-run) and skip without a
 * display or when libVLC is not installed; the test then exits with 77 so
 * ctest reports it as skipped. The analysis cases feed the analysis a
 * generated mix and always run.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <obs-module.h>
#include <util/platform.h>
#include <util/threading.h>

#define RATE 48000
#define FILE_SECONDS 3.0
#define FILE_FRAMES ((uint32_t)(RATE * FILE_SECONDS))
#define MAX_FILES 3

/* File i rises from LEVEL_LO(i) to LEVEL_HI(i); the ranges do not overlap. */
#define LEVEL_LO(i) (0.10 + 0.25 * (i))
#define LEVEL_HI(i) (0.30 + 0.25 * (i))
/* Enough to tell the files apart in a 1024-frame block. */
#define TONE_HZ 1000.0

static double level_at(unsigned file, uint32_t frame)
{
	return LEVEL_LO(file) + (LEVEL_HI(file) - LEVEL_LO(file)) * (double)frame / FILE_FRAMES;
}

static void put_u16(FILE *f, uint16_t v)
{
	fwrite(&v, 2, 1, f);
}

static void put_u32(FILE *f, uint32_t v)
{
	fwrite(&v, 4, 1, f);
}

/* Stereo 32-bit float WAV. The tone starts and ends at full level, with no
 * silence at either end, as in the report. */
static bool write_wav(const char *path, unsigned file)
{
	FILE *f = fopen(path, "wb");
	if (!f)
		return false;

	const unsigned channels = 2;
	const uint32_t data_size = FILE_FRAMES * channels * 4;

	fwrite("RIFF", 1, 4, f);
	put_u32(f, 4 + 8 + 16 + 8 + data_size);
	fwrite("WAVE", 1, 4, f);
	fwrite("fmt ", 1, 4, f);
	put_u32(f, 16);
	put_u16(f, 3); /* WAVE_FORMAT_IEEE_FLOAT */
	put_u16(f, (uint16_t)channels);
	put_u32(f, RATE);
	put_u32(f, RATE * channels * 4);
	put_u16(f, (uint16_t)(channels * 4));
	put_u16(f, 32);
	fwrite("data", 1, 4, f);
	put_u32(f, data_size);

	for (uint32_t i = 0; i < FILE_FRAMES; i++) {
		float v = (float)(level_at(file, i) * sin(2.0 * M_PI * TONE_HZ * i / RATE));
		fwrite(&v, 4, 1, f);
		fwrite(&v, 4, 1, f);
	}

	fclose(f);
	return true;
}

/* ------------------------------------------------------------------------ */
/* Program mix analysis                                                     */

struct file_stats {
	uint64_t frames;   /* frames of the mix that carry this file */
	double first_pos;  /* position in the file (s) of the first such block */
	double last_pos;   /* position in the file (s) of the last such block */
	uint64_t first_at; /* index of the mix frame where the file first appears */
	uint64_t last_at;  /* index of the mix frame after the file was last heard */
	bool seen;
};

/* Mix blocks kept for the analysis: the longest playlist waits 6 s per file
 * plus 4 s (play_playlist), and the mix runs on while it is torn down. */
#define MAX_BLOCKS ((MAX_FILES * 6 + 4) * RATE / AUDIO_OUTPUT_FRAMES + 64)

struct mix_block {
	uint64_t start; /* index of the block's first mix frame */
	uint32_t frames;
	float peak;
};

struct capture {
	pthread_mutex_t mutex;
	uint64_t mix_frames;
	uint64_t frames_since_sound;
	bool heard;
	struct mix_block blocks[MAX_BLOCKS];
	unsigned num_blocks;
	struct file_stats files[MAX_FILES];
	unsigned num_files;
};

/* Peak level of a block of the tone -> file index and position (s). */
static bool classify(const struct capture *cap, double peak, unsigned *file, double *pos)
{
	for (unsigned i = 0; i < cap->num_files; i++) {
		double lo = LEVEL_LO(i), hi = LEVEL_HI(i);
		if (peak >= lo - 0.01 && peak <= hi + 0.01) {
			double p = (peak - lo) / (hi - lo) * FILE_SECONDS;
			*file = i;
			*pos = p < 0.0 ? 0.0 : (p > FILE_SECONDS ? FILE_SECONDS : p);
			return true;
		}
	}
	return false;
}

static void mix_cb(void *param, size_t mix_idx, struct audio_data *data)
{
	struct capture *cap = param;
	const float *l = (const float *)data->data[0];
	UNUSED_PARAMETER(mix_idx);

	double peak = 0.0;
	for (uint32_t i = 0; l && i < data->frames; i++) {
		double a = fabs((double)l[i]);
		if (a > peak)
			peak = a;
	}

	pthread_mutex_lock(&cap->mutex);
	if (peak > 0.05) {
		cap->heard = true;
		cap->frames_since_sound = 0;
	} else {
		cap->frames_since_sound += data->frames;
	}
	if (cap->num_blocks < MAX_BLOCKS) {
		struct mix_block *b = &cap->blocks[cap->num_blocks++];
		b->start = cap->mix_frames;
		b->frames = data->frames;
		b->peak = (float)peak;
	}
	cap->mix_frames += data->frames;
	pthread_mutex_unlock(&cap->mutex);
}

/* A block's peak is its file's level only when the block holds enough of
 * the tone: one that holds just the first (last) few frames of a file, at a
 * track boundary with silence on the other side, peaks lower, at the level of
 * another file or of an earlier point of the file. Count a block as audio of
 * a file only when a block next to it is from the same point of that file
 * too: a block lasts 21 ms, so allow 0.1 s between the two positions. */
#define NEIGHBOUR_POS_TOLERANCE 0.1

/* Mix blocks -> per-file stats. */
static void analyze(struct capture *cap)
{
	unsigned file[MAX_BLOCKS];
	double pos[MAX_BLOCKS];
	bool classified[MAX_BLOCKS];
	unsigned n = cap->num_blocks;

	memset(cap->files, 0, sizeof(cap->files));
	for (unsigned b = 0; b < n; b++)
		classified[b] = cap->blocks[b].peak > 0.05 && classify(cap, cap->blocks[b].peak, &file[b], &pos[b]);

	for (unsigned b = 0; b < n; b++) {
		if (!classified[b])
			continue;

		bool agrees = false;
		if (b > 0 && classified[b - 1] && file[b - 1] == file[b] &&
		    fabs(pos[b - 1] - pos[b]) <= NEIGHBOUR_POS_TOLERANCE)
			agrees = true;
		if (b + 1 < n && classified[b + 1] && file[b + 1] == file[b] &&
		    fabs(pos[b + 1] - pos[b]) <= NEIGHBOUR_POS_TOLERANCE)
			agrees = true;

		const struct mix_block *blk = &cap->blocks[b];
		if (!agrees) {
			fprintf(stderr,
				"mix %.3f s: a block peaks at %.3f, file %u at %.3f s, but no block next to it "
				"does; not counted\n",
				(double)blk->start / RATE, blk->peak, file[b] + 1, pos[b]);
			continue;
		}

		struct file_stats *fs = &cap->files[file[b]];
		if (!fs->seen) {
			fs->seen = true;
			fs->first_pos = pos[b];
			fs->first_at = blk->start;
		}
		fs->last_pos = pos[b];
		fs->last_at = blk->start + blk->frames;
		fs->frames += blk->frames;
	}
}

/* ------------------------------------------------------------------------ */
/* libobs setup                                                             */

/* libobs is up (a display is available) */
static bool obs_up;
/* ... and the module registered vlc_source (libVLC is installed) */
static bool vlc_ready;

static int start_obs(void **state)
{
	UNUSED_PARAMETER(state);
	const char *display = getenv("DISPLAY");
	if (!display || !*display)
		return 0;

	if (!obs_startup("en-US", NULL, NULL))
		return 0;

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS)
		return 0;

	struct obs_audio_info oai = {.samples_per_sec = RATE, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai))
		return 0;

	obs_up = true;

	obs_module_t *mod = NULL;
	if (obs_open_module(&mod, VLC_MODULE_PATH, VLC_DATA_PATH) != MODULE_SUCCESS || !obs_init_module(mod))
		return 0;

	/* the module only registers vlc_source when libVLC is installed */
	vlc_ready = obs_source_get_display_name("vlc_source") != NULL;
	return 0;
}

static int stop_obs(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

/* Plays num_files files as one playlist through vlc_source and fills cap
 * from the program mix. */
static void play_playlist(unsigned num_files, struct capture *cap)
{
	char paths[MAX_FILES][512];
	const char *tmp = getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp";
	uint64_t id = os_gettime_ns();

	obs_data_t *settings = obs_data_create();
	obs_data_array_t *list = obs_data_array_create();
	for (unsigned i = 0; i < num_files; i++) {
		snprintf(paths[i], sizeof(paths[i]), "%s/obs-vlc-gap-%llu-%u.wav", tmp, (unsigned long long)id, i);
		if (!write_wav(paths[i], i)) {
			fprintf(stderr, "could not write %s\n", paths[i]);
			fail();
		}
		obs_data_t *item = obs_data_create();
		obs_data_set_string(item, "value", paths[i]);
		obs_data_array_push_back(list, item);
		obs_data_release(item);
	}
	obs_data_set_array(settings, "playlist", list);
	obs_data_set_string(settings, "playback_behavior", "always_play");
	obs_data_set_bool(settings, "loop", false);
	obs_data_array_release(list);

	memset(cap, 0, sizeof(*cap));
	pthread_mutex_init(&cap->mutex, NULL);
	cap->num_files = num_files;

	obs_add_raw_audio_callback(0, NULL, mix_cb, cap);

	obs_source_t *src = obs_source_create("vlc_source", "vlc", settings, NULL);
	obs_data_release(settings);
	if (!src) {
		obs_remove_raw_audio_callback(0, mix_cb, cap);
		fprintf(stderr, "could not create vlc_source\n");
		fail();
	}
	obs_set_output_source(0, src);

	/* Until the playlist has been heard and the mix stayed silent for
	 * 1.5 s afterwards, or 6 s per file plus 4 s. */
	uint64_t deadline = os_gettime_ns() + (uint64_t)(num_files * 6 + 4) * 1000000000ULL;
	for (;;) {
		os_sleep_ms(20);
		pthread_mutex_lock(&cap->mutex);
		bool done = cap->heard && cap->frames_since_sound > RATE * 3 / 2;
		pthread_mutex_unlock(&cap->mutex);
		if (done || os_gettime_ns() > deadline)
			break;
	}

	obs_set_output_source(0, NULL);
	obs_remove_raw_audio_callback(0, mix_cb, cap);
	obs_source_release(src);
	for (unsigned i = 0; i < num_files; i++)
		os_unlink(paths[i]);
	pthread_mutex_destroy(&cap->mutex);
	analyze(cap);

	for (unsigned i = 0; i < num_files; i++) {
		const struct file_stats *fs = &cap->files[i];
		fprintf(stderr,
			"file %u: %.3f s of %.1f s in the mix, heard from %.3f s to %.3f s of the file, "
			"mix %.3f s - %.3f s\n",
			i + 1, (double)fs->frames / RATE, FILE_SECONDS, fs->first_pos, fs->last_pos,
			(double)fs->first_at / RATE, (double)fs->last_at / RATE);
	}
}

/* A 1024-frame block of the mix is 21 ms. libVLC needs some 60-130 ms to
 * open the next item, and libobs pulls a start that is less than 70 ms late
 * back to where the previous track ended; the first time that happens it may
 * drop that much while it adds audio buffering. Allow for both per edge; the
 * reported cut is about a second. */
#define TOLERANCE 0.15

static bool check_file(const struct capture *cap, unsigned i)
{
	const struct file_stats *fs = &cap->files[i];
	bool ok = true;

	if (!fs->seen) {
		fprintf(stderr, "file %u never reached the program mix\n", i + 1);
		return false;
	}
	if (fs->first_pos > TOLERANCE) {
		fprintf(stderr, "file %u: the first %.3f s are missing from the mix\n", i + 1, fs->first_pos);
		ok = false;
	}
	if (fs->last_pos < FILE_SECONDS - TOLERANCE) {
		fprintf(stderr, "file %u: the last %.3f s are missing from the mix\n", i + 1,
			FILE_SECONDS - fs->last_pos);
		ok = false;
	}
	if ((double)fs->frames / RATE < FILE_SECONDS - 2 * TOLERANCE) {
		fprintf(stderr, "file %u: only %.3f s of %.1f s reached the mix\n", i + 1, (double)fs->frames / RATE,
			FILE_SECONDS);
		ok = false;
	}
	return ok;
}

/* ------------------------------------------------------------------------ */
/* Analysis checks (no display or libVLC needed)                            */

/* Feeds the analysis a program mix made of the test files: file i after
 * gap[i] frames of silence, its first keep[i] frames, in the 1024-frame
 * blocks in which libobs hands the mix to raw audio callbacks. */
static void feed_mix(struct capture *cap, unsigned num_files, const uint32_t *gap, const uint32_t *keep)
{
	size_t total = AUDIO_OUTPUT_FRAMES;
	for (unsigned i = 0; i < num_files; i++)
		total += gap[i] + keep[i];

	float *mix = calloc(total, sizeof(float));
	assert_non_null(mix);
	size_t at = 0;
	for (unsigned i = 0; i < num_files; i++) {
		at += gap[i];
		for (uint32_t f = 0; f < keep[i]; f++)
			mix[at++] = (float)(level_at(i, f) * sin(2.0 * M_PI * TONE_HZ * f / RATE));
	}

	memset(cap, 0, sizeof(*cap));
	pthread_mutex_init(&cap->mutex, NULL);
	cap->num_files = num_files;
	for (size_t pos = 0; pos + AUDIO_OUTPUT_FRAMES <= total; pos += AUDIO_OUTPUT_FRAMES) {
		struct audio_data block = {.frames = AUDIO_OUTPUT_FRAMES};
		block.data[0] = (uint8_t *)(mix + pos);
		mix_cb(cap, 0, &block);
	}
	pthread_mutex_destroy(&cap->mutex);
	free(mix);
	analyze(cap);
}

/* ------------------------------------------------------------------------ */
/* Cases                                                                    */

/* The reported bug: two files, no gap in either file. */
static void playlist_plays_every_file_to_the_end(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip(); /* no display */
	if (!vlc_ready)
		skip(); /* libVLC is not installed */

	struct capture cap;
	play_playlist(2, &cap);

	bool ok = check_file(&cap, 0) & check_file(&cap, 1);
	if (ok && cap.files[1].first_at + RATE / 10 < cap.files[0].last_at) {
		fprintf(stderr, "file 2 starts before file 1 has ended\n");
		ok = false;
	}
	if (!ok) {
		fprintf(stderr, "playlist track boundary cut audio\n");
		fail();
	}
}

/* Three files: every boundary, including one after an earlier boundary. */
static void playlist_of_three_plays_every_file_to_the_end(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	if (!vlc_ready)
		skip();

	struct capture cap;
	play_playlist(3, &cap);

	bool ok = check_file(&cap, 0) & check_file(&cap, 1) & check_file(&cap, 2);
	if (!ok) {
		fprintf(stderr, "playlist track boundary cut audio\n");
		fail();
	}
}

/* Source-level capture: how many frames vlc_source has handed to libobs. */
struct delivered {
	pthread_mutex_t mutex;
	uint64_t frames;
};

static void delivered_cb(void *param, obs_source_t *source, const struct audio_data *audio, bool muted)
{
	struct delivered *d = param;
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(muted);
	pthread_mutex_lock(&d->mutex);
	d->frames += audio->frames;
	pthread_mutex_unlock(&d->mutex);
}

static uint64_t delivered_frames(struct delivered *d)
{
	pthread_mutex_lock(&d->mutex);
	uint64_t frames = d->frames;
	pthread_mutex_unlock(&d->mutex);
	return frames;
}

/* Guard: stopping while libVLC is finishing a track (all of it has been
 * handed to libobs, but it has not been played yet) must not hang the
 * thread that stops the source. */
static void stop_at_track_end_is_prompt(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	if (!vlc_ready)
		skip();

	char path[512];
	snprintf(path, sizeof(path), "%s/obs-vlc-gap-%llu-stop.wav", getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp",
		 (unsigned long long)os_gettime_ns());
	if (!write_wav(path, 0)) {
		fprintf(stderr, "could not write %s\n", path);
		fail();
	}

	obs_data_t *settings = obs_data_create();
	obs_data_array_t *list = obs_data_array_create();
	obs_data_t *item = obs_data_create();
	obs_data_set_string(item, "value", path);
	obs_data_array_push_back(list, item);
	obs_data_release(item);
	/* the same file twice, so that there is a next item to start */
	item = obs_data_create();
	obs_data_set_string(item, "value", path);
	obs_data_array_push_back(list, item);
	obs_data_release(item);
	obs_data_set_array(settings, "playlist", list);
	obs_data_set_string(settings, "playback_behavior", "always_play");
	obs_data_set_bool(settings, "loop", false);
	obs_data_array_release(list);

	struct delivered delivered = {0};
	pthread_mutex_init(&delivered.mutex, NULL);
	obs_source_t *src = obs_source_create("vlc_source", "vlc", settings, NULL);
	obs_data_release(settings);
	if (!src) {
		os_unlink(path);
		pthread_mutex_destroy(&delivered.mutex);
		fprintf(stderr, "could not create vlc_source\n");
		fail();
	}
	obs_source_add_audio_capture_callback(src, delivered_cb, &delivered);
	obs_set_output_source(0, src);

	/* wait until the first item has been handed over completely */
	for (int i = 0; i < 1000 && delivered_frames(&delivered) < FILE_FRAMES; i++)
		os_sleep_ms(5);
	bool handed_over = delivered_frames(&delivered) >= FILE_FRAMES;
	/* libVLC hands the audio over about a second ahead; stop well
	 * inside that second */
	os_sleep_ms(200);

	uint64_t t0 = os_gettime_ns();
	obs_source_media_stop(src);
	enum obs_media_state st = obs_source_media_get_state(src);
	for (int i = 0; i < 1000 && st != OBS_MEDIA_STATE_STOPPED; i++) {
		os_sleep_ms(2);
		st = obs_source_media_get_state(src);
	}
	double took = (double)(os_gettime_ns() - t0) / 1e9;

	obs_set_output_source(0, NULL);
	obs_source_remove_audio_capture_callback(src, delivered_cb, &delivered);
	obs_source_release(src);
	os_unlink(path);
	pthread_mutex_destroy(&delivered.mutex);

	fprintf(stderr, "stop at the end of a track took %.3f s\n", took);
	if (!handed_over) {
		fprintf(stderr, "vlc_source never handed over the first track\n");
		fail();
	}
	if (st != OBS_MEDIA_STATE_STOPPED || took > 0.5) {
		fprintf(stderr, "stopping at the end of a track took %.3f s (state %d)\n", took, (int)st);
		fail();
	}
}

struct end_stop {
	obs_source_t *src;
	volatile bool stop_sent;
	volatile long started_after_stop;
};

/* "media_ended", on libVLC's thread that reports the end of a track. libVLC's
 * playlist moves on to the next item from another thread once this report
 * is done. Stop now, and hold the report until that Stop is under way, so the
 * Stop and the move to the next item meet, as a Stop pressed just as a track
 * ends does. */
static void end_stop_ended(void *param, calldata_t *cd)
{
	struct end_stop *es = param;
	UNUSED_PARAMETER(cd);
	if (os_atomic_set_bool(&es->stop_sent, true))
		return;
	obs_source_media_stop(es->src);
	/* the Stop runs on the next video tick (30 fps) */
	os_sleep_ms(150);
}

static void end_stop_started(void *param, calldata_t *cd)
{
	struct end_stop *es = param;
	UNUSED_PARAMETER(cd);
	if (os_atomic_load_bool(&es->stop_sent))
		os_atomic_inc_long(&es->started_after_stop);
}

/* Guard: a Stop that meets the move to the next playlist item stays in
 * force; libVLC must not start an item after it. */
static void stop_as_track_ends_stays_stopped(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	if (!vlc_ready)
		skip();

	char path[512];
	snprintf(path, sizeof(path), "%s/obs-vlc-gap-%llu-endstop.wav", getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp",
		 (unsigned long long)os_gettime_ns());
	if (!write_wav(path, 0)) {
		fprintf(stderr, "could not write %s\n", path);
		fail();
	}

	obs_data_t *settings = obs_data_create();
	obs_data_array_t *list = obs_data_array_create();
	for (int i = 0; i < 2; i++) {
		obs_data_t *item = obs_data_create();
		obs_data_set_string(item, "value", path);
		obs_data_array_push_back(list, item);
		obs_data_release(item);
	}
	obs_data_set_array(settings, "playlist", list);
	obs_data_set_string(settings, "playback_behavior", "always_play");
	obs_data_set_bool(settings, "loop", false);
	obs_data_array_release(list);

	struct end_stop es = {0};
	obs_source_t *src = obs_source_create("vlc_source", "vlc", settings, NULL);
	obs_data_release(settings);
	if (!src) {
		os_unlink(path);
		fprintf(stderr, "could not create vlc_source\n");
		fail();
	}
	es.src = src;
	signal_handler_t *sh = obs_source_get_signal_handler(src);
	signal_handler_connect(sh, "media_ended", end_stop_ended, &es);
	signal_handler_connect(sh, "media_started", end_stop_started, &es);
	obs_set_output_source(0, src);

	/* the first item plays for 3 s */
	for (int i = 0; i < 2000 && !os_atomic_load_bool(&es.stop_sent); i++)
		os_sleep_ms(5);
	bool ended = os_atomic_load_bool(&es.stop_sent);
	/* the Stop has been handled, and the next item would be playing */
	os_sleep_ms(1000);
	enum obs_media_state st = obs_source_media_get_state(src);
	long started = os_atomic_load_long(&es.started_after_stop);

	obs_set_output_source(0, NULL);
	signal_handler_disconnect(sh, "media_ended", end_stop_ended, &es);
	signal_handler_disconnect(sh, "media_started", end_stop_started, &es);
	obs_source_release(src);
	os_unlink(path);

	fprintf(stderr, "stop as a track ends: state %d after 1 s, %ld item(s) started after the stop\n", (int)st,
		started);
	if (!ended) {
		fprintf(stderr, "the first item never ended\n");
		fail();
	}
	if (st != OBS_MEDIA_STATE_STOPPED || started) {
		fprintf(stderr, "a Stop as a track ended did not stay in force\n");
		fail();
	}
}

/* Guard: a single file plays completely. */
static void single_file_plays_to_the_end(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	if (!vlc_ready)
		skip();

	struct capture cap;
	play_playlist(1, &cap);

	if (!check_file(&cap, 0)) {
		fprintf(stderr, "a single file did not play completely\n");
		fail();
	}
}

/* When libVLC takes more than 70 ms to open the next item, libobs leaves the
 * gap silent. The next file then starts at any point of a mix block, also
 * a few frames before its end: there the tone has not yet reached its level,
 * and the block's peak is that of another file or of an earlier point of the
 * file. The analysis must not take such a block for audio of that file. Here
 * file 3 starts 7 frames before the end of a block, after 123 ms of silence:
 * the peak of that block, 0.42, is file 2's level at 1.1 s (CI run
 * 39679055607 reported "file 2: the last 1.886 s are missing"). */
static void partial_block_at_track_start_is_not_misread(void **state)
{
	UNUSED_PARAMETER(state);
	/* 2 * FILE_FRAMES + 5881 frames before file 3: 7 short of a block end */
	const uint32_t gap[3] = {0, 0, 5881};
	const uint32_t keep[3] = {FILE_FRAMES, FILE_FRAMES, FILE_FRAMES};
	struct capture cap;
	feed_mix(&cap, 3, gap, keep);

	bool ok = check_file(&cap, 0) & check_file(&cap, 1) & check_file(&cap, 2);
	if (!ok) {
		fprintf(stderr, "the analysis misread a block at a track start\n");
		fail();
	}
}

/* The same with two files: file 2 starts 7 frames before the end of a block
 * after 115 ms of silence; that block's peak is file 1's level at 2.2 s. */
static void partial_block_at_second_track_start_is_not_misread(void **state)
{
	UNUSED_PARAMETER(state);
	const uint32_t gap[2] = {0, 5497};
	const uint32_t keep[2] = {FILE_FRAMES, FILE_FRAMES};
	struct capture cap;
	feed_mix(&cap, 2, gap, keep);

	bool ok = check_file(&cap, 0) & check_file(&cap, 1);
	if (!ok) {
		fprintf(stderr, "the analysis misread a block at a track start\n");
		fail();
	}
}

/* Guard: the analysis still reports a track whose last second is missing,
 * with and without a silent gap before the next file. */
static void cut_track_end_is_reported(void **state)
{
	UNUSED_PARAMETER(state);
	const uint32_t keep[2] = {FILE_FRAMES - RATE, FILE_FRAMES};
	struct capture cap;

	const uint32_t no_gap[2] = {0, 0};
	feed_mix(&cap, 2, no_gap, keep);
	assert_false(check_file(&cap, 0));
	assert_true(check_file(&cap, 1));

	const uint32_t gap[2] = {0, 5497};
	feed_mix(&cap, 2, gap, keep);
	assert_false(check_file(&cap, 0));
	assert_true(check_file(&cap, 1));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(partial_block_at_track_start_is_not_misread),
		cmocka_unit_test(partial_block_at_second_track_start_is_not_misread),
		cmocka_unit_test(cut_track_end_is_reported),
		cmocka_unit_test(single_file_plays_to_the_end),
		cmocka_unit_test(playlist_plays_every_file_to_the_end),
		cmocka_unit_test(playlist_of_three_plays_every_file_to_the_end),
		cmocka_unit_test(stop_at_track_end_is_prompt),
		cmocka_unit_test(stop_as_track_ends_stays_stopped),
	};

	int failed = cmocka_run_group_tests(tests, start_obs, stop_obs);
	if (failed)
		return failed;
	return vlc_ready ? 0 : 77;
}
