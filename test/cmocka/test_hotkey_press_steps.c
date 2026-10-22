/* The hotkey press state machine (libobs/obs-hotkey-press.c), poll by poll.
 *
 * This test compiles obs-hotkey.c and obs-hotkey-press.c into itself with an
 * isolated core (obs is renamed hotkey_test_obs by CMake) and answers
 * obs_hotkeys_platform_is_pressed() from a scripted key state that logs
 * every query. No hotkey thread runs: each case calls the per-binding entry
 * points the way query_hotkeys() and obs_hotkey_inject_event() do, and
 * checks the binding's pressed and modifiers_match fields, its hotkey's
 * pressed count, the callbacks, and which keys were queried in which order.
 * So it characterizes the C only; test_hotkey_press_state covers the real
 * libobs (and with it a Rust swap) through the public API.
 *
 * It pins what the real libobs cannot reach or cannot show:
 * - that obs_hotkey_binding_fields() points at the binding's own key,
 *   pressed flag and modifiers_match latch, the fields the state machine
 *   reads and writes through it;
 * - the non-strict modifier branch (libobs always sets strict_modifiers);
 * - the modifiers_match latch, one poll at a time;
 * - that the key state is queried lazily: the modifiers in a fixed order,
 *   then a binding's key only when its modifiers and latch already match,
 *   never for a modifier-only or an empty binding.
 *
 * Cases marked "characterized, not endorsed" pin behaviour that looks odd;
 * the port keeps it, and a fix is a separate change. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs-internal.h>

struct obs_core *hotkey_test_obs;

#define MAX_LOG 32

static bool held[OBS_KEY_LAST_VALUE];
static obs_key_t queried[MAX_LOG];
static size_t num_queried;

bool obs_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	UNUSED_PARAMETER(context);
	if (num_queried < MAX_LOG)
		queried[num_queried] = key;
	num_queried++;
	return (unsigned)key < OBS_KEY_LAST_VALUE && held[key];
}

/* One callback: the hotkey's tag (a routed one: 100 + id) and pressed. */
struct event {
	long tag;
	bool pressed;
};

static struct event events[MAX_LOG];
static size_t num_events;

static void log_event(long tag, bool pressed)
{
	if (num_events < MAX_LOG)
		events[num_events] = (struct event){tag, pressed};
	num_events++;
}

static void hotkey_callback(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);
	log_event((long)(intptr_t)data, pressed);
}

static void router(void *data, obs_hotkey_id id, bool pressed)
{
	UNUSED_PARAMETER(data);
	log_event(100 + (long)id, pressed);
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	hotkey_test_obs = bzalloc(sizeof(*hotkey_test_obs));
	assert_int_equal(pthread_mutex_init_recursive(&hotkey_test_obs->hotkeys.mutex), 0);
	hotkey_test_obs->hotkeys.strict_modifiers = true;
	hotkey_test_obs->hotkeys.signals = signal_handler_create();
	static const char *signals[] = {"void hotkey_register(ptr key)", "void hotkey_unregister(ptr key)",
					"void hotkey_bindings_changed(ptr key)", NULL};
	assert_true(signal_handler_add_array(hotkey_test_obs->hotkeys.signals, signals));
	memset(held, 0, sizeof(held));
	num_queried = 0;
	num_events = 0;
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkeys_free();
	signal_handler_destroy(hotkey_test_obs->hotkeys.signals);
	pthread_mutex_destroy(&hotkey_test_obs->hotkeys.mutex);
	bfree(hotkey_test_obs);
	hotkey_test_obs = NULL;
	return 0;
}

/* ------------------------------------------------------------------------- */

#define CTRL INTERACT_CONTROL_KEY
#define SHIFT INTERACT_SHIFT_KEY
#define ALT INTERACT_ALT_KEY
#define CMD INTERACT_COMMAND_KEY

static obs_hotkey_id register_tagged(long tag)
{
	obs_hotkey_id id = obs_hotkey_register_frontend("test", "Test", hotkey_callback, (void *)(intptr_t)tag);
	assert_int_not_equal(id, OBS_INVALID_HOTKEY_ID);
	return id;
}

static void bind(obs_hotkey_id id, uint32_t modifiers, obs_key_t key)
{
	obs_key_combination_t combo = {modifiers, key};
	obs_hotkey_load_bindings(id, &combo, 1);
}

static obs_hotkey_binding_t *binding_at(size_t idx)
{
	assert_true(idx < hotkey_test_obs->hotkeys.bindings.num);
	return &hotkey_test_obs->hotkeys.bindings.array[idx];
}

/* Hold exactly these keys (OBS_KEY_NONE ends the list). */
static void hold(obs_key_t key, ...)
{
	memset(held, 0, sizeof(held));
	va_list args;
	va_start(args, key);
	for (; key != OBS_KEY_NONE; key = (obs_key_t)va_arg(args, int))
		held[key] = true;
	va_end(args);
}

/* One poll of the hotkey thread: query_hotkeys(). */
static void poll_bindings(bool no_press, bool strict)
{
	uint32_t modifiers = obs_hotkeys_query_modifiers();
	for (size_t i = 0; i < hotkey_test_obs->hotkeys.bindings.num; i++)
		obs_hotkey_binding_query(binding_at(i), modifiers, no_press, strict);
}

static void poll_strict(void)
{
	poll_bindings(false, true);
}

/* obs_hotkey_inject_event() */
static void inject(uint32_t modifiers, obs_key_t key, bool pressed, bool strict)
{
	obs_key_combination_t combo = {modifiers, key};
	for (size_t i = 0; i < hotkey_test_obs->hotkeys.bindings.num; i++)
		obs_hotkey_binding_inject(binding_at(i), combo, pressed, strict);
}

static void expect_binding(size_t idx, bool pressed, bool modifiers_match)
{
	obs_hotkey_binding_t *binding = binding_at(idx);
	if (binding->pressed != pressed || binding->modifiers_match != modifiers_match)
		fprintf(stderr, "binding %zu: pressed %d, modifiers_match %d; expected %d, %d\n", idx, binding->pressed,
			binding->modifiers_match, pressed, modifiers_match);
	assert_int_equal(binding->pressed, pressed);
	assert_int_equal(binding->modifiers_match, modifiers_match);
}

static void expect_events(const struct event *expected, size_t count)
{
	bool same = num_events == count;
	for (size_t i = 0; same && i < count; i++)
		same = events[i].tag == expected[i].tag && events[i].pressed == expected[i].pressed;
	if (!same) {
		fprintf(stderr, "callbacks:");
		for (size_t i = 0; i < num_events && i < MAX_LOG; i++)
			fprintf(stderr, " %ld%s", events[i].tag, events[i].pressed ? "+" : "-");
		fprintf(stderr, "\nexpected: ");
		for (size_t i = 0; i < count; i++)
			fprintf(stderr, " %ld%s", expected[i].tag, expected[i].pressed ? "+" : "-");
		fprintf(stderr, "\n");
	}
	num_events = 0;
	assert_true(same);
}

#define EXPECT(...)                                                                 \
	do {                                                                        \
		const struct event expected_[] = {__VA_ARGS__};                     \
		expect_events(expected_, sizeof(expected_) / sizeof(expected_[0])); \
	} while (false)
#define EXPECT_NONE() expect_events(NULL, 0)
#define P(tag) {(tag), true}
#define R(tag) {(tag), false}

/* The keys queried since the last check are exactly `expected`. */
static void expect_queried(const obs_key_t *expected, size_t count)
{
	bool same = num_queried == count;
	for (size_t i = 0; same && i < count; i++)
		same = queried[i] == expected[i];
	if (!same) {
		fprintf(stderr, "queried:");
		for (size_t i = 0; i < num_queried && i < MAX_LOG; i++)
			fprintf(stderr, " %s", obs_key_to_name(queried[i]));
		fprintf(stderr, "\nexpected:");
		for (size_t i = 0; i < count; i++)
			fprintf(stderr, " %s", obs_key_to_name(expected[i]));
		fprintf(stderr, "\n");
	}
	num_queried = 0;
	assert_true(same);
}

#define EXPECT_QUERIED(...)                                                          \
	do {                                                                         \
		const obs_key_t expected_[] = {__VA_ARGS__};                         \
		expect_queried(expected_, sizeof(expected_) / sizeof(expected_[0])); \
	} while (false)
#define EXPECT_NOT_QUERIED() expect_queried(NULL, 0)
#define MODIFIER_QUERIES OBS_KEY_SHIFT, OBS_KEY_CONTROL, OBS_KEY_ALT, OBS_KEY_META

/* ------------------------------------------------------------------------- */

/* obs_hotkeys_query_modifiers() queries Shift, Control, Alt and Meta, once
 * each and in that order, and maps Meta to the command bit. */
static void query_modifiers(void **state)
{
	UNUSED_PARAMETER(state);
	assert_int_equal(obs_hotkeys_query_modifiers(), 0);
	EXPECT_QUERIED(MODIFIER_QUERIES);

	hold(OBS_KEY_SHIFT, OBS_KEY_ALT, OBS_KEY_A, OBS_KEY_NONE);
	assert_int_equal(obs_hotkeys_query_modifiers(), SHIFT | ALT);
	EXPECT_QUERIED(MODIFIER_QUERIES);

	hold(OBS_KEY_CONTROL, OBS_KEY_META, OBS_KEY_NONE);
	assert_int_equal(obs_hotkeys_query_modifiers(), CTRL | CMD);
	EXPECT_QUERIED(MODIFIER_QUERIES);
}

/* Strict Ctrl+A, poll by poll. A new binding's latch is false. A poll whose
 * modifiers do not match resets the latch to false (and releases); a poll
 * whose modifiers match while the latch is false only sets it. The binding
 * is pressed on a later poll with matching modifiers and A held. */
static void latch_steps(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id a = register_tagged(1);
	bind(a, CTRL, OBS_KEY_A);
	expect_binding(0, false, false);

	hold(OBS_KEY_CONTROL, OBS_KEY_A, OBS_KEY_NONE);
	poll_strict();
	expect_binding(0, false, true);
	EXPECT_NONE();
	poll_strict();
	expect_binding(0, true, true);
	EXPECT(P(1));
	poll_strict();
	expect_binding(0, true, true);
	EXPECT_NONE();

	/* Ctrl released: released, latch false. */
	hold(OBS_KEY_A, OBS_KEY_NONE);
	poll_strict();
	expect_binding(0, false, false);
	EXPECT(R(1));

	/* Ctrl again: one poll to set the latch, the next presses. */
	hold(OBS_KEY_CONTROL, OBS_KEY_A, OBS_KEY_NONE);
	poll_strict();
	expect_binding(0, false, true);
	EXPECT_NONE();
	poll_strict();
	expect_binding(0, true, true);
	EXPECT(P(1));

	/* A released, Ctrl still held: released, latch stays true, so A
	 * again presses on the next poll. */
	hold(OBS_KEY_CONTROL, OBS_KEY_NONE);
	poll_strict();
	expect_binding(0, false, true);
	EXPECT(R(1));
	hold(OBS_KEY_CONTROL, OBS_KEY_A, OBS_KEY_NONE);
	poll_strict();
	expect_binding(0, true, true);
	EXPECT(P(1));
}

/* A strict binding without modifiers, held from the start, is pressed on the
 * second poll: the first only sets the latch. */
static void new_binding_second_poll(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id a = register_tagged(1);
	bind(a, 0, OBS_KEY_A);
	hold(OBS_KEY_A, OBS_KEY_NONE);

	poll_strict();
	expect_binding(0, false, true);
	EXPECT_NONE();
	poll_strict();
	expect_binding(0, true, true);
	EXPECT(P(1));
}

/* The binding's key is queried only after its modifiers and latch match,
 * and before no_press is looked at. A modifier-only and an empty binding
 * never query their key. */
static void key_state_is_lazy(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id a = register_tagged(1);
	bind(a, CTRL, OBS_KEY_A);
	num_queried = 0;

	/* Modifiers differ: no key query. */
	poll_strict();
	EXPECT_QUERIED(MODIFIER_QUERIES);

	/* Modifiers match, latch false: no key query. */
	hold(OBS_KEY_CONTROL, OBS_KEY_NONE);
	poll_strict();
	EXPECT_QUERIED(MODIFIER_QUERIES);

	/* Latch true: one query of A per poll, pressed or not, with presses
	 * on or off. */
	poll_strict();
	EXPECT_QUERIED(MODIFIER_QUERIES, OBS_KEY_A);
	hold(OBS_KEY_CONTROL, OBS_KEY_A, OBS_KEY_NONE);
	poll_bindings(true, true);
	EXPECT_QUERIED(MODIFIER_QUERIES, OBS_KEY_A);
	poll_strict();
	EXPECT_QUERIED(MODIFIER_QUERIES, OBS_KEY_A);
	EXPECT(P(1));
	poll_strict();
	EXPECT_QUERIED(MODIFIER_QUERIES, OBS_KEY_A);
	EXPECT_NONE();

	/* A modifier-only and an empty binding: no key query, in either
	 * branch. */
	obs_hotkey_id m = register_tagged(2);
	obs_hotkey_id e = register_tagged(3);
	bind(a, 0, OBS_KEY_NONE);
	EXPECT(R(1));
	bind(m, CTRL, OBS_KEY_NONE);
	bind(e, 0, OBS_KEY_NONE);
	num_queried = 0;
	for (int i = 0; i < 3; i++) {
		poll_strict();
		EXPECT_QUERIED(MODIFIER_QUERIES);
		poll_bindings(false, false);
		EXPECT_QUERIED(MODIFIER_QUERIES);
	}
	EXPECT(P(2));

	/* Injecting queries nothing. */
	inject(CTRL, OBS_KEY_A, true, true);
	inject(0, OBS_KEY_A, false, false);
	EXPECT_NOT_QUERIED();
	EXPECT(P(1), P(3));
}

/* A modifier-only binding skips the latch and the key query: it is pressed
 * on the first poll with exactly its modifiers held (strict), and released
 * when they change. A poll never sets its latch (only an inject does). */
static void modifier_only_steps(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id a = register_tagged(1);
	bind(a, CTRL | SHIFT, OBS_KEY_NONE);

	hold(OBS_KEY_CONTROL, OBS_KEY_SHIFT, OBS_KEY_NONE);
	poll_strict();
	expect_binding(0, true, false);
	EXPECT(P(1));

	hold(OBS_KEY_CONTROL, OBS_KEY_SHIFT, OBS_KEY_ALT, OBS_KEY_NONE);
	poll_strict();
	expect_binding(0, false, false);
	EXPECT(R(1));

	hold(OBS_KEY_CONTROL, OBS_KEY_SHIFT, OBS_KEY_NONE);
	poll_strict();
	expect_binding(0, true, false);
	EXPECT(P(1));
}

/* An empty binding (no key, no modifiers) is never pressed by a poll; it
 * only resets its latch. Injected events press it: every one without
 * modifiers when strict, every one at all when not. Characterized, not
 * endorsed. */
static void empty_binding(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id a = register_tagged(1);
	bind(a, 0, OBS_KEY_NONE);

	hold(OBS_KEY_A, OBS_KEY_NONE);
	poll_strict();
	poll_bindings(false, false);
	expect_binding(0, false, true);
	EXPECT_NONE();

	inject(SHIFT, OBS_KEY_A, true, true);
	EXPECT_NONE();
	inject(0, OBS_KEY_B, false, true);
	expect_binding(0, true, true);
	EXPECT(P(1));
	poll_strict();
	expect_binding(0, false, true);
	EXPECT(R(1));

	inject(CTRL | ALT, OBS_KEY_NONE, false, false);
	EXPECT(P(1));
	poll_bindings(false, false);
	EXPECT(R(1));
}

/* Not strict: a binding's modifiers must all be held, others may be too. A
 * binding without modifiers has its latch set on every poll, so it is
 * pressed on the first poll. */
static void non_strict(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id a = register_tagged(1);
	obs_hotkey_id b = register_tagged(2);
	obs_hotkey_id m = register_tagged(3);
	bind(a, 0, OBS_KEY_A);
	bind(b, CTRL, OBS_KEY_B);
	bind(m, ALT, OBS_KEY_NONE);

	hold(OBS_KEY_CONTROL, OBS_KEY_SHIFT, OBS_KEY_A, OBS_KEY_NONE);
	poll_bindings(false, false);
	expect_binding(0, true, true);
	expect_binding(1, false, true);
	expect_binding(2, false, false);
	EXPECT(P(1));

	hold(OBS_KEY_CONTROL, OBS_KEY_SHIFT, OBS_KEY_ALT, OBS_KEY_A, OBS_KEY_B, OBS_KEY_NONE);
	poll_bindings(false, false);
	expect_binding(1, true, true);
	expect_binding(2, true, false);
	EXPECT(P(2), P(3));

	hold(OBS_KEY_SHIFT, OBS_KEY_ALT, OBS_KEY_B, OBS_KEY_NONE);
	poll_bindings(false, false);
	expect_binding(0, false, true);
	expect_binding(1, false, false);
	expect_binding(2, true, false);
	EXPECT(R(1), R(2));

	/* Injected, not strict: a superset of the modifiers matches. */
	inject(CTRL | SHIFT, OBS_KEY_B, true, false);
	expect_binding(1, true, true);
	EXPECT(P(2));
}

/* Presses off (no_press): nothing is pressed and the latch is not touched
 * while the checks pass; a pressed binding stays pressed while held and is
 * released when its key is. */
static void no_press_steps(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id a = register_tagged(1);
	bind(a, 0, OBS_KEY_A);

	hold(OBS_KEY_A, OBS_KEY_NONE);
	poll_bindings(true, true);
	poll_bindings(true, true);
	expect_binding(0, false, true);
	EXPECT_NONE();

	poll_strict();
	expect_binding(0, true, true);
	EXPECT(P(1));
	poll_bindings(true, true);
	expect_binding(0, true, true);
	EXPECT_NONE();

	hold(OBS_KEY_NONE);
	poll_bindings(true, true);
	expect_binding(0, false, true);
	EXPECT(R(1));
}

/* An inject that presses sets the latch even where the binding is already
 * pressed; one that does not press leaves the binding alone. Presses off
 * (thread_disable_press) do not apply to it. */
static void inject_steps(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id a = register_tagged(1);
	bind(a, CTRL, OBS_KEY_A);

	inject(CTRL, OBS_KEY_A, false, true);
	inject(CTRL, OBS_KEY_B, true, true);
	inject(0, OBS_KEY_A, true, true);
	expect_binding(0, false, false);
	EXPECT_NONE();

	hotkey_test_obs->hotkeys.thread_disable_press = true;
	inject(CTRL, OBS_KEY_A, true, true);
	expect_binding(0, true, true);
	EXPECT(P(1));

	/* Held: the poll keeps it pressed; an injected release changes
	 * nothing. */
	hold(OBS_KEY_CONTROL, OBS_KEY_A, OBS_KEY_NONE);
	poll_strict();
	inject(CTRL, OBS_KEY_A, false, true);
	expect_binding(0, true, true);
	EXPECT_NONE();

	/* Ctrl released: the poll releases and clears the latch; an injected
	 * press sets it again. */
	hold(OBS_KEY_A, OBS_KEY_NONE);
	poll_strict();
	expect_binding(0, false, false);
	EXPECT(R(1));
	inject(CTRL, OBS_KEY_A, true, true);
	expect_binding(0, true, true);
	EXPECT(P(1));
	inject(CTRL, OBS_KEY_A, true, true);
	EXPECT_NONE();
}

/* The hotkey's pressed count over two bindings: the callback runs on the
 * first press and the last release. Rerouted, the router gets them; with no
 * router nothing is called, but the count still moves. */
static void pressed_count_and_routing(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id a = register_tagged(1);
	obs_key_combination_t combos[] = {{0, OBS_KEY_A}, {0, OBS_KEY_MOUSE3}};
	obs_hotkey_load_bindings(a, combos, 2);
	obs_hotkey_t *hotkey = binding_at(0)->hotkey;
	assert_ptr_equal(binding_at(1)->hotkey, hotkey);

	inject(0, OBS_KEY_A, true, true);
	assert_int_equal(hotkey->pressed, 1);
	EXPECT(P(1));
	inject(0, OBS_KEY_MOUSE3, true, true);
	assert_int_equal(hotkey->pressed, 2);
	EXPECT_NONE();

	hold(OBS_KEY_MOUSE3, OBS_KEY_NONE);
	poll_strict();
	assert_int_equal(hotkey->pressed, 1);
	expect_binding(0, false, true);
	expect_binding(1, true, true);
	EXPECT_NONE();
	hold(OBS_KEY_NONE);
	poll_strict();
	assert_int_equal(hotkey->pressed, 0);
	EXPECT(R(1));

	hotkey_test_obs->hotkeys.reroute_hotkeys = true;
	hotkey_test_obs->hotkeys.router_func = router;
	inject(0, OBS_KEY_A, true, true);
	poll_strict();
	EXPECT(P(100 + (long)a), R(100 + (long)a));

	hotkey_test_obs->hotkeys.router_func = NULL;
	inject(0, OBS_KEY_A, true, true);
	assert_int_equal(hotkey->pressed, 1);
	EXPECT_NONE();
	hotkey_test_obs->hotkeys.router_func = router;
	poll_strict();
	assert_int_equal(hotkey->pressed, 0);
	EXPECT(R(100 + (long)a));
}

/* obs_hotkey_binding_fields() points into the binding itself: at the
 * combination it matches, its pressed flag and its modifiers_match latch. So
 * what the state machine reads and writes through it is the binding's own
 * state, and a press or release (in obs-hotkey.c) shows through it. */
static void binding_fields(void **state)
{
	UNUSED_PARAMETER(state);
	obs_hotkey_id a = register_tagged(1);
	obs_key_combination_t combos[] = {{CTRL, OBS_KEY_A}, {0, OBS_KEY_MOUSE3}};
	obs_hotkey_load_bindings(a, combos, 2);

	for (size_t i = 0; i < 2; i++) {
		obs_hotkey_binding_t *binding = binding_at(i);
		struct obs_hotkey_binding_fields fields = obs_hotkey_binding_fields(binding);
		assert_ptr_equal(fields.key, &binding->key);
		assert_ptr_equal(fields.pressed, &binding->pressed);
		assert_ptr_equal(fields.modifiers_match, &binding->modifiers_match);
		assert_int_equal(fields.key->modifiers, combos[i].modifiers);
		assert_int_equal(fields.key->key, combos[i].key);
		assert_false(*fields.pressed);
		assert_false(*fields.modifiers_match);
	}

	/* A latch set through the accessor is the one a poll reads: Ctrl+A,
	 * held, is pressed on the first poll instead of the second. */
	struct obs_hotkey_binding_fields fields = obs_hotkey_binding_fields(binding_at(0));
	*fields.modifiers_match = true;
	hold(OBS_KEY_CONTROL, OBS_KEY_A, OBS_KEY_NONE);
	poll_strict();
	assert_true(*fields.pressed);
	assert_true(*fields.modifiers_match);
	EXPECT(P(1));

	/* A released, Ctrl held: the release clears the pressed flag it
	 * points at and keeps the latch. */
	hold(OBS_KEY_CONTROL, OBS_KEY_NONE);
	poll_strict();
	assert_false(*fields.pressed);
	assert_true(*fields.modifiers_match);
	EXPECT(R(1));

	/* An inject presses the second binding through the same fields. */
	struct obs_hotkey_binding_fields mouse = obs_hotkey_binding_fields(binding_at(1));
	inject(0, OBS_KEY_MOUSE3, true, true);
	assert_true(*mouse.pressed);
	assert_true(*mouse.modifiers_match);
	EXPECT(P(1));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(query_modifiers, setup, teardown),
		cmocka_unit_test_setup_teardown(latch_steps, setup, teardown),
		cmocka_unit_test_setup_teardown(new_binding_second_poll, setup, teardown),
		cmocka_unit_test_setup_teardown(key_state_is_lazy, setup, teardown),
		cmocka_unit_test_setup_teardown(modifier_only_steps, setup, teardown),
		cmocka_unit_test_setup_teardown(empty_binding, setup, teardown),
		cmocka_unit_test_setup_teardown(non_strict, setup, teardown),
		cmocka_unit_test_setup_teardown(no_press_steps, setup, teardown),
		cmocka_unit_test_setup_teardown(inject_steps, setup, teardown),
		cmocka_unit_test_setup_teardown(pressed_count_and_routing, setup, teardown),
		cmocka_unit_test_setup_teardown(binding_fields, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
