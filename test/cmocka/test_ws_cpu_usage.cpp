/* obs-websocket GetStats reports the CPU usage of OBS since the same client's
 * previous GetStats (obs-rust/obs-studio#1346).
 *
 * All clients shared one os_cpu_usage_info_t, and os_cpu_usage_info_query()
 * moves its "last" times forward on every call. A client got the usage since
 * the last GetStats from any client, not since its own: with two clients
 * polling, each saw only the slice since the other one's call. The query ran
 * without a lock while requests run in parallel on the thread pool, a data
 * race (and two mutable references in the Rust obs-util). Each session now has
 * its own CPU usage state behind a lock; requests without a session use the
 * shared one, also locked.
 *
 * This compiles the plugin's request handlers against real libobs and sends
 * GetStats through RequestHandler::ProcessRequest with two sessions: the test
 * burns CPU, one session polls, the process idles, then the other session
 * polls. The second session's interval covers the burn too, so it must report
 * a comparable usage, not the near zero of the idle slice. The plugin's
 * globals are stubbed. obs_startup() needs a display on Linux; the cases skip
 * without one. */

/* The C++ headers come before cmocka.h: its macros break libstdc++. */
#include <cmath>
#include <cstdio>
#include <thread>
#include <vector>

#include <obs.h>
#include <util/platform.h>

#include "obs-websocket.h"
#include "WebSocketApi.h"
#include "requesthandler/RequestHandler.h"
#include "websocketserver/WebSocketServer.h"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
// cmocka.h has no extern "C" guard of its own on every platform.
extern "C" {
#include <cmocka.h>
}

static os_cpu_usage_info_t *shared_cpu_info;

/* Plugin globals from obs-websocket.cpp, which would pull in the server. */
os_cpu_usage_info_t *GetCpuUsageInfo()
{
	return shared_cpu_info;
}
ConfigPtr GetConfig()
{
	return nullptr;
}
EventHandlerPtr GetEventHandler()
{
	return nullptr;
}
WebSocketApiPtr GetWebSocketApi()
{
	return nullptr;
}
WebSocketServerPtr GetWebSocketServer()
{
	return nullptr;
}
bool IsDebugEnabled()
{
	return false;
}
bool IsServerOffBySafeMode()
{
	return false;
}
void ClearServerOffBySafeMode() {}

/* Normally defined by OBS_DECLARE_MODULE() in the plugin. */
extern "C" obs_module_t *obs_current_module(void)
{
	return nullptr;
}

/* Referenced by BroadcastCustomEvent and CallVendorRequest, which the test does
 * not send. */
void WebSocketServer::BroadcastEvent(uint64_t, const std::string &, const json &, uint8_t) {}
enum WebSocketApi::RequestReturnCode WebSocketApi::PerformVendorRequest(std::string, std::string, obs_data_t *,
									obs_data_t *)
{
	return NoVendor;
}

/* GetStats asks the frontend for the recording path (for the free disk
 * space); the frontend API has no callbacks here, so the test links this in
 * its place (-Wl,--wrap). */
extern "C" char *__wrap_obs_frontend_get_current_record_output_path(void)
{
	return bstrdup(".");
}

static bool have_obs;

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_startup("en-US", nullptr, nullptr)) {
		fprintf(stderr, "obs_startup failed (no display?); skipping\n");
		return 0;
	}
	have_obs = true;
	shared_cpu_info = os_cpu_usage_info_start();
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (!have_obs) {
		return 0;
	}
	os_cpu_usage_info_destroy(shared_cpu_info);
	shared_cpu_info = nullptr;
	obs_shutdown();
	return 0;
}

static double get_cpu_usage(SessionPtr session)
{
	RequestHandler handler(session);
	RequestResult r = handler.ProcessRequest(Request("GetStats", json::object()));
	assert_int_equal(r.StatusCode, RequestStatus::Success);
	return r.ResponseData["cpuUsage"].get<double>();
}

/* Keeps one core busy for about ms milliseconds of process CPU time. */
static void burn_cpu(uint64_t ms)
{
	volatile uint64_t sink = 0;
	uint64_t end = os_gettime_ns() + ms * 1000000ULL;
	while (os_gettime_ns() < end) {
		for (int i = 0; i < 10000; i++) {
			sink = sink + (uint64_t)i;
		}
	}
}

static void each_session_measures_its_own_interval(void **state)
{
	UNUSED_PARAMETER(state);
	if (!have_obs) {
		skip();
	}

	SessionPtr a = std::make_shared<WebSocketSession>();
	SessionPtr b = std::make_shared<WebSocketSession>();
	get_cpu_usage(a);
	get_cpu_usage(b);

	burn_cpu(400);
	double usage_a = get_cpu_usage(a);
	os_sleep_ms(400);
	double usage_b = get_cpu_usage(b);

	fprintf(stderr, "session a (burn): %.3f%%, session b (burn + idle): %.3f%%\n", usage_a, usage_b);
	assert_true(usage_a > 0.0);
	/* b's interval is the burn plus as long again idle, about half of a's
	 * usage. With one shared state b saw only the idle slice. */
	if (usage_b < usage_a * 0.25) {
		fprintf(stderr,
			"session b reported %.3f%%, under a quarter of a's %.3f%%: it measured "
			"from a's GetStats, not its own\n",
			usage_b, usage_a);
	}
	assert_true(usage_b >= usage_a * 0.25);
}

static void parallel_get_stats_on_one_session(void **state)
{
	UNUSED_PARAMETER(state);
	if (!have_obs) {
		skip();
	}

	SessionPtr session = std::make_shared<WebSocketSession>();
	const int threads = 8;
	const int calls = 50;
	std::vector<double> results(threads * calls, -1.0);
	std::vector<std::thread> pool;

	for (int t = 0; t < threads; t++) {
		pool.emplace_back([&, t]() {
			for (int i = 0; i < calls; i++) {
				RequestHandler handler(t % 2 ? session : nullptr);
				RequestResult r = handler.ProcessRequest(Request("GetStats", json::object()));
				if (r.StatusCode == RequestStatus::Success) {
					results[t * calls + i] = r.ResponseData["cpuUsage"].get<double>();
				}
			}
		});
	}
	for (std::thread &thread : pool) {
		thread.join();
	}

	for (double usage : results) {
		assert_true(std::isfinite(usage));
		assert_true(usage >= 0.0);
		assert_true(usage <= 100.0);
	}
}

int main()
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(each_session_measures_its_own_interval),
		cmocka_unit_test(parallel_get_stats_on_one_session),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
