/* obs-rust/obs-studio#2672: an update host that cannot be reached must not
 * hold shutdown. file-updater's do_http_request installs no
 * CURLOPT_CONNECTTIMEOUT, so a fetch whose target blackholes the connect
 * blocks on the kernel's TCP connect timeout (tcp_syn_retries=6, ~130 s on
 * Linux), and update_info_destroy's unbounded pthread_join waits it out.
 * rtmp-services starts the service-list update at module load and calls
 * update_info_destroy from obs_module_unload, so quitting while that fetch
 * is in flight stalls the exit by that timeout. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <util/platform.h>

#include <file-updater/file-updater.h>

/* RFC 5737 TEST-NET-1: guaranteed unroutable, so the connect SYN is
 * blackholed and only a client-side connect timeout ends the attempt. */
#define UNREACHABLE_URL "http://192.0.2.1/obs-test-package.json"

/* Well below the kernel's default TCP connect timeout that the unfixed
 * build pays (~130 s here), above the fetch's own bound. */
#define SHUTDOWN_BUDGET_MS 40000ULL

/* single_file_download calls the confirm callback unconditionally, so it
 * has to be a real function; a failed download never reaches it. */
static bool confirm_cb(void *param, struct file_download_data *file)
{
	(void)param;
	(void)file;
	return true;
}

static void unreachable_host_does_not_hold_shutdown(void **state)
{
	(void)state;

	uint64_t start = os_gettime_ns();
	update_info_t *info =
		update_info_create_single_with_done("[test] ", "obs-test", UNREACHABLE_URL, confirm_cb, NULL, NULL);
	assert_non_null(info);

	/* update_info_destroy joins the update thread, so this measures how
	 * long an in-flight fetch to an unreachable host can hold shutdown. */
	update_info_destroy(info);
	uint64_t elapsed_ms = (os_gettime_ns() - start) / 1000000;

	if (elapsed_ms >= SHUTDOWN_BUDGET_MS)
		fail_msg("an unreachable update host held shutdown for %llu ms", (unsigned long long)elapsed_ms);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(unreachable_host_does_not_hold_shutdown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
