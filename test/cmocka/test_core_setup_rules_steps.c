/* Characterization of the setup rules of libobs/obs.c.h
 * (obs-rust/obs-studio#2878), moved to libobs/obs-core-setup-rules.c before
 * their port to rust/obs-core (core_setup_rules). It pins the CURRENT
 * behaviour; cases marked "characterized, not endorsed" are odd but real.
 *
 * Two parts:
 * - the steps: the hidden functions of obs-core-setup-rules.c, compiled into
 *   this test from the moved file: each reset gate in order, the audio
 *   buffering ticks, the command line's copy, the data path match, the
 *   latest version compare, the capture device check, the monitoring device
 *   checks and the frame-count rule;
 * - the callers: obs_reset_video(), obs_reset_audio2(),
 *   obs_set_cmdline_args(), obs_remove_data_path(),
 *   obs_get_latest_input_type_id(), obs_set_audio_monitoring_device() and
 *   obs_get_video_frame_counts() of the real libobs, through the public
 *   API. These run on libobs with the C backend, the hybrid libobs and
 *   libobs-c-reference.
 *
 * The command line set first with an argc of 0 is pinned on the steps only:
 * through libobs, the later copy leaks the first block, which the leak
 * checker of this sanitized test reports.
 *
 * obs_startup() needs a display on Linux, so the test runs under xvfb-run
 * there. The caller cases skip only when no display is set at all; with one
 * set, a failed obs_startup() fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <obsconfig.h>
#include <util/platform.h>
#include <obs-core-setup-rules.h>

/* obs_add_data_path() and obs_remove_data_path() are deprecated, but still
 * exported: the remove rule is pinned through them like every other
 * export. */
#if defined(_MSC_VER)
#pragma warning(disable : 4996)
#else
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif

/* ================================================================ steps */

static int gate(enum obs_core_setup_rules_video_step step, bool value)
{
	return obs_core_setup_rules_video_gate(step, value);
}

/* Each video gate: no core fails, an active video or a starting output is
 * busy, a bad size is an invalid parameter; anything else goes on. */
static void test_video_gate(void **state)
{
	(void)state;
	assert_int_equal(gate(OBS_CORE_SETUP_RULES_VIDEO_CORE, false), OBS_VIDEO_FAIL);
	assert_int_equal(gate(OBS_CORE_SETUP_RULES_VIDEO_CORE, true), OBS_VIDEO_SUCCESS);
	assert_int_equal(gate(OBS_CORE_SETUP_RULES_VIDEO_ACTIVE, true), OBS_VIDEO_CURRENTLY_ACTIVE);
	assert_int_equal(gate(OBS_CORE_SETUP_RULES_VIDEO_ACTIVE, false), OBS_VIDEO_SUCCESS);
	assert_int_equal(gate(OBS_CORE_SETUP_RULES_VIDEO_STARTING, true), OBS_VIDEO_CURRENTLY_ACTIVE);
	assert_int_equal(gate(OBS_CORE_SETUP_RULES_VIDEO_STARTING, false), OBS_VIDEO_SUCCESS);
	assert_int_equal(gate(OBS_CORE_SETUP_RULES_VIDEO_OUTPUT_SIZE, false), OBS_VIDEO_INVALID_PARAM);
	assert_int_equal(gate(OBS_CORE_SETUP_RULES_VIDEO_OUTPUT_SIZE, true), OBS_VIDEO_SUCCESS);
	assert_int_equal(gate(OBS_CORE_SETUP_RULES_VIDEO_BASE_SIZE, false), OBS_VIDEO_INVALID_PARAM);
	assert_int_equal(gate(OBS_CORE_SETUP_RULES_VIDEO_BASE_SIZE, true), OBS_VIDEO_SUCCESS);
	assert_int_equal(OBS_CORE_SETUP_RULES_VIDEO_GO, OBS_VIDEO_SUCCESS);
}

/* The gates in obs_reset_video()'s order: the first that fails decides. */
static int video_gates(bool core, bool active, bool starting, bool output_ok, bool base_ok)
{
	const struct {
		enum obs_core_setup_rules_video_step step;
		bool value;
	} steps[] = {
		{OBS_CORE_SETUP_RULES_VIDEO_CORE, core},         {OBS_CORE_SETUP_RULES_VIDEO_ACTIVE, active},
		{OBS_CORE_SETUP_RULES_VIDEO_STARTING, starting}, {OBS_CORE_SETUP_RULES_VIDEO_OUTPUT_SIZE, output_ok},
		{OBS_CORE_SETUP_RULES_VIDEO_BASE_SIZE, base_ok},
	};
	for (size_t i = 0; i < sizeof(steps) / sizeof(steps[0]); i++) {
		int r = gate(steps[i].step, steps[i].value);
		if (r != OBS_CORE_SETUP_RULES_VIDEO_GO)
			return r;
	}
	return OBS_VIDEO_SUCCESS;
}

static void test_video_gate_order(void **state)
{
	(void)state;
	assert_int_equal(video_gates(false, true, true, false, false), OBS_VIDEO_FAIL);
	assert_int_equal(video_gates(true, true, false, false, false), OBS_VIDEO_CURRENTLY_ACTIVE);
	assert_int_equal(video_gates(true, false, true, false, false), OBS_VIDEO_CURRENTLY_ACTIVE);
	assert_int_equal(video_gates(true, false, false, false, true), OBS_VIDEO_INVALID_PARAM);
	assert_int_equal(video_gates(true, false, false, true, false), OBS_VIDEO_INVALID_PARAM);
	assert_int_equal(video_gates(true, false, false, true, true), OBS_VIDEO_SUCCESS);
}

static void test_audio_gate(void **state)
{
	(void)state;
	assert_true(obs_core_setup_rules_audio_refused(OBS_CORE_SETUP_RULES_AUDIO_CORE, false));
	assert_false(obs_core_setup_rules_audio_refused(OBS_CORE_SETUP_RULES_AUDIO_CORE, true));
	assert_true(obs_core_setup_rules_audio_refused(OBS_CORE_SETUP_RULES_AUDIO_ACTIVE, true));
	assert_false(obs_core_setup_rules_audio_refused(OBS_CORE_SETUP_RULES_AUDIO_ACTIVE, false));
}

/* 0 ms takes the default of 45 ticks; any other is the frames in that
 * time, rounded up to whole ticks of 1024 frames. */
static void test_buffer_ticks(void **state)
{
	(void)state;
	assert_false(obs_core_setup_rules_buffer_set(0));
	assert_true(obs_core_setup_rules_buffer_set(1));
	assert_true(obs_core_setup_rules_buffer_set(UINT32_MAX));
	assert_int_equal(obs_core_setup_rules_buffer_default_ticks(), 45);
	assert_int_equal(OBS_CORE_SETUP_RULES_AUDIO_OUTPUT_FRAMES, AUDIO_OUTPUT_FRAMES);

	/* 1 ms at 48 kHz is 48 frames: one tick. */
	assert_int_equal(obs_core_setup_rules_buffer_ticks(1, 48000), 1);
	/* Below one frame: none. */
	assert_int_equal(obs_core_setup_rules_buffer_ticks(1, 999), 0);
	assert_int_equal(obs_core_setup_rules_buffer_ticks(1, 1000), 1);
	assert_int_equal(obs_core_setup_rules_buffer_ticks(1000, 48000), 47);
	/* An exact multiple of 1024 frames is not rounded up. */
	assert_int_equal(obs_core_setup_rules_buffer_ticks(1024, 1000), 1);
	assert_int_equal(obs_core_setup_rules_buffer_ticks(1024, 2000), 2);
	assert_int_equal(obs_core_setup_rules_buffer_ticks(1025, 1000), 2);
	assert_int_equal(obs_core_setup_rules_buffer_ticks(21333, 48000), 1000);
	/* The frames themselves are rounded down first. */
	assert_int_equal(obs_core_setup_rules_buffer_ticks(3, 341), 1);
	assert_int_equal(obs_core_setup_rules_buffer_ticks(0, 48000), 0);

	/* Characterized, not endorsed: the product is a uint32_t, so 22370 ms
	 * at 192 kHz (4,295,040,000 frames * 1000) wraps to 72,704 and gives
	 * one tick, not 4195. */
	assert_int_equal(obs_core_setup_rules_buffer_ticks(22369, 192000), 4195);
	assert_int_equal(obs_core_setup_rules_buffer_ticks(22370, 192000), 1);
	assert_int_equal(obs_core_setup_rules_buffer_ticks(UINT32_MAX, UINT32_MAX), 0);
}

static void test_cmdline(void **state)
{
	(void)state;
	assert_false(obs_core_setup_rules_cmdline_set(0));
	assert_true(obs_core_setup_rules_cmdline_set(1));
	assert_true(obs_core_setup_rules_cmdline_set(3));
	/* Characterized, not endorsed: a negative argc counts as set. */
	assert_true(obs_core_setup_rules_cmdline_set(-1));

	assert_int_equal(obs_core_setup_rules_cmdline_len(0, 0), 1);
	assert_int_equal(obs_core_setup_rules_cmdline_len(0, 3), 4);
	assert_int_equal(obs_core_setup_rules_cmdline_len(4, 9), 14);

	/* argc + 1 pointers: the last one is the NULL terminator. */
	assert_int_equal(obs_core_setup_rules_cmdline_table(0, 8), 8);
	assert_int_equal(obs_core_setup_rules_cmdline_table(3, 8), 32);
	assert_int_equal(obs_core_setup_rules_cmdline_table(3, 4), 16);
	assert_int_equal(obs_core_setup_rules_cmdline_table(3, sizeof(char *)), 4 * sizeof(char *));
	/* Characterized, not endorsed: an argc of -1 gives an empty table. */
	assert_int_equal(obs_core_setup_rules_cmdline_table(-1, 8), 0);
}

/* The data file search ends at its first hit. */
static void test_data_file_found(void **state)
{
	(void)state;
	assert_true(obs_core_setup_rules_data_file_found(true));
	assert_false(obs_core_setup_rules_data_file_found(false));
}

static void test_data_path_match(void **state)
{
	(void)state;
	assert_true(obs_core_setup_rules_data_path_match(0));
	assert_false(obs_core_setup_rules_data_path_match(1));
	assert_false(obs_core_setup_rules_data_path_match(-1));
	assert_false(obs_core_setup_rules_data_path_match(INT32_MIN));
}

/* The latest of a list of versions, as obs_get_latest_input_type_id()
 * walks it: the index of the winner, or -1. */
static int latest_of(const uint32_t *versions, size_t num)
{
	int best = OBS_CORE_SETUP_RULES_LATEST_NONE;
	int found = -1;
	for (size_t i = 0; i < num; i++) {
		if (obs_core_setup_rules_latest_newer(versions[i], best)) {
			found = (int)i;
			best = (int)versions[i];
		}
	}
	return found;
}

static void test_latest(void **state)
{
	(void)state;
	assert_int_equal(OBS_CORE_SETUP_RULES_LATEST_NONE, -1);
	assert_true(obs_core_setup_rules_latest_newer(0, -1));
	assert_false(obs_core_setup_rules_latest_newer(0, 0));
	assert_true(obs_core_setup_rules_latest_newer(2, 1));
	assert_false(obs_core_setup_rules_latest_newer(1, 2));

	/* The first of equal versions is kept. */
	const uint32_t equal[] = {0, 1, 1};
	assert_int_equal(latest_of(equal, 3), 1);
	const uint32_t later[] = {0, 2, 1};
	assert_int_equal(latest_of(later, 3), 1);
	assert_int_equal(latest_of(NULL, 0), -1);

	/* Characterized, not endorsed: a version above INT_MAX is negative as
	 * int, so it is never chosen, even alone. */
	assert_false(obs_core_setup_rules_latest_newer(0x80000000u, -1));
	assert_false(obs_core_setup_rules_latest_newer(UINT32_MAX, -1));
	assert_true(obs_core_setup_rules_latest_newer(0x7fffffffu, 0));
	const uint32_t negative[] = {0x80000000u};
	assert_int_equal(latest_of(negative, 1), -1);
	const uint32_t mixed[] = {0x80000000u, 0, 0xffffffffu};
	assert_int_equal(latest_of(mixed, 3), 1);
}

static void test_aoc_check(void **state)
{
	(void)state;
	assert_int_equal(OBS_CORE_SETUP_RULES_DO_NOT_SELF_MONITOR, OBS_SOURCE_DO_NOT_SELF_MONITOR);
	for (uint32_t bit = 0; bit < 32; bit++)
		assert_int_equal(obs_core_setup_rules_aoc_check(1u << bit),
				 (1u << bit) == OBS_SOURCE_DO_NOT_SELF_MONITOR);
	assert_false(obs_core_setup_rules_aoc_check(0));
	assert_true(obs_core_setup_rules_aoc_check(UINT32_MAX));
	assert_false(obs_core_setup_rules_aoc_check(~(uint32_t)OBS_SOURCE_DO_NOT_SELF_MONITOR));
}

static void test_monitor(void **state)
{
	(void)state;
	assert_true(obs_core_setup_rules_monitor_missing(false));
	assert_false(obs_core_setup_rules_monitor_missing(true));
	assert_true(obs_core_setup_rules_monitor_same(0));
	assert_false(obs_core_setup_rules_monitor_same(1));
	assert_false(obs_core_setup_rules_monitor_same(-1));
}

/* The frame-count rule as obs_get_video_frame_counts() asks it, with the
 * reads it made counted. */
static bool counted(bool is_main, bool encoder_only, bool same_view, bool has_video, int *reads)
{
	enum obs_core_setup_rules_mix step = obs_core_setup_rules_mix_step(OBS_CORE_SETUP_RULES_MIX_MAIN, is_main);
	*reads = 1;
	if (step == OBS_CORE_SETUP_RULES_MIX_ENCODER_ONLY) {
		step = obs_core_setup_rules_mix_step(step, encoder_only);
		*reads += 1;
	}
	if (step == OBS_CORE_SETUP_RULES_MIX_VIEW) {
		step = obs_core_setup_rules_mix_step(step, same_view);
		*reads += 1;
	}
	if (step == OBS_CORE_SETUP_RULES_MIX_VIDEO) {
		step = obs_core_setup_rules_mix_step(step, has_video);
		*reads += 1;
	}
	assert_true(step == OBS_CORE_SETUP_RULES_MIX_COUNT || step == OBS_CORE_SETUP_RULES_MIX_SKIP);
	return step == OBS_CORE_SETUP_RULES_MIX_COUNT;
}

static void test_mix_step(void **state)
{
	(void)state;
	int reads;

	/* The main mix counts with a video, whatever else it is. */
	assert_true(counted(true, false, false, true, &reads));
	assert_int_equal(reads, 2);
	assert_true(counted(true, true, false, true, &reads));
	assert_false(counted(true, false, false, false, &reads));
	assert_int_equal(reads, 2);

	/* An encoder-only mix of the main view counts. */
	assert_true(counted(false, true, true, true, &reads));
	assert_int_equal(reads, 4);
	/* Of another view, it does not, and its video is not looked at. */
	assert_false(counted(false, true, false, true, &reads));
	assert_int_equal(reads, 3);
	/* Another mix that is not encoder-only never counts. */
	assert_false(counted(false, false, true, true, &reads));
	assert_int_equal(reads, 2);
	/* Without a video, nothing counts. */
	assert_false(counted(false, true, true, false, &reads));
	assert_int_equal(reads, 4);

	/* The end states stay where they are. */
	assert_int_equal(obs_core_setup_rules_mix_step(OBS_CORE_SETUP_RULES_MIX_COUNT, false),
			 OBS_CORE_SETUP_RULES_MIX_COUNT);
	assert_int_equal(obs_core_setup_rules_mix_step(OBS_CORE_SETUP_RULES_MIX_SKIP, true),
			 OBS_CORE_SETUP_RULES_MIX_SKIP);
}

/* ============================================================== callers */

static bool obs_up;

/* Before obs_startup(): no core. */
static void test_before_startup(void **state)
{
	(void)state;
	struct obs_video_info ovi = {0};
	ovi.base_width = ovi.output_width = 1280;
	ovi.base_height = ovi.output_height = 720;
	ovi.fps_num = 30;
	ovi.fps_den = 1;
	assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_FAIL);

	struct obs_audio_info2 oai = {48000, SPEAKERS_STEREO, 1000, false};
	assert_false(obs_reset_audio2(&oai));

	/* The data paths live outside the core: the first equal path goes. */
	obs_add_data_path("core_setup_rules_a/");
	obs_add_data_path("core_setup_rules_b/");
	obs_add_data_path("core_setup_rules_a/");
	assert_false(obs_remove_data_path("core_setup_rules_c/"));
	assert_true(obs_remove_data_path("core_setup_rules_a/"));
	assert_true(obs_remove_data_path("core_setup_rules_a/"));
	assert_false(obs_remove_data_path("core_setup_rules_a/"));
	assert_true(obs_remove_data_path("core_setup_rules_b/"));
	assert_false(obs_remove_data_path("core_setup_rules_b/"));
}

static int group_setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
			fprintf(stderr, "obs_startup failed with a display set\n");
			return -1;
		}
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

#define SKIP_WITHOUT_OBS()      \
	do {                    \
		if (!obs_up)    \
			skip(); \
	} while (0)

/* No video is active and no output starting here, so the size gates and
 * the fps gate (still in obs.c) decide, in that order. */
static void test_reset_video_gates(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();
	struct obs_video_info ovi = {0};
	ovi.fps_num = 30;
	ovi.fps_den = 1;

	ovi.base_width = 1280;
	ovi.base_height = 720;
	ovi.output_width = 0;
	ovi.output_height = 720;
	assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_INVALID_PARAM);

	ovi.output_width = 1280;
	ovi.base_width = 0;
	assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_INVALID_PARAM);

	ovi.base_width = 1280;
	ovi.fps_num = 0;
	assert_int_equal(obs_reset_video(&ovi), OBS_VIDEO_INVALID_PARAM);
	assert_false(obs_video_active());
}

static uint32_t buffering_ms(uint32_t rate, uint32_t ms)
{
	struct obs_audio_info2 oai = {rate, SPEAKERS_STEREO, ms, false};
	assert_true(obs_reset_audio2(&oai));
	struct obs_audio_info2 got;
	memset(&got, 0, sizeof(got));
	assert_true(obs_get_audio_info2(&got));
	assert_int_equal(got.samples_per_sec, rate);
	return got.max_buffering_ms;
}

/* The buffering comes back in milliseconds from its ticks:
 * ticks * 1024 * 1000 / rate. */
static void test_reset_audio_ticks(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();
	/* A NULL info frees the audio and is a success. */
	assert_true(obs_reset_audio2(NULL));

	/* 0 ms: the default of 45 ticks. */
	assert_int_equal(buffering_ms(48000, 0), 45 * 1024 * 1000 / 48000);
	/* 1 ms at 48 kHz: one tick. */
	assert_int_equal(buffering_ms(48000, 1), 1024 * 1000 / 48000);
	/* 1000 ms at 48 kHz: 48000 frames, 47 ticks. */
	assert_int_equal(buffering_ms(48000, 1000), 47 * 1024 * 1000 / 48000);
	/* 1000 ms at 44.1 kHz: 44100 frames, 44 ticks. */
	assert_int_equal(buffering_ms(44100, 1000), 44 * 1024 * 1000 / 44100);
	/* Characterized, not endorsed: the frames wrap in uint32_t, so
	 * 22370 ms at 192 kHz buffers one tick. (22369 ms, 4195 ticks, is
	 * left out: the milliseconds logged from it overflow an int.) */
	assert_int_equal(buffering_ms(192000, 22370), 1024 * 1000 / 192000);

	assert_true(obs_reset_audio2(NULL));
}

static void test_cmdline_once(void **state)
{
	(void)state;
	const char *argv[] = {"obs", "--multi", ""};
	const char *again[] = {"other"};
	obs_set_cmdline_args(3, argv);
	obs_set_cmdline_args(1, again);

	struct obs_cmdline_args args = obs_get_cmdline_args();
	assert_int_equal(args.argc, 3);
	assert_string_equal(args.argv[0], "obs");
	assert_string_equal(args.argv[1], "--multi");
	assert_string_equal(args.argv[2], "");
	assert_null(args.argv[3]);
	/* The strings follow the table of argc + 1 pointers in one block. */
	assert_ptr_equal(args.argv[0], (char *)(args.argv + 4));
	assert_ptr_equal(args.argv[1], args.argv[0] + 4);
	assert_ptr_equal(args.argv[2], args.argv[1] + 8);
}

static const char *get_name(void *type_data)
{
	(void)type_data;
	return "core setup rules";
}

static void register_input(const char *id, uint32_t version)
{
	struct obs_source_info info;
	memset(&info, 0, sizeof(info));
	info.id = id;
	info.type = OBS_SOURCE_TYPE_INPUT;
	info.get_name = get_name;
	info.version = version;
	obs_register_source(&info);
}

static void test_latest_input_type(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();
	assert_null(obs_get_latest_input_type_id(NULL));

	register_input("core_setup_rules_latest", 0);
	register_input("core_setup_rules_latest", 2);
	register_input("core_setup_rules_latest", 1);
	assert_string_equal(obs_get_latest_input_type_id("core_setup_rules_latest"), "core_setup_rules_latest_v2");

	register_input("core_setup_rules_max", 0x7fffffffu);
	register_input("core_setup_rules_max", 0);
	assert_string_equal(obs_get_latest_input_type_id("core_setup_rules_max"), "core_setup_rules_max_v2147483647");

	/* Characterized, not endorsed: a version above INT_MAX is never the
	 * latest, so version 0 wins. */
	register_input("core_setup_rules_negative", 0);
	register_input("core_setup_rules_negative", 0x80000000u);
	assert_string_equal(obs_get_latest_input_type_id("core_setup_rules_negative"), "core_setup_rules_negative");
}

static void expect_device(const char *name, const char *id)
{
	const char *got_name = NULL;
	const char *got_id = NULL;
	obs_get_audio_monitoring_device(&got_name, &got_id);
	assert_string_equal(got_name, name);
	assert_string_equal(got_id, id);
}

static void test_monitoring_device(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();
	/* The device is set up with the audio. */
	struct obs_audio_info2 oai = {48000, SPEAKERS_STEREO, 0, false};
	assert_true(obs_reset_audio2(&oai));
	bool available = obs_audio_monitoring_available();
	expect_device("Default", "default");

	/* NULL or empty names and ids are refused before anything else. */
	assert_false(obs_set_audio_monitoring_device(NULL, "x"));
	assert_false(obs_set_audio_monitoring_device("x", NULL));
	assert_false(obs_set_audio_monitoring_device("", "x"));
	assert_false(obs_set_audio_monitoring_device("x", ""));
	assert_false(obs_set_audio_monitoring_device(NULL, NULL));
	expect_device("Default", "default");

	/* The same id: true, and the name is not taken. */
	assert_int_equal(obs_set_audio_monitoring_device("Renamed", "default"), available);
	expect_device("Default", "default");

	assert_int_equal(obs_set_audio_monitoring_device("Core setup rules", "core_setup_rules_dev"), available);
	if (available) {
		expect_device("Core setup rules", "core_setup_rules_dev");
		assert_true(obs_set_audio_monitoring_device("Default", "default"));
	}
	expect_device("Default", "default");
}

#define DATA_PROBE "core_setup_rules_probe.txt"
#define DATA_DIR_A "core_setup_rules_dir_a/"
#define DATA_DIR_B "core_setup_rules_dir_b/"

static void write_probe(const char *dir)
{
	char path[256];
	snprintf(path, sizeof(path), "%s%s", dir, DATA_PROBE);
	os_mkdir(dir);
	assert_true(os_quick_write_utf8_file(path, "x", 1, false));
}

static void remove_probe(const char *dir)
{
	char path[256];
	snprintf(path, sizeof(path), "%s%s", dir, DATA_PROBE);
	os_unlink(path);
	os_rmdir(dir);
}

static void expect_found(const char *file, const char *want)
{
	char *found = obs_find_data_file(file);
	if (!want) {
		assert_null(found);
		return;
	}
	assert_non_null(found);
	assert_string_equal(found, want);
	bfree(found);
}

/* obs_find_data_file(): the libobs data directory first, then each added
 * path in the order it was added; the first hit wins. */
static void test_find_data_file_order(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();

	char *missing = obs_find_data_file(DATA_PROBE);
	if (missing) {
		/* characterized, not endorsed: on macOS find_libobs_data_file
		 * returns the libobs framework's Resources path without checking
		 * that the file exists, so the added paths are never searched
		 * (as test_obs_core_info_characterization pins). */
		bfree(missing);
		skip();
	}

	write_probe(DATA_DIR_A);
	write_probe(DATA_DIR_B);

	/* Not found anywhere yet. */
	expect_found(DATA_PROBE, NULL);

	/* The first added path that has the file wins. */
	obs_add_data_path(DATA_DIR_A);
	obs_add_data_path(DATA_DIR_B);
	expect_found(DATA_PROBE, DATA_DIR_A DATA_PROBE);

	/* A path without the file is passed over. */
	remove_probe(DATA_DIR_A);
	expect_found(DATA_PROBE, DATA_DIR_B DATA_PROBE);
	write_probe(DATA_DIR_A);

	/* Without the first path, the second. */
	assert_true(obs_remove_data_path(DATA_DIR_A));
	expect_found(DATA_PROBE, DATA_DIR_B DATA_PROBE);

	/* Added again, it now comes after the second. */
	obs_add_data_path(DATA_DIR_A);
	expect_found(DATA_PROBE, DATA_DIR_B DATA_PROBE);

#if !defined(_WIN32) && !defined(__APPLE__)
	/* The libobs data directory comes before every added path. On Linux and
	 * the BSDs find_libobs_data_file() looks in OBS_DATA_PATH "/libobs/"
	 * first, relative to the working directory in a build tree: a probe put
	 * there and in an added path is found in the libobs directory. */
	if (OBS_DATA_PATH[0] != '/') {
		assert_int_not_equal(os_mkdirs(OBS_DATA_PATH "/libobs"), MKDIR_ERROR);
		write_probe(OBS_DATA_PATH "/libobs/");
		expect_found(DATA_PROBE, OBS_DATA_PATH "/libobs/" DATA_PROBE);
		char path[256];
		snprintf(path, sizeof(path), "%s%s", OBS_DATA_PATH "/libobs/", DATA_PROBE);
		os_unlink(path);
		expect_found(DATA_PROBE, DATA_DIR_B DATA_PROBE);
	}
#endif

	assert_true(obs_remove_data_path(DATA_DIR_B));
	assert_true(obs_remove_data_path(DATA_DIR_A));
	expect_found(DATA_PROBE, NULL);
	remove_probe(DATA_DIR_A);
	remove_probe(DATA_DIR_B);
}

/* Before any video reset there is no main mix: nothing is counted, and
 * either count may be left out. */
static void test_frame_counts_without_mix(void **state)
{
	(void)state;
	SKIP_WITHOUT_OBS();
	uint32_t total = 123;
	uint32_t skipped = 456;
	obs_get_video_frame_counts(&total, &skipped);
	assert_int_equal(total, 0);
	assert_int_equal(skipped, 0);
	total = 7;
	obs_get_video_frame_counts(&total, NULL);
	assert_int_equal(total, 0);
	skipped = 7;
	obs_get_video_frame_counts(NULL, &skipped);
	assert_int_equal(skipped, 0);
	obs_get_video_frame_counts(NULL, NULL);
}

#define CASE(fn) cmocka_unit_test(fn)

int main(void)
{
	const struct CMUnitTest steps[] = {
		CASE(test_video_gate),      CASE(test_video_gate_order), CASE(test_audio_gate),
		CASE(test_buffer_ticks),    CASE(test_cmdline),          CASE(test_data_file_found),
		CASE(test_data_path_match), CASE(test_latest),           CASE(test_aoc_check),
		CASE(test_monitor),         CASE(test_mix_step),         CASE(test_before_startup),
	};
	const struct CMUnitTest callers[] = {
		CASE(test_reset_video_gates),    CASE(test_reset_audio_ticks), CASE(test_cmdline_once),
		CASE(test_latest_input_type),    CASE(test_monitoring_device), CASE(test_frame_counts_without_mix),
		CASE(test_find_data_file_order),
	};
	int failed = cmocka_run_group_tests_name("steps", steps, NULL, NULL);
	failed += cmocka_run_group_tests_name("callers", callers, group_setup, group_teardown);
	return failed;
}
