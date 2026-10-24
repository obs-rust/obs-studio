#include <string.h>
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <cmocka.h>
#include <time.h>
#include <netdb.h>

#include "obs-ffmpeg-net-host.h"

/*
 * obs-ffmpeg's POSIX network host (obs-rust/obs-studio#853), found through
 * the plugins/obs-ffmpeg/hosts/posix include directory: the SRT output's
 * connection timestamp is wall-clock seconds, address errors read as
 * gai_strerror does, and the system syslog.h stays out of libsrt's way.
 * The Windows header (ftime, gai_strerrorA, libsrt's syslog codes) is
 * compiled by the cross check.
 */

static void test_wall_time_is_wall_clock(void **state)
{
	(void)state;
	double before = (double)time(NULL);
	double now = obs_ffmpeg_host_wall_time();
	double after = (double)time(NULL) + 1.0;

	assert_true(now >= before && now <= after);
}

static void test_gai_strerror(void **state)
{
	(void)state;
	assert_string_equal(obs_ffmpeg_host_gai_strerror(EAI_NONAME), gai_strerror(EAI_NONAME));
}

static void test_system_syslog_suppressed(void **state)
{
	(void)state;
#ifdef _SYS_SYSLOG_H
	assert_true(true);
#else
	fail_msg("_SYS_SYSLOG_H is not defined");
#endif
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_wall_time_is_wall_clock),
		cmocka_unit_test(test_gai_strerror),
		cmocka_unit_test(test_system_syslog_suppressed),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
