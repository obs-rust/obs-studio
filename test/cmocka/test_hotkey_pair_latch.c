/* Characterization of the hotkey pair press latch in libobs before its port
 * to the obs-hotkey crate.
 *
 * A hotkey pair (obs_hotkey_pair_register_*) is two hotkeys, such as Start
 * and Stop Streaming, each with a func that returns whether it acted. libobs
 * registers each half with its own callback, which keeps one latch per half
 * (pressed0 and pressed1 of the pair):
 * - while one half is latched, every event of the other half is dropped;
 * - a press whose func returns true latches its half; its release then
 *   clears the latch without calling the func;
 * - any other event calls the func, and if it returns true the latch takes
 *   the event's pressed value.
 *
 * test_hotkey_press_state covers a pair on the hotkey thread, with keys held
 * through XTEST. This test drives the two halves directly, the way the
 * frontend does: callback rerouting is on, and obs_hotkey_trigger_routed_callback()
 * (which the frontend's router and obs-websocket's TriggerHotkeyByName call)
 * runs a half's callback on the calling thread, with the hotkey mutex held.
 * So no key, display input or hotkey thread timing is involved, and every
 * call is synchronous. It links the real libobs and goes only through the
 * public API, so it also covers the hybrid libobs and will see the port.
 * Run under xvfb-run on Linux: obs_startup needs a display there.
 *
 * Cases marked "characterized, not endorsed" pin behaviour that looks odd;
 * the port keeps it, and a fix is a separate change. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>

#define MAX_CALLS 16

/* One func call: the half (0 or 1, from the func's data), the pressed value,
 * and what the func got as the pair id and the hotkey. */
struct call {
	int half;
	bool pressed;
	obs_hotkey_pair_id pair_id;
	obs_hotkey_id hotkey_id;
};

static int obs_up;
static struct call calls[MAX_CALLS];
static size_t num_calls;

/* What each half's func returns on a press and on a release. */
static bool result_press[2];
static bool result_release[2];

/* The pair under test and its two hotkeys. */
static obs_hotkey_pair_id pair = OBS_INVALID_HOTKEY_PAIR_ID;
static obs_hotkey_id half_id[2];

/* A func that, on a press of half 0, first triggers a press of half 1. */
static bool reentrant;

/* ------------------------------------------------------------------------- */

static bool pair_func(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed)
{
	int half = (int)(intptr_t)data;
	if (num_calls < MAX_CALLS)
		calls[num_calls] = (struct call){half, pressed, id, hotkey ? obs_hotkey_get_id(hotkey) : 0};
	num_calls++;

	if (reentrant && half == 0 && pressed)
		obs_hotkey_trigger_routed_callback(half_id[1], true);

	return pressed ? result_press[half] : result_release[half];
}

static bool find_hotkey(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	struct {
		const char *name;
		obs_hotkey_id id;
	} *find = data;
	if (strcmp(obs_hotkey_get_name(key), find->name) != 0)
		return true;
	find->id = id;
	return false;
}

static obs_hotkey_id hotkey_named(const char *name)
{
	struct {
		const char *name;
		obs_hotkey_id id;
	} find = {name, OBS_INVALID_HOTKEY_ID};
	obs_enum_hotkeys(find_hotkey, &find);
	assert_int_not_equal(find.id, OBS_INVALID_HOTKEY_ID);
	return find.id;
}

/* Press (true) or release (false) one half, as the frontend does after its
 * router got the event. */
static void press(int half)
{
	obs_hotkey_trigger_routed_callback(half_id[half], true);
}

static void release(int half)
{
	obs_hotkey_trigger_routed_callback(half_id[half], false);
}

/* The func calls since the last check were exactly these (half, pressed);
 * each also got the pair's id and its own half's hotkey. */
static void expect_calls(const struct call *expected, size_t count)
{
	bool same = num_calls == count;
	for (size_t i = 0; same && i < count; i++)
		same = calls[i].half == expected[i].half && calls[i].pressed == expected[i].pressed;
	if (!same) {
		fprintf(stderr, "calls:   ");
		for (size_t i = 0; i < num_calls && i < MAX_CALLS; i++)
			fprintf(stderr, " %d%s", calls[i].half, calls[i].pressed ? "+" : "-");
		fprintf(stderr, "\nexpected:");
		for (size_t i = 0; i < count; i++)
			fprintf(stderr, " %d%s", expected[i].half, expected[i].pressed ? "+" : "-");
		fprintf(stderr, "\n");
	}
	assert_true(same);
	for (size_t i = 0; i < count; i++) {
		assert_true(calls[i].pair_id == pair);
		assert_true(calls[i].hotkey_id == half_id[calls[i].half]);
	}
	num_calls = 0;
}

#define EXPECT(...)                                                                 \
	do {                                                                        \
		const struct call expected_[] = {__VA_ARGS__};                      \
		expect_calls(expected_, sizeof(expected_) / sizeof(expected_[0])); \
	} while (false)
#define EXPECT_NONE() expect_calls(NULL, 0)

#define P(half) {(half), true, 0, 0}
#define R(half) {(half), false, 0, 0}

/* Each half's func returns `on_press` for a press and `on_release` for a
 * release. */
static void funcs_return(int half, bool on_press, bool on_release)
{
	result_press[half] = on_press;
	result_release[half] = on_release;
}

/* ------------------------------------------------------------------------- */

static int setup(void **state)
{
	(void)state;
	num_calls = 0;
	reentrant = false;
	/* The frontend's funcs: true for a press they act on, false for a
	 * release. */
	funcs_return(0, true, false);
	funcs_return(1, true, false);

	pair = obs_hotkey_pair_register_frontend("test.pair.first", "first", "test.pair.second", "second", pair_func,
						 pair_func, (void *)(intptr_t)0, (void *)(intptr_t)1);
	assert_int_not_equal(pair, OBS_INVALID_HOTKEY_PAIR_ID);
	half_id[0] = hotkey_named("test.pair.first");
	half_id[1] = hotkey_named("test.pair.second");
	obs_hotkey_enable_callback_rerouting(true);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_hotkey_enable_callback_rerouting(false);
	obs_hotkey_pair_unregister(pair);
	pair = OBS_INVALID_HOTKEY_PAIR_ID;
	return 0;
}

/* ------------------------------------------------------------------------- */
/* One half */

/* A press calls the func, which gets the pair's id and the half's own
 * hotkey. It returns true, so the half latches, and its release clears the
 * latch without calling the func. The next press calls it again. */
static void press_and_release_first(void **state)
{
	(void)state;
	press(0);
	EXPECT(P(0));
	release(0);
	EXPECT_NONE();
	press(0);
	EXPECT(P(0));
	release(0);
	EXPECT_NONE();
}

static void press_and_release_second(void **state)
{
	(void)state;
	press(1);
	EXPECT(P(1));
	release(1);
	EXPECT_NONE();
	press(1);
	EXPECT(P(1));
	release(1);
	EXPECT_NONE();
}

/* A press whose func returns false does not latch: the release calls the
 * func too, and the other half is not blocked. */
static void press_not_acted_on(void **state)
{
	(void)state;
	funcs_return(0, false, false);
	press(0);
	EXPECT(P(0));
	press(1);
	EXPECT(P(1));
	release(1);
	EXPECT_NONE();
	release(0);
	EXPECT(R(0));
}

/* A release without a press calls the func, whatever it returns. A func that
 * returns true for a release latches nothing (the latch takes false). */
static void release_without_press(void **state)
{
	(void)state;
	release(0);
	EXPECT(R(0));
	funcs_return(0, true, true);
	release(0);
	EXPECT(R(0));
	press(1);
	EXPECT(P(1));
	release(1);
	EXPECT_NONE();
}

/* A second press of a latched half calls the func again, and the latch
 * takes its result only if it returns true: a false keeps the half latched,
 * so its one release still clears the latch without a call. Characterized,
 * not endorsed: libobs never sends two presses of one hotkey without a
 * release (the pressed count), but obs-websocket's TriggerHotkeyByName and
 * the frontend's router reach the callback directly. */
static void press_while_latched(void **state)
{
	(void)state;
	press(0);
	EXPECT(P(0));
	funcs_return(0, false, false);
	press(0);
	EXPECT(P(0));
	press(1);
	EXPECT_NONE();
	release(0);
	EXPECT_NONE();
	release(0);
	EXPECT(R(0));
}

/* ------------------------------------------------------------------------- */
/* Both halves */

/* While the first half is latched, the second half's press and release are
 * dropped. Once the first is released, the second works again. */
static void second_blocked_while_first_latched(void **state)
{
	(void)state;
	press(0);
	EXPECT(P(0));
	press(1);
	EXPECT_NONE();
	release(1);
	EXPECT_NONE();
	release(0);
	EXPECT_NONE();
	press(1);
	EXPECT(P(1));
	release(1);
	EXPECT_NONE();
}

static void first_blocked_while_second_latched(void **state)
{
	(void)state;
	press(1);
	EXPECT(P(1));
	press(0);
	EXPECT_NONE();
	release(0);
	EXPECT_NONE();
	release(1);
	EXPECT_NONE();
	press(0);
	EXPECT(P(0));
	release(0);
	EXPECT_NONE();
}

/* The second half pressed while the first is latched, and released after
 * the first: its press was dropped, its release reaches its func.
 * Characterized, not endorsed: the func gets a release without a press. The
 * frontend's funcs ignore releases. This is also what one key bound to both
 * halves does (Start and Stop on one key): the hotkey thread presses both in
 * binding order and releases both in that order. */
static void second_released_after_first(void **state)
{
	(void)state;
	press(0);
	EXPECT(P(0));
	press(1);
	EXPECT_NONE();
	release(0);
	EXPECT_NONE();
	release(1);
	EXPECT(R(1));
}

/* One key on both halves used as a toggle, with the frontend's funcs: each
 * acts only when it applies (here the first acts, then only the second). */
static void one_key_toggle(void **state)
{
	(void)state;
	press(0);
	press(1);
	release(0);
	release(1);
	EXPECT(P(0), R(1));

	funcs_return(0, false, false);
	press(0);
	press(1);
	release(0);
	release(1);
	EXPECT(P(0), P(1));

	funcs_return(0, true, false);
	funcs_return(1, false, false);
	press(0);
	press(1);
	release(0);
	release(1);
	EXPECT(P(0), R(1));
}

/* The latch is per pair: a latched pair does not block another pair. */
static bool other_func(void *data, obs_hotkey_pair_id id, obs_hotkey_t *hotkey, bool pressed)
{
	(void)id;
	(void)hotkey;
	int *count = data;
	if (pressed)
		(*count)++;
	return pressed;
}

static void latch_per_pair(void **state)
{
	(void)state;
	int other_first = 0, other_second = 0;
	obs_hotkey_pair_id other =
		obs_hotkey_pair_register_frontend("test.pair.other.first", "other first", "test.pair.other.second",
						  "other second", other_func, other_func, &other_first, &other_second);
	assert_int_not_equal(other, OBS_INVALID_HOTKEY_PAIR_ID);
	obs_hotkey_id other_second_id = hotkey_named("test.pair.other.second");

	press(0);
	EXPECT(P(0));
	obs_hotkey_trigger_routed_callback(other_second_id, true);
	assert_int_equal(other_second, 1);
	press(1);
	EXPECT_NONE();
	obs_hotkey_trigger_routed_callback(other_second_id, false);
	release(0);
	EXPECT_NONE();
	assert_int_equal(other_first, 0);

	obs_hotkey_pair_unregister(other);
}

/* A func that triggers its partner from inside its press: the partner's
 * press runs first (the first half is not latched yet) and latches the
 * second half, then the first half latches when its func returns. Both are
 * now latched, and every later event of either half is dropped: the pair is
 * dead until it is registered again. Characterized, not endorsed. Only a
 * func that triggers the other half synchronously gets here; the frontend's
 * funcs do not. It also pins that each half writes only its own latch after
 * its func returns. */
static void reentrant_press_latches_both(void **state)
{
	(void)state;
	reentrant = true;
	press(0);
	EXPECT(P(0), P(1));
	reentrant = false;

	release(0);
	release(1);
	press(0);
	press(1);
	release(1);
	release(0);
	EXPECT_NONE();
}

/* ------------------------------------------------------------------------- */

int main(void)
{
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		puts("obs_startup failed, skipping");
		return 0;
	}

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(press_and_release_first, setup, teardown),
		cmocka_unit_test_setup_teardown(press_and_release_second, setup, teardown),
		cmocka_unit_test_setup_teardown(press_not_acted_on, setup, teardown),
		cmocka_unit_test_setup_teardown(release_without_press, setup, teardown),
		cmocka_unit_test_setup_teardown(press_while_latched, setup, teardown),
		cmocka_unit_test_setup_teardown(second_blocked_while_first_latched, setup, teardown),
		cmocka_unit_test_setup_teardown(first_blocked_while_second_latched, setup, teardown),
		cmocka_unit_test_setup_teardown(second_released_after_first, setup, teardown),
		cmocka_unit_test_setup_teardown(one_key_toggle, setup, teardown),
		cmocka_unit_test_setup_teardown(latch_per_pair, setup, teardown),
		cmocka_unit_test_setup_teardown(reentrant_press_latches_both, setup, teardown),
	};
	int rc = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return rc;
}
