/* linux-v4l2: the Virtual Camera also sends OBS's audio mix to an
 * "OBS Virtual Microphone" (obs-rust/obs-studio#3011). The audio goes into the
 * FIFO of a PulseAudio/PipeWire module-pipe-source loaded with pactl.
 *
 * The real v4l2-output.c is compiled in. A real non-blocking pipe stands in
 * for the module's FIFO, so no sound server is needed: a packet that does not
 * fit whole is dropped (never split, never blocked on), so the reader always
 * sees whole frames, and pactl's "load-module" reply is parsed for the
 * module index that "unload-module" needs. */

/* v4l2-output.c needs it (versionsort) before the first system header. */
#define _GNU_SOURCE

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>

#include "plugins/linux-v4l2/v4l2-output.c.h"

static int fds[2];
static size_t pipe_size;

static int setup_pipe(void **state)
{
	(void)state;
	assert_int_equal(pipe2(fds, O_NONBLOCK | O_CLOEXEC), 0);
	int sz = fcntl(fds[1], F_SETPIPE_SZ, 4096);
	assert_true(sz >= 4096);
	pipe_size = (size_t)sz;
	return 0;
}

static int teardown_pipe(void **state)
{
	(void)state;
	close(fds[0]);
	close(fds[1]);
	return 0;
}

static void packet_that_fits_is_written_whole(void **state)
{
	(void)state;
	uint8_t packet[1024];
	for (size_t i = 0; i < sizeof(packet); i++)
		packet[i] = (uint8_t)i;

	assert_true(vmic_write_packet(fds[1], pipe_size, packet, sizeof(packet)));

	uint8_t got[2048];
	ssize_t n = read(fds[0], got, sizeof(got));
	assert_int_equal(n, (ssize_t)sizeof(packet));
	assert_memory_equal(got, packet, sizeof(packet));
}

static void packet_that_does_not_fit_is_dropped_whole(void **state)
{
	(void)state;
	uint8_t packet[1000];
	memset(packet, 0xAB, sizeof(packet));

	size_t written = 0;
	size_t accepted = 0;
	for (int i = 0; i < 16; i++) {
		if (vmic_write_packet(fds[1], pipe_size, packet, sizeof(packet))) {
			accepted++;
			written += sizeof(packet);
		}
	}
	/* The pipe holds only whole packets: nothing was split, and the
	 * writer stopped once the next packet no longer fit. */
	assert_true(accepted >= 1);
	assert_true(accepted < 16);
	assert_true(written <= pipe_size);

	int queued = 0;
	assert_int_equal(ioctl(fds[0], FIONREAD, &queued), 0);
	assert_int_equal((size_t)queued, written);
	assert_int_equal((size_t)queued % sizeof(packet), 0);

	/* Once the reader drains it, packets go through again. */
	uint8_t drain[8192];
	while (read(fds[0], drain, sizeof(drain)) > 0)
		;
	assert_true(vmic_write_packet(fds[1], pipe_size, packet, sizeof(packet)));
}

static void closed_fifo_is_not_written(void **state)
{
	(void)state;
	uint8_t packet[16] = {0};
	assert_false(vmic_write_packet(-1, pipe_size, packet, sizeof(packet)));
}

static void module_index_is_parsed_from_pactl_reply(void **state)
{
	(void)state;
	uint32_t id = 0;
	assert_true(vmic_parse_module_id("536870913\n", &id));
	assert_int_equal(id, 536870913u);
	assert_true(vmic_parse_module_id("27", &id));
	assert_int_equal(id, 27u);

	assert_false(vmic_parse_module_id("", &id));
	assert_false(vmic_parse_module_id(NULL, &id));
	assert_false(vmic_parse_module_id("Failure: Module initialization failed\n", &id));
	assert_false(vmic_parse_module_id("12abc\n", &id));
	assert_false(vmic_parse_module_id("99999999999999\n", &id));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(packet_that_fits_is_written_whole, setup_pipe, teardown_pipe),
		cmocka_unit_test_setup_teardown(packet_that_does_not_fit_is_dropped_whole, setup_pipe, teardown_pipe),
		cmocka_unit_test(closed_fifo_is_not_written),
		cmocka_unit_test(module_index_is_parsed_from_pactl_reply),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
