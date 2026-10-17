/* Volume meter repaints (obs-rust/obs-studio#1281): one shared 16 ms timer
 * repainted every meter forever, also while the mixer was hidden, OBS was
 * minimized or all sources were silent. volumeMeterSettled() says when a
 * repaint would draw the same meter; VolumeMeterTicker repaints only meters
 * that want it, stops when none does, and wake() (from any thread) starts
 * it again. Runs on Qt's offscreen platform with fake meters. */

#include <QCoreApplication>
#include <QElapsedTimer>

/* Before cmocka.h: cmocka 2.x defines a stop() macro, which turns the
 * ticker's QTimer::stop() call into an undefined QTimer::_stop(). */
#include "VolumeMeterTicker.hpp"

#include <atomic>
#include <cmath>
#include <limits>
#include <thread>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

static const float inf = std::numeric_limits<float>::infinity();
static const float floor_db = -60.0f;

struct Levels {
	float currentMagnitude[2];
	float currentPeak[2];
	float currentInputPeak[2];
	float displayMagnitude[2];
	float displayPeak[2];
	float displayPeakHold[2];
	float displayInputPeakHold[2];

	bool settled() const
	{
		return volumeMeterSettled(currentMagnitude, currentPeak, currentInputPeak, displayMagnitude,
					  displayPeak, displayPeakHold, displayInputPeakHold, floor_db);
	}
};

/* A silent source: input -inf, the display at the floor. */
static Levels silent()
{
	return {{-inf, -inf},     {-inf, -inf}, {-inf, -inf}, {floor_db, floor_db},
		{-75.0f, -90.0f}, {-inf, -inf}, {-inf, -inf}};
}

static void silence_after_decay_is_settled(void **)
{
	assert_true(silent().settled());
}

static void a_decaying_peak_is_not_settled(void **)
{
	Levels l = silent();
	l.displayPeak[1] = -20.0f;
	assert_false(l.settled());
}

static void a_pending_peak_hold_is_not_settled(void **)
{
	// A steady tone at -40 dB, but the hold still shows an earlier -6 dB.
	Levels l = {{-40.0f, -40.0f}, {-40.0f, -40.0f}, {-40.0f, -40.0f}, {-40.0f, -40.0f},
		    {-40.0f, -40.0f}, {-6.0f, -40.0f},  {-40.0f, -40.0f}};
	assert_false(l.settled());
	l.displayPeakHold[0] = -40.0f;
	assert_true(l.settled());
}

static void the_magnitude_settles_within_a_tenth_of_a_db(void **)
{
	Levels l = {{-40.0f, -40.0f}, {-40.0f, -40.0f}, {-40.0f, -40.0f}, {-40.03f, -39.96f},
		    {-40.0f, -40.0f}, {-40.0f, -40.0f}, {-40.0f, -40.0f}};
	assert_true(l.settled());
	l.displayMagnitude[1] = -41.0f;
	assert_false(l.settled());
}

static void a_nan_is_not_settled(void **)
{
	Levels l = silent();
	l.displayPeak[0] = std::nanf("");
	assert_false(l.settled());
}

/* Run the event loop for `ms` milliseconds. */
static void run_for(int ms)
{
	QElapsedTimer t;
	t.start();
	while (t.elapsed() < ms) {
		QCoreApplication::processEvents(QEventLoop::AllEvents, 5);
	}
}

struct FakeMeter {
	std::atomic<bool> wants{false};
	std::atomic<int> repaints{0};

	VolumeMeterTicker::Client client()
	{
		return {[this]() { return wants.load(); }, [this]() { repaints++; }};
	}
};

/* Each ticker test gets a fresh ticker and two meters. A failed assert
 * longjmps past destructors, so they live on the heap and teardown deletes
 * them; a ticker left alive would fire into freed meters. */
static VolumeMeterTicker *ticker;
static FakeMeter *a, *b;

static int setup(void **)
{
	ticker = new VolumeMeterTicker(QCoreApplication::instance());
	a = new FakeMeter();
	b = new FakeMeter();
	return 0;
}

static int teardown(void **)
{
	delete ticker;
	delete a;
	delete b;
	ticker = nullptr;
	a = b = nullptr;
	return 0;
}

static void meters_with_nothing_to_draw_stop_the_timer(void **)
{
	// Hidden, minimized or settled meters: nothing wants a repaint.
	ticker->add(a->client());
	ticker->add(b->client());
	run_for(200);
	assert_false(ticker->active());
	assert_int_equal(a->repaints.load() + b->repaints.load(), 0);
}

static void a_meter_that_still_moves_keeps_the_timer_running(void **)
{
	a->wants = true;
	ticker->add(a->client());
	ticker->add(b->client());
	run_for(200);
	assert_true(ticker->active());
	assert_true(a->repaints.load() >= 5);
	assert_int_equal(b->repaints.load(), 0);
	// Once it settles, the timer stops.
	a->wants = false;
	run_for(100);
	assert_false(ticker->active());
}

static void a_wake_from_another_thread_restarts_the_timer(void **)
{
	ticker->add(a->client());
	run_for(100);
	assert_false(ticker->active());
	// The audio thread delivers a new level.
	std::thread audio([]() {
		a->wants = true;
		ticker->wake();
	});
	audio.join();
	run_for(100);
	assert_true(ticker->active());
	assert_true(a->repaints.load() >= 3);
}

static void a_removed_meter_is_not_repainted(void **)
{
	a->wants = true;
	int id = ticker->add(a->client());
	ticker->remove(id);
	ticker->wake();
	run_for(100);
	assert_int_equal(a->repaints.load(), 0);
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QCoreApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(silence_after_decay_is_settled),
		cmocka_unit_test(a_decaying_peak_is_not_settled),
		cmocka_unit_test(a_pending_peak_hold_is_not_settled),
		cmocka_unit_test(the_magnitude_settles_within_a_tenth_of_a_db),
		cmocka_unit_test(a_nan_is_not_settled),
		cmocka_unit_test_setup_teardown(meters_with_nothing_to_draw_stop_the_timer, setup, teardown),
		cmocka_unit_test_setup_teardown(a_meter_that_still_moves_keeps_the_timer_running, setup, teardown),
		cmocka_unit_test_setup_teardown(a_wake_from_another_thread_restarts_the_timer, setup, teardown),
		cmocka_unit_test_setup_teardown(a_removed_meter_is_not_repainted, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
