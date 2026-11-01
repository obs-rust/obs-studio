/* write_packet() (plugins/obs-ffmpeg/obs-ffmpeg-mux.c) sends each packet to
 * the obs-ffmpeg-mux helper as an ffm_packet_info header followed by the
 * packet data. The shipped os_process_pipe_write() is unbuffered, so two
 * calls are two write(2) syscalls per packet, small audio packets included
 * (#3211). A packet up to 64 KiB goes out in one call; a larger one keeps two,
 * since copying it costs more than the syscall saved. The helper must receive
 * the same bytes either way.
 *
 * obs-ffmpeg-mux.c is built into the test; os_process_pipe_write() is wrapped
 * to record the calls. No display or OBS core is needed.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

#include <obs-module.h>
#include <util/bmem.h>

#include "obs-ffmpeg-mux.h"
#include "ffmpeg-mux/ffmpeg-mux.h"

const char *obs_module_text(const char *lookup)
{
	return lookup;
}

static size_t write_calls;
static uint8_t written[200000];
static size_t written_len;

size_t __wrap_os_process_pipe_write(os_process_pipe_t *pp, const uint8_t *data, size_t len)
{
	(void)pp;
	write_calls++;
	assert_true(written_len + len <= sizeof(written));
	memcpy(written + written_len, data, len);
	written_len += len;
	return len;
}

static void run(size_t size, size_t expected_calls)
{
	struct ffmpeg_muxer stream;
	memset(&stream, 0, sizeof(stream));
	stream.pipe = (os_process_pipe_t *)&stream;

	uint8_t *data = bmalloc(size);
	for (size_t i = 0; i < size; i++)
		data[i] = (uint8_t)(i * 31 + 7);

	struct encoder_packet packet = {
		.data = data,
		.size = size,
		.pts = 1234,
		.dts = 1200,
		.type = OBS_ENCODER_AUDIO,
		.track_idx = 1,
		.keyframe = true,
	};

	write_calls = 0;
	written_len = 0;
	assert_true(write_packet(&stream, &packet));
	assert_int_equal(write_calls, expected_calls);

	struct ffm_packet_info info = {.pts = 1234,
				       .dts = 1200,
				       .size = (uint32_t)size,
				       .index = 1,
				       .type = FFM_PACKET_AUDIO,
				       .keyframe = true};
	assert_int_equal(written_len, sizeof(info) + size);
	struct ffm_packet_info got;
	memcpy(&got, written, sizeof(got));
	assert_true(got.pts == info.pts && got.dts == info.dts && got.size == info.size && got.index == info.index &&
		    got.type == info.type && got.keyframe == info.keyframe);
	assert_memory_equal(written + sizeof(info), data, size);
	assert_int_equal(stream.total_bytes, size);

	bfree(data);
}

static void small_packet_is_one_write(void **state)
{
	(void)state;
	run(400, 1);
}

static void packet_at_threshold_is_one_write(void **state)
{
	(void)state;
	run(65536, 1);
}

static void large_packet_keeps_two_writes(void **state)
{
	(void)state;
	run(150000, 2);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(small_packet_is_one_write),
		cmocka_unit_test(packet_at_threshold_is_one_write),
		cmocka_unit_test(large_packet_keeps_two_writes),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
