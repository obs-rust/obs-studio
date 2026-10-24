/* obs-ffmpeg-mux, the helper process OBS records through
 * (plugins/obs-ffmpeg/ffmpeg-mux/ffmpeg-mux.c.h). Split from #1464:
 *
 * - The AVIO seek callback returned 0 for every seek, though FFmpeg expects
 *   the new position (and an error for AVSEEK_SIZE and other unsupported
 *   requests).
 * - free_avformat() dereferenced output->pb when the recording file could
 *   not be opened, so pb was never allocated.
 * - main() returned 0 even when a packet could not be read or written, so OBS
 *   could not tell a failed recording from a clean one.
 * - read_change_file() (split recording) left argv[1] pointing at its own
 *   filename buffer when opening the next file failed; main() then freed
 *   that buffer and, on Windows, argv[1] again.
 *
 * - open_output_file() ignored its writer's mutex, event and thread results
 *   (obs-rust/obs-studio#2164): with an event that could not be created,
 *   the writer thread waited on or signalled a NULL event.
 *
 * The file is included with main() renamed so the static functions can be
 * called directly, and with os_event_init() wrapped so a case can fail it.
 * stdin is redirected to a temporary file for the cases that read packets.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "plugins/obs-ffmpeg/ffmpeg-mux/build/ffmpeg-mux.pch.h"

/* The Nth os_event_init() call fails (0: none). */
static int event_init_fail_at;
static int event_init_calls;

static int test_os_event_init(os_event_t **event, enum os_event_type type)
{
	if (event_init_fail_at && ++event_init_calls == event_init_fail_at)
		return EAGAIN;
	return os_event_init(event, type);
}

int ffmpeg_mux_main(int argc, char *argv[]);
#define main ffmpeg_mux_main
#define os_event_init test_os_event_init
#include "plugins/obs-ffmpeg/ffmpeg-mux/ffmpeg-mux.c.h"
#undef os_event_init
#undef main

static void temp_path(char *buf, size_t size, const char *name)
{
	const char *tmp = getenv("TMPDIR");
	snprintf(buf, size, "%s/obs-ffmpeg-mux-helper-%ld-%s", tmp && *tmp ? tmp : "/tmp", (long)getpid(), name);
}

/* Points stdin at a temporary file holding `size` bytes of `data`. */
static void stdin_from(const void *data, size_t size)
{
	char path[512];
	temp_path(path, sizeof(path), "stdin");
	FILE *f = fopen(path, "wb");
	assert_non_null(f);
	assert_int_equal(fwrite(data, 1, size, f), size);
	fclose(f);
	assert_non_null(freopen(path, "rb", stdin));
	unlink(path);
}

/* ------------------------------------------------------------------------- */

static void test_seek_reports_position(void **state)
{
	(void)state;
	struct ffmpeg_mux ffm = {0};
	pthread_mutex_init(&ffm.io.data_mutex, NULL);
	ffm.io.next_pos = 10;

	assert_int_equal(ffmpeg_mux_seek_av_buffer(&ffm, 100, SEEK_SET), 100);
	assert_int_equal(ffm.io.next_pos, 100);
	assert_int_equal(ffmpeg_mux_seek_av_buffer(&ffm, -40, SEEK_CUR), 60);
	assert_int_equal(ffm.io.next_pos, 60);
	assert_int_equal(ffmpeg_mux_seek_av_buffer(&ffm, 5, SEEK_SET | AVSEEK_FORCE), 5);
	assert_int_equal(ffm.io.next_pos, 5);

	pthread_mutex_destroy(&ffm.io.data_mutex);
}

static void test_seek_rejects_unsupported(void **state)
{
	(void)state;
	struct ffmpeg_mux ffm = {0};
	pthread_mutex_init(&ffm.io.data_mutex, NULL);
	ffm.io.next_pos = 10;

	/* The size of a stream being written is unknown: AVSEEK_SIZE fails. */
	assert_true(ffmpeg_mux_seek_av_buffer(&ffm, 0, AVSEEK_SIZE) < 0);
	assert_true(ffmpeg_mux_seek_av_buffer(&ffm, 0, SEEK_END) < 0);
	/* Before the start of the file. */
	assert_true(ffmpeg_mux_seek_av_buffer(&ffm, -11, SEEK_CUR) < 0);
	assert_int_equal(ffm.io.next_pos, 10);

	pthread_mutex_destroy(&ffm.io.data_mutex);
}

/* The recording file cannot be created: the context is freed before its IO
 * context exists. */
static void test_failed_open_cleanup(void **state)
{
	(void)state;
	struct ffmpeg_mux ffm = {0};
	ffm.params.file = "/nonexistent-obs-rust-dir/out.mkv";
	dstr_copy(&ffm.params.printable_file, ffm.params.file);
	ffm.params.tracks = 1;
	ffm.params.acodec = "pcm_s16le";
	ffm.params.muxer_settings = "";
	ffm.audio = calloc(1, sizeof(*ffm.audio));
	ffm.audio[0] = (struct audio_params){.name = "a",
					     .abitrate = 128,
					     .sample_rate = 48000,
					     .frame_size = 1024,
					     .channels = 2};
	ffm.audio_header = calloc(1, sizeof(*ffm.audio_header));

	assert_int_equal(ffmpeg_mux_init_context(&ffm), FFM_ERROR);
	assert_null(ffm.output);

	ffmpeg_mux_free(&ffm);
}

/* One of the writer's two events cannot be created: the open fails, and
 * the writer thread is not started around a NULL event. */
static void test_writer_event_init_failure(void **state)
{
	(void)state;

	for (int n = 1; n <= 2; n++) {
		char out[512];
		temp_path(out, sizeof(out), "event.mkv");

		struct ffmpeg_mux ffm = {0};
		ffm.params.file = out;
		dstr_copy(&ffm.params.printable_file, ffm.params.file);
		ffm.params.tracks = 1;
		ffm.params.acodec = "pcm_s16le";
		ffm.params.muxer_settings = "";
		ffm.audio = calloc(1, sizeof(*ffm.audio));
		ffm.audio[0] = (struct audio_params){.name = "a",
						     .abitrate = 128,
						     .sample_rate = 48000,
						     .frame_size = 1024,
						     .channels = 2};
		ffm.audio_header = calloc(1, sizeof(*ffm.audio_header));

		event_init_calls = 0;
		event_init_fail_at = n;
		int ret = ffmpeg_mux_init_context(&ffm);
		int calls = event_init_calls;
		event_init_fail_at = 0;

		bool active = ffm.io.active;
		ffmpeg_mux_free(&ffm);
		unlink(out);

		assert_int_equal(ret, FFM_ERROR);
		assert_false(active);
		/* It stops at the first init that fails. */
		assert_int_equal(calls, n);
	}
}

/* A packet whose payload ends early fails the recording, and the exit
 * status says so. */
static void test_main_reports_packet_failure(void **state)
{
	(void)state;
	char out[512];
	temp_path(out, sizeof(out), "out.mkv");

	uint8_t input[sizeof(struct ffm_packet_info) * 2 + 8] = {0};
	struct ffm_packet_info header = {.type = FFM_PACKET_AUDIO, .index = 0, .size = 0};
	struct ffm_packet_info packet = {.type = FFM_PACKET_AUDIO, .index = 0, .size = 64};
	memcpy(input, &header, sizeof(header));
	memcpy(input + sizeof(header), &packet, sizeof(packet));
	stdin_from(input, sizeof(input));

	char *argv[] = {"obs-ffmpeg-mux", out, "0", "1", "pcm_s16le", "a", "128", "48000", "1024", "0", "2", NULL};
	int ret = ffmpeg_mux_main((int)(sizeof(argv) / sizeof(argv[0])) - 1, argv);
	unlink(out);

	assert_int_equal(ret, FFM_ERROR);
}

/* Opening the next split file fails: argv[1] is the caller's again. */
static void test_change_file_failure_restores_argv(void **state)
{
	(void)state;
	static const char next[] = "next.mkv";
	stdin_from(next, sizeof(next) - 1);

	char orig[] = "orig.mkv";
	char *argv[] = {"obs-ffmpeg-mux", orig, NULL};
	struct ffmpeg_mux ffm = {0};
	struct resize_buf filename = {0};

	assert_false(read_change_file(&ffm, (uint32_t)(sizeof(next) - 1), &filename, 2, argv));
	assert_ptr_equal(argv[1], orig);

	resize_buf_free(&filename);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_seek_reports_position),
		cmocka_unit_test(test_seek_rejects_unsupported),
		cmocka_unit_test(test_failed_open_cleanup),
		cmocka_unit_test(test_writer_event_init_failure),
		cmocka_unit_test(test_main_reports_packet_failure),
		cmocka_unit_test(test_change_file_failure_restores_argv),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
