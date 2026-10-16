/* Regression test for upstream obsproject/obs-studio#10750 (reported by
 * zenorbi): two recordings started in the same second wrote to the same
 * file, and one recording was lost.
 *
 * The frontend picks a recording's file name in GetOutputFilename() and
 * FindBestFilename() (frontend/OBSApp.cpp). Unless "Overwrite if file
 * exists" is on, they only check that the name is not taken yet; nothing
 * creates the file. The ffmpeg_muxer output then opened the path to check
 * that it was writable and deleted it again (ffmpeg_mux_start_internal() in
 * plugins/obs-ffmpeg/obs-ffmpeg-mux.c), and the obs-ffmpeg-mux helper
 * created the file only once the encoders had sent their headers, often
 * hundreds of milliseconds later. Any other output that chose a name in
 * that window, from a second OBS (the report: two instances started 640 ms
 * apart) or a second output in the same OBS, saw the same name as free.
 * Both helpers then wrote to one file.
 *
 * output_filename_reserve() is the Qt-free name selection that
 * GetOutputFilename() calls. The race cases let 8 threads, and 4 processes,
 * choose a name for the same path, and only then let each "muxer" open its
 * path ("wb", as obs-ffmpeg-mux does) and write its own data. Every output
 * must get its own file and every file must survive.
 *
 * The last case drives the real ffmpeg_muxer output (obs-ffmpeg-mux.c is
 * built into the test) and checks that starting it keeps a reserved file.
 * It needs core video, so it runs on Linux under xvfb-run and skips without
 * a display.
 *
 * The stream dump cases are the follow-up: with Enhanced Broadcasting and
 * "Enable stream dump to FLV" on, GenerateMultitrackVideoStreamDumpConfig()
 * (frontend/utility/BasicOutputHandler.cpp) reserves the dump's file when
 * the go-live starts, but the dump output opens it only once the stream
 * output has started. When the go-live fails before that (the config
 * cannot be fetched or parsed, or the stream output does not start), the
 * frontend kept no owner for the file, and every failed attempt left an
 * empty .flv in the recording folder. output_filename_reservation_*() is
 * that owner; the frontend holds the dump's reservation in it, hands it
 * over when the dump output starts and cancels it on every failure.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>

#include <obs.h>
#include <obs-module.h>
#include <util/bmem.h>
#include <util/dstr.h>
#include <util/platform.h>
#include <util/threading.h>

#ifdef _WIN32
#include <windows.h>
#else
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>
#endif

#include "output-filename-reserve.h"

/* The file name both OBS instances chose in the report. */
#define ISSUE_NAME "2024-05-29 15-41-54"

#define THREADS 8
#define PROCESSES 4

/* The current test's directory, removed by cleanup_dir() if the test fails
 * before removing it. */
static char *test_dir;

static char *create_temp_dir(void);

static char *make_temp_dir(void)
{
	test_dir = create_temp_dir();
	return test_dir;
}

static char *create_temp_dir(void)
{
#ifdef _WIN32
	char base[MAX_PATH];
	GetTempPathA(MAX_PATH, base);
	struct dstr dir = {0};
	dstr_printf(&dir, "%sobs-test-reserve-%lu-%llu", base, (unsigned long)GetCurrentProcessId(),
		    (unsigned long long)os_gettime_ns());
	dstr_replace(&dir, "\\", "/");
	if (os_mkdirs(dir.array) == MKDIR_ERROR) {
		fprintf(stderr, "could not create %s\n", dir.array);
		dstr_free(&dir);
		fail();
	}
	return dir.array;
#else
	const char *tmp = getenv("TMPDIR");
	struct dstr dir = {0};
	dstr_printf(&dir, "%s/obs-test-reserve-XXXXXX", tmp && *tmp ? tmp : "/tmp");
	if (!mkdtemp(dir.array)) {
		fprintf(stderr, "mkdtemp failed for %s\n", dir.array);
		dstr_free(&dir);
		fail();
	}
	return dir.array;
#endif
}

static void remove_dir(char *dir)
{
	os_dir_t *d = os_opendir(dir);
	struct os_dirent *ent;
	while (d && (ent = os_readdir(d)) != NULL) {
		if (ent->directory)
			continue;
		struct dstr file = {0};
		dstr_printf(&file, "%s/%s", dir, ent->d_name);
		os_unlink(file.array);
		dstr_free(&file);
	}
	if (d)
		os_closedir(d);
	os_rmdir(dir);
	if (dir == test_dir)
		test_dir = NULL;
	bfree(dir);
}

static int cleanup_dir(void **state)
{
	(void)state;
	if (test_dir)
		remove_dir(test_dir);
	return 0;
}

static char *join(const char *dir, const char *name)
{
	struct dstr path = {0};
	dstr_printf(&path, "%s/%s", dir, name);
	return path.array;
}

/* What obs-ffmpeg-mux does once it has its headers: open the path "wb". */
static bool write_file(const char *path, const char *data)
{
	FILE *f = os_fopen(path, "wb");
	if (!f)
		return false;
	bool ok = fwrite(data, 1, strlen(data), f) == strlen(data);
	return fclose(f) == 0 && ok;
}

static char *read_file(const char *path)
{
	return os_quick_read_utf8_file(path);
}

static bool file_exists_with_size(const char *path, int64_t size)
{
	return os_get_file_size(path) == size;
}

static void expect_str(const char *what, const char *got, const char *expected)
{
	if (!got || strcmp(got, expected) != 0) {
		fprintf(stderr, "%s\n  got:      \"%s\"\n  expected: \"%s\"\n", what, got ? got : "(null)", expected);
		fail();
	}
}

/* ------------------------------------------------------------------------- */
/* N outputs choosing a file in the same second */

/* Every output has chosen its name before any of them opens its file: the
 * muxer creates the file only after the encoders have started. */
struct race {
	const char *candidate;
	bool no_space;

	/* A barrier (os_event_signal wakes one waiter, so it cannot be used to
	 * release all threads at once). */
	pthread_mutex_t mutex;
	pthread_cond_t cond;
	int arrived;
	int generation;
};

struct racer {
	struct race *race;
	int index;
	char *path;
};

static void barrier_wait(struct race *race)
{
	pthread_mutex_lock(&race->mutex);
	int generation = race->generation;
	if (++race->arrived == THREADS) {
		race->arrived = 0;
		race->generation++;
		pthread_cond_broadcast(&race->cond);
	} else {
		while (generation == race->generation)
			pthread_cond_wait(&race->cond, &race->mutex);
	}
	pthread_mutex_unlock(&race->mutex);
}

static void *racer_thread(void *param)
{
	struct racer *r = param;
	struct race *race = r->race;

	/* all outputs start in the same second */
	barrier_wait(race);
	r->path = output_filename_reserve(race->candidate, race->no_space, false, NULL);

	/* the muxers open their files once every output has a name */
	barrier_wait(race);
	if (r->path) {
		char data[32];
		snprintf(data, sizeof(data), "output %d", r->index);
		write_file(r->path, data);
	}
	return NULL;
}

/* Checks that n outputs got n different files and that each file holds what
 * its own output wrote. */
static void check_outputs(const char *what, char **paths, int n)
{
	int distinct = 0;
	int survived = 0;

	for (int i = 0; i < n; i++) {
		bool first = paths[i] != NULL;
		for (int j = 0; j < i && first; j++)
			first = !paths[j] || strcmp(paths[i], paths[j]) != 0;
		if (first)
			distinct++;

		char expected[32];
		snprintf(expected, sizeof(expected), "output %d", i);
		char *data = paths[i] ? read_file(paths[i]) : NULL;
		if (data && strcmp(data, expected) == 0)
			survived++;
		bfree(data);
	}

	if (distinct != n || survived != n) {
		fprintf(stderr,
			"%s: %d outputs started in the same second got %d distinct files; %d of %d recordings survived\n",
			what, n, distinct, survived, n);
		for (int i = 0; i < n; i++)
			fprintf(stderr, "  output %d -> %s\n", i, paths[i] ? paths[i] : "(null)");
		fail();
	}
}

static void run_thread_race(bool no_space)
{
	char *dir = make_temp_dir();
	char *candidate = join(dir, ISSUE_NAME ".mkv");

	struct race race = {.candidate = candidate, .no_space = no_space};
	struct racer racers[THREADS];
	pthread_t threads[THREADS];
	char *paths[THREADS];

	pthread_mutex_init(&race.mutex, NULL);
	pthread_cond_init(&race.cond, NULL);

	for (int i = 0; i < THREADS; i++) {
		racers[i] = (struct racer){.race = &race, .index = i};
		pthread_create(&threads[i], NULL, racer_thread, &racers[i]);
	}
	for (int i = 0; i < THREADS; i++) {
		pthread_join(threads[i], NULL);
		paths[i] = racers[i].path;
	}

	pthread_cond_destroy(&race.cond);
	pthread_mutex_destroy(&race.mutex);

	check_outputs(no_space ? "threads, no spaces" : "threads", paths, THREADS);

	for (int i = 0; i < THREADS; i++)
		bfree(paths[i]);
	bfree(candidate);
	remove_dir(dir);
}

/* A recording and a second output in one OBS, e.g. a second canvas. */
static void test_same_second_threads_get_distinct_files(void **state)
{
	(void)state;
	run_thread_race(false);
}

static void test_same_second_threads_get_distinct_files_no_space(void **state)
{
	(void)state;
	run_thread_race(true);
}

/* Two OBS instances, as in the report. */
static void test_same_second_processes_get_distinct_files(void **state)
{
	(void)state;
#ifdef _WIN32
	skip();
#else
	char *dir = make_temp_dir();
	char *candidate = join(dir, ISSUE_NAME ".mkv");
	int go[2], names[2], write_go[2];
	pid_t pids[PROCESSES];

	if (pipe(go) != 0 || pipe(names) != 0 || pipe(write_go) != 0) {
		fprintf(stderr, "pipe failed\n");
		fail();
	}

	fflush(stderr);
	for (int i = 0; i < PROCESSES; i++) {
		pids[i] = fork();
		if (pids[i] < 0) {
			fprintf(stderr, "fork failed\n");
			fail();
		}
		if (pids[i] == 0) {
			char c;
			close(go[1]);
			close(names[0]);
			close(write_go[1]);
			/* all children wake when the parent closes go[1] */
			if (read(go[0], &c, 1) != 0)
				_exit(2);
			char *path = output_filename_reserve(candidate, false, false, NULL);
			char line[1024] = {0};
			snprintf(line, sizeof(line), "%d %s", i, path ? path : "");
			if (write(names[1], line, sizeof(line)) != (ssize_t)sizeof(line))
				_exit(3);
			if (read(write_go[0], &c, 1) != 0)
				_exit(4);
			char data[32];
			snprintf(data, sizeof(data), "output %d", i);
			_exit(path && write_file(path, data) ? 0 : 5);
		}
	}

	close(go[0]);
	close(names[1]);
	close(write_go[0]);
	close(go[1]);

	char *paths[PROCESSES] = {0};
	for (int n = 0; n < PROCESSES; n++) {
		char line[1024];
		if (read(names[0], line, sizeof(line)) != (ssize_t)sizeof(line)) {
			fprintf(stderr, "a child did not report its file name\n");
			fail();
		}
		int i = atoi(line);
		const char *path = strchr(line, ' ');
		if (i >= 0 && i < PROCESSES && path && path[1])
			paths[i] = bstrdup(path + 1);
	}
	close(write_go[1]);
	close(names[0]);

	for (int i = 0; i < PROCESSES; i++) {
		int status = 0;
		waitpid(pids[i], &status, 0);
		if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
			fprintf(stderr, "child %d failed (status %d)\n", i, status);
			fail();
		}
	}

	check_outputs("processes", paths, PROCESSES);

	for (int i = 0; i < PROCESSES; i++)
		bfree(paths[i]);
	bfree(candidate);
	remove_dir(dir);
#endif
}

/* ------------------------------------------------------------------------- */
/* the single-recording behaviour stays the same */

static void test_single_recording_keeps_its_name(void **state)
{
	(void)state;
	char *dir = make_temp_dir();
	char *candidate = join(dir, ISSUE_NAME ".mkv");

	char *path = output_filename_reserve(candidate, false, false, NULL);
	expect_str("first recording", path, candidate);
	write_file(path, "first");
	bfree(path);

	/* a later recording with the same name gets " (2)", then " (3)" */
	char *second = join(dir, ISSUE_NAME " (2).mkv");
	char *third = join(dir, ISSUE_NAME " (3).mkv");
	path = output_filename_reserve(candidate, false, false, NULL);
	expect_str("second recording", path, second);
	write_file(path, "second");
	bfree(path);
	path = output_filename_reserve(candidate, false, false, NULL);
	expect_str("third recording", path, third);
	write_file(path, "third");
	bfree(path);
	bfree(second);
	bfree(third);

	/* "File name without space" gives "_2" */
	char *nospace = join(dir, ISSUE_NAME "_2.mkv");
	path = output_filename_reserve(candidate, true, false, NULL);
	expect_str("recording without spaces", path, nospace);
	bfree(path);
	bfree(nospace);

	char *data = read_file(candidate);
	expect_str("the first recording", data, "first");
	bfree(data);

	bfree(candidate);
	remove_dir(dir);
}

/* "Overwrite if file exists": the same name, nothing created or truncated. */
static void test_overwrite_keeps_name_and_file(void **state)
{
	(void)state;
	char *dir = make_temp_dir();
	char *candidate = join(dir, ISSUE_NAME ".mkv");
	char *fresh = join(dir, "fresh.mkv");
	bool reserved = true;

	write_file(candidate, "old recording");
	char *path = output_filename_reserve(candidate, false, true, &reserved);
	expect_str("overwrite, existing file", path, candidate);
	if (reserved) {
		fprintf(stderr, "overwrite must not reserve a file\n");
		fail();
	}
	bfree(path);
	char *data = read_file(candidate);
	expect_str("the existing file", data, "old recording");
	bfree(data);

	path = output_filename_reserve(fresh, false, true, &reserved);
	expect_str("overwrite, new file", path, fresh);
	if (reserved || os_file_exists(fresh)) {
		fprintf(stderr, "overwrite must not create %s\n", fresh);
		fail();
	}
	bfree(path);

	bfree(fresh);
	bfree(candidate);
	remove_dir(dir);
}

/* A path the output cannot write keeps its name, so the output reports the
 * error as before. */
static void test_unwritable_path_keeps_its_name(void **state)
{
	(void)state;
	char *dir = make_temp_dir();
	char *candidate = join(dir, "missing/" ISSUE_NAME ".mkv");
	bool reserved = true;

	char *path = output_filename_reserve(candidate, false, false, &reserved);
	expect_str("missing directory", path, candidate);
	if (reserved) {
		fprintf(stderr, "a path in a missing directory cannot be reserved\n");
		fail();
	}
	bfree(path);

	bfree(candidate);
	remove_dir(dir);
}

/* An output that fails to start leaves no empty file; one that wrote data
 * keeps it. */
static void test_release_removes_only_unwritten_reservations(void **state)
{
	(void)state;
	char *dir = make_temp_dir();
	char *candidate = join(dir, ISSUE_NAME ".mkv");

	char *path = output_filename_reserve(candidate, false, false, NULL);
	output_filename_release(path);
	if (os_file_exists(path)) {
		fprintf(stderr, "release left the empty reservation %s\n", path);
		fail();
	}
	bfree(path);

	path = output_filename_reserve(candidate, false, false, NULL);
	write_file(path, "recorded");
	output_filename_release(path);
	if (!file_exists_with_size(path, 8)) {
		fprintf(stderr, "release removed the written file %s\n", path);
		fail();
	}
	bfree(path);

	output_filename_release(NULL);

	bfree(candidate);
	remove_dir(dir);
}

/* ------------------------------------------------------------------------- */
/* the stream dump of a failed go-live leaves no empty file */

/* A file name the stream dump's default format gives. */
#define DUMP_NAME "2026-10-16 06-07-13"

static int count_files(const char *dir)
{
	int n = 0;
	os_dir_t *d = os_opendir(dir);
	struct os_dirent *ent;
	while (d && (ent = os_readdir(d)) != NULL) {
		if (!ent->directory)
			n++;
	}
	if (d)
		os_closedir(d);
	return n;
}

/* What GenerateMultitrackVideoStreamDumpConfig() does at each go-live: the
 * name is reserved and handed to the reservation. */
static char *reserve_dump(struct output_filename_reservation *reservation, const char *candidate)
{
	bool reserved = false;
	char *path = output_filename_reserve(candidate, false, false, &reserved);
	if (!reserved) {
		fprintf(stderr, "could not reserve %s\n", candidate);
		fail();
	}
	output_filename_reservation_hold(reservation, path);
	return path;
}

/* The go-live fails before the stream output starts (config download or
 * parse error): the dump's file goes away. */
static void test_failed_go_live_removes_dump_file(void **state)
{
	(void)state;
	char *dir = make_temp_dir();
	char *candidate = join(dir, DUMP_NAME ".flv");
	struct output_filename_reservation reservation = {0};

	char *path = reserve_dump(&reservation, candidate);
	output_filename_reservation_cancel(&reservation);
	if (os_file_exists(path)) {
		fprintf(stderr, "a failed go-live left the empty stream dump %s\n", path);
		fail();
	}
	if (reservation.path) {
		fprintf(stderr, "the reservation still holds %s after cancel\n", reservation.path);
		fail();
	}
	bfree(path);

	bfree(candidate);
	remove_dir(dir);
}

/* Three failed attempts, as a user retrying Start Streaming does: nothing
 * is left in the recording folder. */
static void test_repeated_failed_go_lives_leave_no_files(void **state)
{
	(void)state;
	char *dir = make_temp_dir();
	char *candidate = join(dir, DUMP_NAME ".flv");
	struct output_filename_reservation reservation = {0};

	for (int i = 0; i < 3; i++) {
		char *path = reserve_dump(&reservation, candidate);
		output_filename_reservation_cancel(&reservation);
		bfree(path);
	}
	int left = count_files(dir);
	if (left != 0) {
		fprintf(stderr, "3 failed go-lives left %d empty file(s) in the recording folder\n", left);
		fail();
	}

	bfree(candidate);
	remove_dir(dir);
}

/* A new go-live while an earlier reservation is still held (no failure or
 * start reported for it) does not leak the earlier file. */
static void test_new_go_live_replaces_an_unsettled_reservation(void **state)
{
	(void)state;
	char *dir = make_temp_dir();
	char *candidate = join(dir, DUMP_NAME ".flv");
	struct output_filename_reservation reservation = {0};

	char *first = reserve_dump(&reservation, candidate);
	char *second = reserve_dump(&reservation, candidate);
	if (os_file_exists(first)) {
		fprintf(stderr, "the earlier empty reservation %s was left behind\n", first);
		fail();
	}
	if (!os_file_exists(second)) {
		fprintf(stderr, "the new reservation %s is missing\n", second);
		fail();
	}
	output_filename_reservation_cancel(&reservation);
	bfree(first);
	bfree(second);

	bfree(candidate);
	remove_dir(dir);
}

/* The stream starts and the dump output with it: the file is the dump's
 * now, even while it is still empty, and stays. */
static void test_started_dump_keeps_its_file(void **state)
{
	(void)state;
	char *dir = make_temp_dir();
	char *candidate = join(dir, DUMP_NAME ".flv");
	struct output_filename_reservation reservation = {0};

	char *path = reserve_dump(&reservation, candidate);
	output_filename_reservation_hand_over(&reservation);
	/* the frontend cancels on every later failure; nothing is held now */
	output_filename_reservation_cancel(&reservation);
	if (!os_file_exists(path)) {
		fprintf(stderr, "the started stream dump's file %s was removed\n", path);
		fail();
	}

	/* a file the dump has written to survives a cancel, too */
	char *written = reserve_dump(&reservation, candidate);
	write_file(written, "FLV");
	output_filename_reservation_cancel(&reservation);
	if (!file_exists_with_size(written, 3)) {
		fprintf(stderr, "cancel removed the written stream dump %s\n", written);
		fail();
	}
	bfree(written);
	bfree(path);

	bfree(candidate);
	remove_dir(dir);
}

/* "Overwrite if file exists" reserves nothing: an existing file of that
 * name, even an empty one, is not the reservation's to remove. */
static void test_unreserved_dump_path_is_left_alone(void **state)
{
	(void)state;
	char *dir = make_temp_dir();
	char *candidate = join(dir, DUMP_NAME ".flv");
	struct output_filename_reservation reservation = {0};
	bool reserved = true;

	write_file(candidate, "");
	char *path = output_filename_reserve(candidate, false, true, &reserved);
	output_filename_reservation_hold(&reservation, reserved ? path : NULL);
	output_filename_reservation_cancel(&reservation);
	output_filename_reservation_hold(&reservation, "");
	output_filename_reservation_cancel(&reservation);
	if (!os_file_exists(candidate)) {
		fprintf(stderr, "cancel removed %s, which it did not reserve\n", candidate);
		fail();
	}
	bfree(path);

	bfree(candidate);
	remove_dir(dir);
}

/* ------------------------------------------------------------------------- */
/* the ffmpeg_muxer output keeps a reserved file */

#ifdef TEST_FFMPEG_MUX
extern struct obs_output_info ffmpeg_muxer;

/* obs-ffmpeg-mux.c looks up its UI strings through the module locale. */
const char *obs_module_text(const char *lookup)
{
	return lookup;
}

#define VENC_ID "test_reserve_video_encoder"
#define AENC_ID "test_reserve_audio_encoder"

static uint8_t packet_byte;
static bool obs_up;

static const char *enc_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);
	return "test encoder";
}

static void *enc_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	UNUSED_PARAMETER(settings);
	return encoder;
}

static void enc_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static bool enc_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	UNUSED_PARAMETER(data);
	packet->data = &packet_byte;
	packet->size = 1;
	packet->pts = frame->pts;
	packet->dts = frame->pts;
	packet->keyframe = true;
	*received = true;
	return true;
}

static size_t aenc_frame_size(void *data)
{
	UNUSED_PARAMETER(data);
	return 1024;
}

static int mux_setup(void **state)
{
	(void)state;
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
	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS || !obs_reset_audio(&oai)) {
		fprintf(stderr, "core video/audio failed; skipping the ffmpeg_muxer case\n");
		obs_shutdown();
		return 0;
	}

	struct obs_encoder_info venc = {
		.id = VENC_ID,
		.type = OBS_ENCODER_VIDEO,
		.codec = "h264",
		.get_name = enc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = enc_encode,
	};
	struct obs_encoder_info aenc = {
		.id = AENC_ID,
		.type = OBS_ENCODER_AUDIO,
		.codec = "aac",
		.get_name = enc_name,
		.create = enc_create,
		.destroy = enc_destroy,
		.encode = enc_encode,
		.get_frame_size = aenc_frame_size,
	};
	obs_register_encoder(&venc);
	obs_register_encoder(&aenc);
	obs_register_output(&ffmpeg_muxer);
	obs_up = true;
	return 0;
}

static int mux_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	return 0;
}

static bool wait_inactive(obs_output_t *output)
{
	for (int i = 0; i < 500 && obs_output_active(output); i++)
		os_sleep_ms(10);
	return !obs_output_active(output);
}

/* ffmpeg_mux_start_internal() opened the path to check that it is writable
 * and deleted it again, which threw the reservation away. */
static void test_ffmpeg_muxer_start_keeps_reservation(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	char *dir = make_temp_dir();
	char *reserved_path = join(dir, ISSUE_NAME ".mkv");

	/* the empty file output_filename_reserve() leaves for the output */
	if (!write_file(reserved_path, "")) {
		fprintf(stderr, "could not create %s\n", reserved_path);
		fail();
	}

	obs_encoder_t *venc = obs_video_encoder_create(VENC_ID, "venc", NULL, NULL);
	obs_encoder_t *aenc = obs_audio_encoder_create(AENC_ID, "aenc", NULL, 0, NULL);
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "path", reserved_path);
	obs_output_t *output = obs_output_create("ffmpeg_muxer", "recording", settings, NULL);
	obs_data_release(settings);
	if (!venc || !aenc || !output) {
		fprintf(stderr, "could not create the encoders or the ffmpeg_muxer output\n");
		fail();
	}
	obs_encoder_set_video(venc, obs_get_video());
	obs_encoder_set_audio(aenc, obs_get_audio());
	obs_output_set_video_encoder(output, venc);
	obs_output_set_audio_encoder(output, aenc, 0);

	/* The obs-ffmpeg-mux helper is not next to the test, so the start
	 * fails after the writability check; either way the reservation must
	 * still be there for the helper (or for the frontend to release). */
	bool started = obs_output_start(output);
	bool kept = os_file_exists(reserved_path);
	if (started) {
		obs_output_stop(output);
		wait_inactive(output);
	}

	obs_output_release(output);
	obs_encoder_release(venc);
	obs_encoder_release(aenc);

	if (!kept) {
		fprintf(stderr, "starting ffmpeg_muxer (started: %d) deleted the reserved file %s\n", started,
			reserved_path);
		fail();
	}

	bfree(reserved_path);
	remove_dir(dir);
}
#else
static int mux_setup(void **state)
{
	(void)state;
	return 0;
}

static int mux_teardown(void **state)
{
	(void)state;
	return 0;
}

static void test_ffmpeg_muxer_start_keeps_reservation(void **state)
{
	(void)state;
	skip();
}
#endif

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(test_same_second_threads_get_distinct_files, cleanup_dir),
		cmocka_unit_test_teardown(test_same_second_threads_get_distinct_files_no_space, cleanup_dir),
		cmocka_unit_test_teardown(test_same_second_processes_get_distinct_files, cleanup_dir),
		cmocka_unit_test_teardown(test_single_recording_keeps_its_name, cleanup_dir),
		cmocka_unit_test_teardown(test_overwrite_keeps_name_and_file, cleanup_dir),
		cmocka_unit_test_teardown(test_unwritable_path_keeps_its_name, cleanup_dir),
		cmocka_unit_test_teardown(test_release_removes_only_unwritten_reservations, cleanup_dir),
		cmocka_unit_test_teardown(test_failed_go_live_removes_dump_file, cleanup_dir),
		cmocka_unit_test_teardown(test_repeated_failed_go_lives_leave_no_files, cleanup_dir),
		cmocka_unit_test_teardown(test_new_go_live_replaces_an_unsettled_reservation, cleanup_dir),
		cmocka_unit_test_teardown(test_started_dump_keeps_its_file, cleanup_dir),
		cmocka_unit_test_teardown(test_unreserved_dump_path_is_left_alone, cleanup_dir),
	};
	const struct CMUnitTest mux_tests[] = {
		cmocka_unit_test_teardown(test_ffmpeg_muxer_start_keeps_reservation, cleanup_dir),
	};

	int failed = cmocka_run_group_tests_name("output_filename_reserve", tests, NULL, NULL);
	failed += cmocka_run_group_tests_name("ffmpeg_muxer", mux_tests, mux_setup, mux_teardown);
	return failed;
}
