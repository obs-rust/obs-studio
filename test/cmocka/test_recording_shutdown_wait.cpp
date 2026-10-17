/* Quitting OBS while recording waited for the recording to stop by sleeping
 * on the UI thread in 10 ms steps, up to 5 s, so the window froze for the
 * whole wait: no repaint, no timer, no queued event ran (#1283).
 *
 * WaitForOutputToStop() (frontend/widgets/RecordingShutdownWait.hpp) is the
 * wait OBSBasic::closeWindow() now uses. The fake output here has the
 * libobs output handler and stops on another thread the way libobs does: it
 * handler "stop" while still active, then "deactivate", then is no longer
 * active. The tests check that the wait ends after the stop, that a timer
 * posted to the UI event loop fires during the wait, and that the timeout
 * still ends the wait (and force-stops the recording) when no stop comes.
 *
 * Needs only QtCore, so it runs without a display.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
// cmocka.h 1.1 (Debian 13) and the Windows cmocka.h have no extern "C" guard.
extern "C" {
#include <cmocka.h>
}

#include <atomic>
#include <chrono>
#include <cstdio>
#include <thread>

#include <QCoreApplication>
#include <QTimer>

#include <callback/signal.h>
#include <util/platform.h>

#include "RecordingShutdown.hpp"
#include "RecordingShutdownWait.hpp"

#define TIMEOUT_MS 5000

struct FakeOutput {
	signal_handler_t *handler = nullptr;
	std::atomic<bool> active{true};
	std::thread stopper;

	FakeOutput()
	{
		handler = signal_handler_create();
		signal_handler_add(handler, "void stop(ptr output, int code)");
		signal_handler_add(handler, "void deactivate(ptr output)");
	}

	~FakeOutput()
	{
		if (stopper.joinable()) {
			stopper.join();
		}
		signal_handler_destroy(handler);
	}

	/* stop on another thread after delay_ms, in libobs's order */
	void StopAfter(int delay_ms)
	{
		stopper = std::thread([this, delay_ms]() {
			std::this_thread::sleep_for(std::chrono::milliseconds(delay_ms));
			uint8_t stack[128];
			calldata_t cd;
			calldata_init_fixed(&cd, stack, sizeof(stack));
			calldata_set_ptr(&cd, "output", nullptr);
			calldata_set_int(&cd, "code", 0);
			signal_handler_signal(handler, "stop", &cd);
			std::this_thread::sleep_for(std::chrono::milliseconds(20));
			signal_handler_signal(handler, "deactivate", &cd);
			active = false;
		});
	}

	bool Wait(uint64_t timeout_ms)
	{
		return WaitForOutputToStop(handler, timeout_ms, [this]() { return active.load(); });
	}
};

/* a timer posted to the UI event loop before the wait starts */
struct PostedTimer {
	QTimer timer;
	bool fired = false;
	bool fired_while_active = false;

	explicit PostedTimer(FakeOutput &out)
	{
		timer.setSingleShot(true);
		QObject::connect(&timer, &QTimer::timeout, [this, &out]() {
			fired = true;
			fired_while_active = out.active.load();
		});
		timer.start(20);
	}
};

static uint64_t ms_since(uint64_t start_ns)
{
	return (os_gettime_ns() - start_ns) / 1000000;
}

/* The wait ends once the output has stopped, and the UI event loop runs
 * while it waits. */
static void test_wait_ends_on_stop_and_keeps_ui_running(void **state)
{
	(void)state;
	FakeOutput out;
	PostedTimer timer(out);
	out.StopAfter(200);

	uint64_t start = os_gettime_ns();
	bool stopped = out.Wait(TIMEOUT_MS);
	uint64_t elapsed = ms_since(start);

	printf("stopped %d after %llu ms, posted timer fired %d (while waiting %d)\n", stopped,
	       (unsigned long long)elapsed, timer.fired, timer.fired_while_active);

	assert_true(stopped);
	assert_false(out.active.load());
	assert_true(elapsed >= 150);
	assert_true(elapsed < TIMEOUT_MS);
	assert_true(timer.fired);
	assert_true(timer.fired_while_active);
}

/* With no stop, the wait ends at the timeout, and the UI still runs. */
static void test_wait_times_out_without_stop(void **state)
{
	(void)state;
	FakeOutput out;
	PostedTimer timer(out);

	uint64_t start = os_gettime_ns();
	bool stopped = out.Wait(300);
	uint64_t elapsed = ms_since(start);

	printf("no stop: returned %d after %llu ms, posted timer fired %d\n", stopped, (unsigned long long)elapsed,
	       timer.fired);

	assert_false(stopped);
	assert_true(elapsed >= 250);
	assert_true(elapsed < 3000);
	assert_true(timer.fired);
}

/* An output that is no longer active needs no wait. */
static void test_wait_returns_at_once_when_stopped(void **state)
{
	(void)state;
	FakeOutput out;
	out.active = false;

	uint64_t start = os_gettime_ns();
	assert_true(out.Wait(TIMEOUT_MS));
	assert_true(ms_since(start) < 1000);
}

struct FakeHandler {
	FakeOutput *out;
	int gracefulStops = 0;
	int forcedStops = 0;

	bool RecordingActive() const { return out->active.load(); }
	void StopRecording(bool force)
	{
		if (force) {
			forcedStops++;
			out->active = false;
		} else {
			gracefulStops++;
		}
	}
};

/* The quit path: a recording that never stops is force-stopped at the
 * timeout, and one that stops is not. */
static void test_shutdown_with_event_loop_wait(void **state)
{
	(void)state;
	auto wait = [](FakeOutput &out) {
		return [&out](uint64_t ms) {
			return out.Wait(ms);
		};
	};

	FakeOutput stuck;
	FakeHandler stuck_handler{&stuck};
	assert_false(StopRecordingForShutdown(&stuck_handler, 300, wait(stuck)));
	assert_int_equal(stuck_handler.gracefulStops, 1);
	assert_int_equal(stuck_handler.forcedStops, 1);

	FakeOutput stopping;
	FakeHandler handler{&stopping};
	stopping.StopAfter(200);
	assert_true(StopRecordingForShutdown(&handler, TIMEOUT_MS, wait(stopping)));
	assert_int_equal(handler.gracefulStops, 1);
	assert_int_equal(handler.forcedStops, 0);
}

int main(int argc, char **argv)
{
	QCoreApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_wait_ends_on_stop_and_keeps_ui_running),
		cmocka_unit_test(test_wait_times_out_without_stop),
		cmocka_unit_test(test_wait_returns_at_once_when_stopped),
		cmocka_unit_test(test_shutdown_with_event_loop_wait),
	};
	return cmocka_run_group_tests(tests, nullptr, nullptr);
}
