// The C++ headers go first: cmocka.h defines macros (fail, ...) that clash
// with libstdc++.
#include <chrono>
#include <condition_variable>
#include <mutex>
#include <string>
#include <thread>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
// The Windows cmocka.h has no extern "C" guard of its own.
extern "C" {
#include <cmocka.h>
}

#include "disk-space-monitor.hpp"

/* obs-rust/obs-studio#1282: the recording timer (every second) and the stats
 * window (every two seconds) queried free disk space on the UI thread; on a
 * hung network path each query blocked for seconds and froze the UI.
 *
 * obs-rust/obs-studio#3949: the cases synchronize on the query instead of
 * timing calls against the wall clock. The fake free-space query blocks until
 * the test opens its gate, so every "does not block" assertion is made while
 * a query is provably still running. */

using Clock = std::chrono::steady_clock;

/* Only turns a hang into a failure; no case passes by waiting it out. */
static const auto BOUND = std::chrono::seconds(10);

/* A hung volume: each query blocks until the gate opens. "/a" has 111 bytes
 * free, anything else 222. If the gate stays shut past BOUND the fake opens
 * it itself and records that, so a blocking regression fails the case
 * instead of hanging it. */
static struct {
	std::mutex mutex;
	std::condition_variable cv;
	int entered = 0;
	int returned = 0;
	bool open = false;
	bool watchdog_fired = false;
} gate;

static void gate_reset()
{
	std::lock_guard<std::mutex> lock(gate.mutex);
	gate.entered = 0;
	gate.returned = 0;
	gate.open = false;
	gate.watchdog_fired = false;
}

static uint64_t gated_free_space(const char *path)
{
	std::unique_lock<std::mutex> lock(gate.mutex);
	gate.entered++;
	gate.cv.notify_all();
	if (!gate.cv.wait_for(lock, BOUND, [] { return gate.open; })) {
		gate.watchdog_fired = true;
		gate.open = true;
	}
	gate.returned++;
	gate.cv.notify_all();
	return std::string(path) == "/a" ? 111 : 222;
}

/* Wait (bounded) until the fake has been entered n times in total. */
static int wait_entered(int n)
{
	std::unique_lock<std::mutex> lock(gate.mutex);
	gate.cv.wait_for(lock, BOUND, [&] { return gate.entered >= n; });
	return gate.entered;
}

/* Wait (bounded) until the fake has returned n times in total. */
static int wait_returned(int n)
{
	std::unique_lock<std::mutex> lock(gate.mutex);
	gate.cv.wait_for(lock, BOUND, [&] { return gate.returned >= n; });
	return gate.returned;
}

static void gate_open()
{
	std::lock_guard<std::mutex> lock(gate.mutex);
	gate.open = true;
	gate.cv.notify_all();
}

static void gate_close()
{
	std::lock_guard<std::mutex> lock(gate.mutex);
	gate.open = false;
}

static int gate_entered()
{
	std::lock_guard<std::mutex> lock(gate.mutex);
	return gate.entered;
}

static int gate_returned()
{
	std::lock_guard<std::mutex> lock(gate.mutex);
	return gate.returned;
}

/* The test, not the fake's watchdog, released every query. */
static void assert_test_opened_gate()
{
	std::lock_guard<std::mutex> lock(gate.mutex);
	assert_false(gate.watchdog_fired);
}

/* Wait (bounded) until latest(path) is valid. */
static DiskSpaceSample wait_for(DiskSpaceMonitor &m, const std::string &path)
{
	auto start = Clock::now();
	DiskSpaceSample s = m.latest(path);
	while (!s.valid && Clock::now() - start < BOUND) {
		std::this_thread::sleep_for(std::chrono::milliseconds(10));
		s = m.latest(path);
	}
	return s;
}

static void request_and_latest_do_not_block(void **state)
{
	(void)state;
	gate_reset();
	DiskSpaceMonitor m(gated_free_space);

	m.request("/a");
	assert_int_equal(wait_entered(1), 1);

	/* the query is held: both calls return while it is still running */
	assert_false(m.latest("/a").valid);
	m.request("/a");
	assert_false(m.latest("/a").valid);
	assert_int_equal(gate_returned(), 0);
	assert_test_opened_gate();

	gate_open();
	DiskSpaceSample s = wait_for(m, "/a");
	assert_true(s.valid);
	assert_int_equal(s.bytes, 111);
	assert_true(s.timestamp_ns != 0);
	assert_test_opened_gate();
}

static void requests_in_flight_are_coalesced(void **state)
{
	(void)state;
	gate_reset();
	DiskSpaceMonitor m(gated_free_space);

	/* some land before the worker takes the request (the pending slot) */
	for (int i = 0; i < 25; i++) {
		m.request("/a");
	}
	assert_int_equal(wait_entered(1), 1);
	/* the rest while the query is held (the query in flight) */
	for (int i = 0; i < 25; i++) {
		m.request("/a");
	}
	assert_int_equal(m.queries_started(), 1);
	assert_int_equal(gate_entered(), 1);
	assert_int_equal(gate_returned(), 0);
	assert_test_opened_gate();

	gate_open();
	assert_true(wait_for(m, "/a").valid);
	/* the worker counts a second query in the same critical section that
	 * publishes the sample, so none started or is about to */
	assert_int_equal(m.queries_started(), 1);
	assert_test_opened_gate();
}

static void path_change_returns_new_value(void **state)
{
	(void)state;
	gate_reset();
	DiskSpaceMonitor m(gated_free_space);

	gate_open();
	m.request("/a");
	assert_int_equal(wait_for(m, "/a").bytes, 111);

	gate_close();
	m.request("/b");
	assert_int_equal(wait_entered(2), 2);
	/* the old path's value is not reported for the new path */
	assert_false(m.latest("/b").valid);
	assert_int_equal(gate_returned(), 1);
	assert_test_opened_gate();

	gate_open();
	DiskSpaceSample s = wait_for(m, "/b");
	assert_true(s.valid);
	assert_int_equal(s.bytes, 222);
	assert_test_opened_gate();
}

static void destroy_does_not_wait_for_hung_query(void **state)
{
	(void)state;
	gate_reset();
	{
		DiskSpaceMonitor m(gated_free_space);
		m.request("/a");
		assert_int_equal(wait_entered(1), 1);
	}
	/* the destructor returned while the query is still held */
	assert_int_equal(gate_returned(), 0);
	assert_test_opened_gate();

	/* let the detached worker finish before the next case */
	gate_open();
	assert_int_equal(wait_returned(1), 1);
	assert_test_opened_gate();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(request_and_latest_do_not_block),
		cmocka_unit_test(requests_in_flight_are_coalesced),
		cmocka_unit_test(path_change_returns_new_value),
		cmocka_unit_test(destroy_does_not_wait_for_hung_query),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
