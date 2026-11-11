/* Characterization of the exported obs_set_output_source against the actual
 * libobs (the testing policy's prerequisite for its Rust swap). It delegates
 * to obs_canvas_set_channel on the main canvas, so every case is observed
 * through both entry points. Cases:
 * - set/get/clear at the channel boundaries (0, 1, MAX_CHANNELS - 1);
 * - the release-build guard for invalid channels (the assert is compiled out,
 *   the rules check returns before touching anything);
 * - exact delegation: the canvas channel setter and the output-source entry
 *   point are interchangeable on the main canvas;
 * - old/new reference ownership: the setter's reference on the new source is
 *   owned by the channel; replacing or clearing releases the previous one,
 *   destroying it when the test held no reference of its own;
 * - the canvas-local and global channel_change routes (local runs first), and
 *   a handler replacing the source: the replacement is stored without a
 *   reference of its own (characterized, not endorsed), and the reference the
 *   setter took on the passed source is never released.
 * Callbacks are synchronous and use private counts; no sleeps.
 * No display or graphics device is needed: obs_startup() alone suffices.
 * Public API only, so it runs against libobs, the hybrid libobs and
 * libobs-c-reference. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <stdio.h>
#include <string.h>

#include <obs.h>

#define PLAIN_ID "test_set_output_source_plain"

static bool obs_up;

#define REQUIRE_OBS()                                                                   \
	do {                                                                            \
		if (!obs_up) {                                                          \
			fprintf(stderr, "skipped: obs_startup failed\n");               \
			skip();                                                         \
		}                                                                       \
	} while (false)

/* ------------------------------------------------------------------------ */
/* Fixture source type: a plain input the main canvas can hold                */

static const char *plain_name(void *data)
{
	(void)data;
	return "set output source plain";
}

static void *plain_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void plain_destroy(void *data)
{
	(void)data;
}

static uint32_t plain_size(void *data)
{
	(void)data;
	return 64;
}

static void register_sources(void)
{
	struct obs_source_info plain = {
		.id = PLAIN_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_REQUIRES_CANVAS,
		.get_name = plain_name,
		.create = plain_create,
		.destroy = plain_destroy,
		.get_width = plain_size,
		.get_height = plain_size,
	};
	obs_register_source(&plain);
}

static bool source_alive(obs_weak_source_t *weak)
{
	obs_source_t *s = obs_weak_source_get_source(weak);
	obs_source_release(s);
	return s != NULL;
}

/* ------------------------------------------------------------------------ */
/* Signal watchers: private counters, order logged through a shared tail      */

struct signal_watch {
	int calls;
	char tag;
	obs_canvas_t *canvas;
	int64_t channel;
	obs_source_t *prev_source;
	obs_source_t *source;
};

static char sig_seq[8];
static int sig_total;

static void watch_cb(void *data, calldata_t *cd)
{
	struct signal_watch *w = data;
	w->calls++;
	if (sig_total < (int)sizeof(sig_seq))
		sig_seq[sig_total] = w->tag;
	sig_total++;
	w->canvas = calldata_ptr(cd, "canvas");
	w->channel = calldata_int(cd, "channel");
	w->prev_source = calldata_ptr(cd, "prev_source");
	w->source = calldata_ptr(cd, "source");
}

static obs_source_t *replacement;
static int channel_changes;

static void replace_cb(void *data, calldata_t *cd)
{
	(void)data;
	channel_changes++;
	if (replacement)
		calldata_set_ptr(cd, "source", replacement);
}

/* ------------------------------------------------------------------------ */
/* Set, get and clear at the channel boundaries                              */

static void set_clear_boundaries(void **state)
{
	(void)state;
	REQUIRE_OBS();

	const uint32_t channels[] = {0, 1, MAX_CHANNELS - 1};
	for (size_t i = 0; i < sizeof(channels) / sizeof(channels[0]); i++) {
		uint32_t ch = channels[i];
		obs_source_t *s = obs_source_create_private(PLAIN_ID, "boundaries plain", NULL);
		assert_non_null(s);
		obs_set_output_source(ch, s);
		obs_source_t *got = obs_get_output_source(ch);
		assert_ptr_equal(got, s);
		obs_source_release(got);
		obs_set_output_source(ch, NULL);
		assert_null(obs_get_output_source(ch));
		/* the setter's reference was owned by the channel and released by
		 * the clear; this release drops the test's own */
		obs_source_release(s);
	}
}

/* ------------------------------------------------------------------------ */
/* The release-build guard for invalid channels                              */

static void release_guard(void **state)
{
	(void)state;
	REQUIRE_OBS();

#ifdef NDEBUG
	/* Debug C asserts; release returns before taking a reference, so an
	 * invalid channel changes nothing and the test still owns its source. */
	obs_source_t *s = obs_source_create_private(PLAIN_ID, "guard plain", NULL);
	assert_non_null(s);
	obs_set_output_source(MAX_CHANNELS, s);
	obs_set_output_source(UINT32_MAX, s);
	assert_null(obs_get_output_source(MAX_CHANNELS));
	assert_null(obs_get_output_source(UINT32_MAX));
	obs_source_release(s);
	obs_wait_for_destroy_queue();
#endif
}

/* ------------------------------------------------------------------------ */
/* Exact delegation: the entry point and the main canvas setter agree        */

static void delegation_equivalence(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *main = obs_get_main_canvas();
	assert_non_null(main);
	obs_source_t *a = obs_source_create_private(PLAIN_ID, "delegation a", NULL);
	obs_source_t *b = obs_source_create_private(PLAIN_ID, "delegation b", NULL);
	assert_non_null(a);
	assert_non_null(b);

	/* the canvas setter observed through the output-source getter */
	obs_canvas_set_channel(main, 3, a);
	obs_source_t *got = obs_get_output_source(3);
	assert_ptr_equal(got, a);
	obs_source_release(got);

	/* the entry point observed through the canvas getter */
	obs_set_output_source(3, b);
	obs_source_t *got2 = obs_canvas_get_channel(main, 3);
	assert_ptr_equal(got2, b);
	obs_source_release(got2);

	obs_set_output_source(3, NULL);
	obs_source_release(a);
	obs_source_release(b);
	obs_wait_for_destroy_queue();
	obs_canvas_release(main);
}

/* ------------------------------------------------------------------------ */
/* Reference ownership on the channel                                      */

static void reference_ownership(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_source_t *a = obs_source_create_private(PLAIN_ID, "ownership a", NULL);
	obs_source_t *b = obs_source_create_private(PLAIN_ID, "ownership b", NULL);
	assert_non_null(a);
	assert_non_null(b);
	obs_weak_source_t *weak_a = obs_source_get_weak_source(a);
	obs_weak_source_t *weak_b = obs_source_get_weak_source(b);

	/* the channel owns the setter's reference: a outlives the test's */
	obs_set_output_source(2, a);
	obs_source_release(a);
	assert_true(source_alive(weak_a));

	/* replacing releases the previous reference, destroying a (the test
	 * held no reference of its own) */
	obs_set_output_source(2, b);
	obs_wait_for_destroy_queue();
	assert_false(source_alive(weak_a));
	assert_true(source_alive(weak_b));

	/* clearing releases b's channel reference; the test's own remains */
	obs_set_output_source(2, NULL);
	obs_source_release(b);
	obs_wait_for_destroy_queue();
	assert_false(source_alive(weak_b));

	obs_weak_source_release(weak_a);
	obs_weak_source_release(weak_b);
}

/* ------------------------------------------------------------------------ */
/* Signal routes, ordering, parameters, and callback replacement            */

static void signal_routes_and_replace(void **state)
{
	(void)state;
	REQUIRE_OBS();

	obs_canvas_t *main = obs_get_main_canvas();
	assert_non_null(main);
	obs_source_t *a = obs_source_create_private(PLAIN_ID, "routes a", NULL);
	obs_source_t *b = obs_source_create_private(PLAIN_ID, "routes b", NULL);
	assert_non_null(a);
	assert_non_null(b);

	struct signal_watch local = {0}, global = {0};
	local.tag = 'c';
	global.tag = 'g';
	signal_handler_t *lsh = obs_canvas_get_signal_handler(main);
	signal_handler_connect(lsh, "channel_change", watch_cb, &local);
	signal_handler_connect(obs_get_signal_handler(), "channel_change", watch_cb, &global);

	/* with handlers: local fires before global, once each */
	obs_set_output_source(6, a);
	assert_int_equal(local.calls, 1);
	assert_int_equal(global.calls, 1);
	assert_true(sig_total >= 2);
	assert_true(sig_seq[0] == 'c' && sig_seq[1] == 'g');

	/* both routes carry the same parameters: the main canvas, the channel,
	 * no previous source and the new source */
	assert_ptr_equal(local.canvas, main);
	assert_ptr_equal(global.canvas, main);
	assert_int_equal(local.channel, 6);
	assert_int_equal(global.channel, 6);
	assert_null(local.prev_source);
	assert_ptr_equal(local.source, a);

	/* replacing while watching: the parameters now carry the previous
	 * source, and both counts rise again */
	obs_set_output_source(6, b);
	assert_int_equal(local.calls, 2);
	assert_int_equal(global.calls, 2);
	assert_ptr_equal(local.prev_source, a);
	assert_ptr_equal(local.source, b);

	signal_handler_disconnect(lsh, "channel_change", watch_cb, &local);
	signal_handler_disconnect(obs_get_signal_handler(), "channel_change", watch_cb, &global);
	obs_set_output_source(6, NULL);
	assert_int_equal(local.calls, 2);
	assert_int_equal(global.calls, 2);

	/* callback replacement (characterized, not endorsed): the handler
	 * swaps c for d, which the channel stores without a reference of its
	 * own, while the reference the setter took on c is never released */
	obs_source_t *c = obs_source_create_private(PLAIN_ID, "routes c", NULL);
	obs_source_t *d = obs_source_create_private(PLAIN_ID, "routes d", NULL);
	assert_non_null(c);
	assert_non_null(d);
	obs_weak_source_t *weak_c = obs_source_get_weak_source(c);
	obs_weak_source_t *weak_d = obs_source_get_weak_source(d);
	channel_changes = 0;
	signal_handler_connect(lsh, "channel_change", replace_cb, NULL);
	replacement = d;
	obs_set_output_source(6, c);
	replacement = NULL;
	assert_int_equal(channel_changes, 1);
	obs_source_t *got = obs_get_output_source(6);
	assert_ptr_equal(got, d);
	obs_source_release(got);
	assert_true(obs_source_active(d));
	assert_false(obs_source_active(c));
	obs_source_get_ref(d);
	obs_set_output_source(6, NULL);
	assert_false(obs_source_active(d));
	obs_source_release(d);
	obs_wait_for_destroy_queue();
	assert_false(source_alive(weak_d));
	obs_source_release(c);
	assert_true(source_alive(weak_c));
	/* drop the reference the setter kept on c, which the channel never
	 * stored and never released */
	obs_source_t *leaked = obs_weak_source_get_source(weak_c);
	obs_source_release(leaked);
	obs_source_release(leaked);
	obs_wait_for_destroy_queue();
	assert_false(source_alive(weak_c));
	signal_handler_disconnect(lsh, "channel_change", replace_cb, NULL);
	obs_weak_source_release(weak_c);
	obs_weak_source_release(weak_d);
	obs_source_release(a);
	obs_source_release(b);
	obs_canvas_release(main);
}

/* ------------------------------------------------------------------------ */

static int setup(void **state)
{
	(void)state;
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed\n");
		return 0;
	}
	register_sources();
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(set_clear_boundaries),
		cmocka_unit_test(release_guard),
		cmocka_unit_test(delegation_equivalence),
		cmocka_unit_test(reference_ownership),
		cmocka_unit_test(signal_routes_and_replace),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
