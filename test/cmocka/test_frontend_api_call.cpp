/* Regression test for obsproject/obs-websocket#1283 (onyx-and-iris): after
 * obs-websocket's CreateProfile succeeds, GetProfileList does not list the
 * new profile yet, and RemoveProfile leaves the removed profile in the list.
 * A client that creates a profile and then sets its parameters
 * (SetProfileParameter) writes them into the profile that was current
 * before.
 *
 * obs_frontend_create_profile(), obs_frontend_duplicate_profile() and
 * obs_frontend_delete_profile() (frontend/OBSStudioAPI.cpp) hand the work to
 * the UI thread through RunFrontendApiCall()
 * (frontend/utility/frontend-api-call.hpp). Called from another thread (an
 * obs-websocket request, a plugin's worker), they returned before the UI
 * thread had run the call, so whatever the caller read next was the old
 * state. obs_frontend_add_scene_collection() already waits.
 *
 * The cases call RunFrontendApiCall() as the API functions do, with a QObject
 * on the main thread standing in for the main window and a list of profiles
 * standing in for its profile list:
 * - from another thread, the call must have run when it returns;
 * - on the UI thread it runs at once (unchanged);
 * - on a libobs thread (must_not_wait) it must not wait for the UI thread,
 *   which may be waiting for that thread (unchanged: it runs later).
 *
 * QtCore only; no display. */

#include <QCoreApplication>
#include <QElapsedTimer>
#include <QObject>
#include <QString>
#include <QStringList>

#include <atomic>
#include <cstdio>
#include <chrono>
#include <thread>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

#include "frontend-api-call.hpp"

static QObject *ui;
static QStringList profiles;

/* Waits up to wait_ms without running the UI thread's event loop, so a
 * caller that does not wait has returned before the UI thread could run the
 * call; then runs the event loop until done is set or timeout_ms passed. */
static bool settle_then_pump(const std::atomic<bool> &done, int wait_ms, int timeout_ms);

/* Runs the UI thread's event loop until done is set or timeout_ms passed. */
static bool pump_until(const std::atomic<bool> &done, int timeout_ms)
{
	QElapsedTimer timer;
	timer.start();
	while (!done.load() && timer.elapsed() < timeout_ms) {
		QCoreApplication::processEvents(QEventLoop::AllEvents, 10);
	}
	return done.load();
}

static bool settle_then_pump(const std::atomic<bool> &done, int wait_ms, int timeout_ms)
{
	QElapsedTimer timer;
	timer.start();
	while (!done.load() && timer.elapsed() < wait_ms) {
		std::this_thread::sleep_for(std::chrono::milliseconds(5));
	}
	return pump_until(done, timeout_ms);
}

/* obs-websocket's CreateProfile then GetProfileList, on a request thread. */
static void test_other_thread_sees_created_profile(void **state)
{
	(void)state;
	profiles = {"Untitled"};
	std::atomic<bool> done{false};
	bool listed = false;

	std::thread request([&]() {
		RunFrontendApiCall(ui, []() { profiles.append("test"); }, false);
		/* GetProfileList, right after the request returned */
		listed = profiles.contains("test");
		done = true;
	});
	bool returned = settle_then_pump(done, 200, 5000);
	request.join();
	QCoreApplication::processEvents();

	assert_true(returned);
	if (!listed) {
		fprintf(stderr, "the call returned before the UI thread created the profile\n");
	}
	assert_true(listed);
}

/* RemoveProfile then GetProfileList. */
static void test_other_thread_sees_removed_profile(void **state)
{
	(void)state;
	profiles = {"Untitled", "test"};
	std::atomic<bool> done{false};
	bool still_listed = true;

	std::thread request([&]() {
		RunFrontendApiCall(ui, []() { profiles.removeAll("test"); }, false);
		still_listed = profiles.contains("test");
		done = true;
	});
	bool returned = settle_then_pump(done, 200, 5000);
	request.join();
	QCoreApplication::processEvents();

	assert_true(returned);
	if (still_listed) {
		fprintf(stderr, "the call returned before the UI thread removed the profile\n");
	}
	assert_false(still_listed);
}

/* A plugin on the UI thread: the call runs at once, as before. */
static void test_ui_thread_runs_at_once(void **state)
{
	(void)state;
	profiles = {"Untitled"};
	RunFrontendApiCall(ui, []() { profiles.append("test"); }, false);
	assert_true(profiles.contains("test"));
}

/* On a libobs thread the call must not wait: the UI thread may be waiting
 * for that thread (activating a profile resets video). The UI thread does
 * not process events until the caller has returned. */
static void test_libobs_thread_does_not_wait(void **state)
{
	(void)state;
	profiles = {"Untitled"};
	std::atomic<bool> done{false};

	std::thread graphics([&]() {
		RunFrontendApiCall(ui, []() { profiles.append("test"); }, true);
		done = true;
	});

	QElapsedTimer timer;
	timer.start();
	while (!done.load() && timer.elapsed() < 2000) {
		std::this_thread::sleep_for(std::chrono::milliseconds(10));
	}
	bool returned = done.load();
	/* let a blocked caller go before joining, so a failure does not hang */
	pump_until(done, 5000);
	graphics.join();
	QCoreApplication::processEvents();

	assert_true(returned);
	assert_true(profiles.contains("test"));
}

int main(int argc, char **argv)
{
	QCoreApplication app(argc, argv);
	QObject main_window;
	ui = &main_window;

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_other_thread_sees_created_profile),
		cmocka_unit_test(test_other_thread_sees_removed_profile),
		cmocka_unit_test(test_ui_thread_runs_at_once),
		cmocka_unit_test(test_libobs_thread_does_not_wait),
	};

	return cmocka_run_group_tests_name("frontend_api_call", tests, NULL, NULL);
}
