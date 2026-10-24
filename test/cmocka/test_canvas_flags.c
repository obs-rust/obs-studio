/* Canvas flags from callers and scene collections, and a canvas whose
 * context fails to initialize (#2141).
 *
 * obs_canvas_create(), obs_canvas_create_private() and obs_load_canvas()
 * take flags from a plugin, a script or a scene collection. Only ACTIVATE,
 * MIX_AUDIO, SCENE_REF and EPHEMERAL are public: MAIN belongs to the main
 * canvas, and libobs sets its internal REMOVED bit (1 << 31) in
 * obs_canvas_remove(). A canvas that kept a REMOVED bit from its caller
 * reported obs_canvas_removed() from the start, and obs_canvas_remove()
 * returned early, so the canvas never signalled its removal and its owner
 * never let it go. Unknown bits were kept and saved back.
 *
 * obs_canvas_create_internal() also leaked the canvas when
 * obs_context_data_init() failed. That needs a failing pthread_mutex_init(),
 * so the test interposes it for its own thread (the executable exports it,
 * see test-graph.json), and counts bmalloc allocations around the failed
 * create. With the C signal handler, its mutex is what fails the context
 * init. The hybrid libobs's Rust signal and proc handlers use no
 * pthread_mutex_init() and never fail, so there the create fails later, at
 * the canvas's own sources mutex, a path that always freed the canvas.
 *
 * Public API only, so it runs against libobs, the hybrid libobs and
 * libobs-c-reference. obs_startup() needs a display on Linux; without one
 * the cases skip. */

#define _GNU_SOURCE

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <dlfcn.h>
#include <errno.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/bmem.h>

#define PUBLIC_FLAGS ((uint32_t)(ACTIVATE | MIX_AUDIO | SCENE_REF | EPHEMERAL))
#define REMOVED_BIT (1u << 31)
#define UNKNOWN_BITS ((1u << 5) | (1u << 30))
#define LOADED_UUID "0b1d9a52-5c4e-4d3a-9a51-2141c0ffee01"

/* ------------------------------------------------------------------------ */
/* A pthread_mutex_init() that fails on demand, for this thread only         */

static __thread int fail_mutex_init;
static __thread int failed_mutex_inits;

__attribute__((visibility("default"))) int pthread_mutex_init(pthread_mutex_t *mutex, const pthread_mutexattr_t *attr)
{
	static int (*real)(pthread_mutex_t *, const pthread_mutexattr_t *);
	if (fail_mutex_init) {
		failed_mutex_inits++;
		return EAGAIN;
	}
	if (!real) {
		void *symbol = dlsym(RTLD_NEXT, "pthread_mutex_init");
		memcpy(&real, &symbol, sizeof(real));
	}
	return real(mutex, attr);
}

/* ------------------------------------------------------------------------ */
/* Removal: the owner lets the canvas go when it signals "remove"            */

struct owner {
	obs_canvas_t *canvas;
	int canvas_removes; /* the canvas's own "remove" */
	int global_removes; /* obs "canvas_remove", public canvases only */
};

static struct owner owner;

static void on_canvas_remove(void *data, calldata_t *cd)
{
	struct owner *o = data;
	if (calldata_ptr(cd, "canvas") != o->canvas)
		return;
	o->canvas_removes++;
	/* As the frontend does: drop the owning reference. */
	obs_canvas_release(o->canvas);
	o->canvas = NULL;
}

static void on_global_remove(void *data, calldata_t *cd)
{
	struct owner *o = data;
	if (o->canvas && calldata_ptr(cd, "canvas") == o->canvas)
		o->global_removes++;
}

static int setup(void **state)
{
	(void)state;
	owner = (struct owner){0};
	if (!obs_startup("en-US", NULL, NULL))
		return 0;
	signal_handler_connect(obs_get_signal_handler(), "canvas_remove", on_global_remove, &owner);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (!obs_initialized())
		return 0;
	signal_handler_disconnect(obs_get_signal_handler(), "canvas_remove", on_global_remove, &owner);
	if (owner.canvas) {
		signal_handler_disconnect(obs_canvas_get_signal_handler(owner.canvas), "remove", on_canvas_remove,
					  &owner);
		obs_canvas_release(owner.canvas);
		owner.canvas = NULL;
	}
	obs_shutdown();
	return 0;
}

#define REQUIRE_OBS()                                                              \
	do {                                                                       \
		if (!obs_initialized()) {                                          \
			fprintf(stderr, "skipped: obs_startup failed (no display?)\n"); \
			skip();                                                    \
		}                                                                  \
	} while (0)

static bool listed(obs_canvas_t *canvas, const char *uuid, const char *name, bool private)
{
	bool found = false;
	obs_canvas_t *by_uuid = obs_get_canvas_by_uuid(uuid);
	if (by_uuid) {
		found = by_uuid == canvas;
		obs_canvas_release(by_uuid);
	}
	if (!private) {
		obs_canvas_t *by_name = obs_get_canvas_by_name(name);
		if (by_name) {
			found = found || by_name == canvas;
			obs_canvas_release(by_name);
		}
	}
	return found;
}

static uint32_t saved_flags(obs_canvas_t *canvas)
{
	obs_data_t *data = obs_save_canvas(canvas);
	if (!data) {
		fprintf(stderr, "obs_save_canvas() wrote nothing\n");
		fail();
	}
	uint32_t flags = (uint32_t)obs_data_get_int(data, "flags");
	obs_data_release(data);
	return flags;
}

/* Checks a new canvas that the test owns: its flags, its saved flags, and
 * that obs_canvas_remove() signals its removal so its owner lets it go and
 * it leaves the canvas list. */
static void check_owned_canvas(obs_canvas_t *canvas, uint32_t expected, bool private)
{
	assert_non_null(canvas);
	owner.canvas = canvas;
	owner.canvas_removes = 0;
	owner.global_removes = 0;

	char uuid[64];
	char name[256];
	snprintf(uuid, sizeof(uuid), "%s", obs_canvas_get_uuid(canvas));
	snprintf(name, sizeof(name), "%s", obs_canvas_get_name(canvas));

	assert_int_equal(obs_canvas_get_flags(canvas), expected);
	assert_false(obs_canvas_removed(canvas));
	if (expected & EPHEMERAL) {
		obs_data_t *data = obs_save_canvas(canvas);
		assert_null(data);
	} else {
		assert_int_equal(saved_flags(canvas), expected);
	}
	assert_true(listed(canvas, uuid, name, private));

	obs_weak_canvas_t *weak = obs_canvas_get_weak_canvas(canvas);
	signal_handler_connect(obs_canvas_get_signal_handler(canvas), "remove", on_canvas_remove, &owner);

	obs_canvas_remove(canvas);

	assert_int_equal(owner.canvas_removes, 1);
	assert_int_equal(owner.global_removes, private ? 0 : 1);
	assert_null(owner.canvas);

	obs_canvas_t *alive = obs_weak_canvas_get_canvas(weak);
	assert_null(alive);
	obs_weak_canvas_release(weak);
	assert_false(listed(NULL, uuid, name, private));
}

/* ------------------------------------------------------------------------ */
/* Internal and unknown bits                                                 */

static obs_data_t *canvas_data(const char *name, long long flags, bool private)
{
	obs_data_t *data = obs_data_create();
	obs_data_set_string(data, "name", name);
	obs_data_set_string(data, "uuid", LOADED_UUID);
	obs_data_set_bool(data, "private", private);
	obs_data_set_int(data, "flags", flags);
	return data;
}

static void test_load_drops_internal_and_unknown_bits(void **state)
{
	(void)state;
	REQUIRE_OBS();

	const uint32_t flags = REMOVED_BIT | UNKNOWN_BITS | MAIN | ACTIVATE | SCENE_REF;
	obs_data_t *data = canvas_data("loaded", flags, false);
	obs_canvas_t *canvas = obs_load_canvas(data);
	obs_data_release(data);

	assert_string_equal(obs_canvas_get_uuid(canvas), LOADED_UUID);
	check_owned_canvas(canvas, ACTIVATE | SCENE_REF, false);
}

static void test_load_private_drops_internal_and_unknown_bits(void **state)
{
	(void)state;
	REQUIRE_OBS();

	const uint32_t flags = REMOVED_BIT | UNKNOWN_BITS | MAIN | MIX_AUDIO;
	obs_data_t *data = canvas_data("loaded private", flags, true);
	obs_canvas_t *canvas = obs_load_canvas(data);
	obs_data_release(data);

	check_owned_canvas(canvas, MIX_AUDIO, true);
}

/* A scene collection can hold any number, including a negative one. */
static void test_load_all_bits(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_data_t *data = canvas_data("all bits", -1, false);
	obs_canvas_t *canvas = obs_load_canvas(data);
	obs_data_release(data);

	/* EPHEMERAL is public, so this one is not saved. */
	check_owned_canvas(canvas, PUBLIC_FLAGS, false);
}

static void test_create_drops_internal_and_unknown_bits(void **state)
{
	(void)state;
	REQUIRE_OBS();

	const uint32_t flags = REMOVED_BIT | UNKNOWN_BITS | MAIN | MIX_AUDIO | SCENE_REF;
	check_owned_canvas(obs_canvas_create("created", NULL, flags), MIX_AUDIO | SCENE_REF, false);
}

static void test_create_private_drops_internal_and_unknown_bits(void **state)
{
	(void)state;
	REQUIRE_OBS();

	const uint32_t flags = REMOVED_BIT | UNKNOWN_BITS | MAIN | ACTIVATE;
	check_owned_canvas(obs_canvas_create_private("created private", NULL, flags), ACTIVATE, true);
}

/* ------------------------------------------------------------------------ */
/* Valid flags are unchanged                                                 */

static void test_valid_flags_unchanged(void **state)
{
	(void)state;
	REQUIRE_OBS();

	/* Every combination of the public flags; the presets are among them. */
	assert_int_equal(PROGRAM & ~PUBLIC_FLAGS, 0);
	assert_int_equal(PREVIEW & ~PUBLIC_FLAGS, 0);
	assert_int_equal(DEVICE & ~PUBLIC_FLAGS, 0);

	for (uint32_t flags = 0; flags <= PUBLIC_FLAGS; flags++) {
		if (flags & ~PUBLIC_FLAGS)
			continue;

		char name[64];
		snprintf(name, sizeof(name), "valid %u", flags);

		check_owned_canvas(obs_canvas_create(name, NULL, flags), flags, false);
		check_owned_canvas(obs_canvas_create_private(name, NULL, flags), flags, true);

		obs_data_t *data = canvas_data(name, flags, false);
		check_owned_canvas(obs_load_canvas(data), flags, false);
		obs_data_release(data);

		data = canvas_data(name, flags, true);
		check_owned_canvas(obs_load_canvas(data), flags, true);
		obs_data_release(data);

		/* MAIN alone is still dropped, as before. */
		check_owned_canvas(obs_canvas_create(name, NULL, flags | MAIN), flags, false);
	}

	/* The main canvas keeps MAIN and PROGRAM, and cannot be removed. */
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	assert_non_null(main_canvas);
	assert_int_equal(obs_canvas_get_flags(main_canvas), MAIN | PROGRAM);
	assert_int_equal(saved_flags(main_canvas), MAIN | PROGRAM);
	owner.global_removes = 0;
	obs_canvas_remove(main_canvas);
	assert_false(obs_canvas_removed(main_canvas));
	assert_int_equal(owner.global_removes, 0);
	obs_canvas_release(main_canvas);
}

/* ------------------------------------------------------------------------ */
/* Context init failure                                                      */

static void test_context_init_failure_frees_the_canvas(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_data_t *data = canvas_data("no context", PROGRAM, false);

	for (int i = 0; i < 3; i++) {
		long before = bnum_allocs();
		failed_mutex_inits = 0;
		fail_mutex_init = 1;
		obs_canvas_t *canvas = i == 0   ? obs_canvas_create("no context", NULL, PROGRAM)
				       : i == 1 ? obs_canvas_create_private("no context", NULL, PROGRAM)
						: obs_load_canvas(data);
		fail_mutex_init = 0;

		/* The interposed pthread_mutex_init() must be the one libobs called. */
		assert_true(failed_mutex_inits > 0);
		assert_null(canvas);
		assert_int_equal(bnum_allocs() - before, 0);
	}

	obs_data_release(data);
}

#define CASE(fn) cmocka_unit_test_setup_teardown(fn, setup, teardown)

int main(void)
{
	const struct CMUnitTest tests[] = {
		CASE(test_load_drops_internal_and_unknown_bits),
		CASE(test_load_private_drops_internal_and_unknown_bits),
		CASE(test_load_all_bits),
		CASE(test_create_drops_internal_and_unknown_bits),
		CASE(test_create_private_drops_internal_and_unknown_bits),
		CASE(test_valid_flags_unchanged),
		CASE(test_context_init_failure_frees_the_canvas),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
