#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>

#include <callback/calldata.h>
#include <callback/signal.h>
#include <util/base.h>
#include <util/bmem.h>

/* Characterization tests: the expected values are what
 * libobs/callback/signal.c.h produces today (#1441). Cases marked
 * "characterized, not endorsed" pin odd behavior of that C.
 *
 * Superseded characterizations (split from #1464, approved by the
 * maintainer as in #1508): signal_handler_connect_global and
 * disconnect_global used to find an entry with da_find, a memcmp of the
 * whole struct global_callback_info (padding and signaling count
 * included), so a global callback could not disconnect itself; and a
 * nested signal reset the thread-local current callback, so
 * signal_handler_remove_current() after it removed nothing. Both are now
 * fixed and the tests below expect the fixed behavior. */

struct log_record {
	int calls;
	int level;
	char message[160];
};

#define MAX_SEEN 16

struct seen {
	int count;
	int ids[MAX_SEEN];
	calldata_t *params[MAX_SEEN];
	char signal[MAX_SEEN][32];
};

struct cb_ctx {
	int id;
	struct seen *seen;
	signal_handler_t *handler;
	void *other;
};

static void capture_log(int level, const char *format, va_list args, void *param)
{
	struct log_record *rec = param;

	rec->calls++;
	rec->level = level;
	vsnprintf(rec->message, sizeof(rec->message), format, args);
}

static void record(struct cb_ctx *ctx, const char *signal, calldata_t *params)
{
	struct seen *seen = ctx->seen;

	assert_true(seen->count < MAX_SEEN);
	seen->ids[seen->count] = ctx->id;
	seen->params[seen->count] = params;
	snprintf(seen->signal[seen->count], sizeof(seen->signal[0]), "%s", signal ? signal : "");
	seen->count++;
}

static void on_signal(void *data, calldata_t *params)
{
	record(data, NULL, params);
}

static void on_global(void *data, const char *signal, calldata_t *params)
{
	record(data, signal, params);
}

static void remove_self(void *data, calldata_t *params)
{
	record(data, NULL, params);
	signal_handler_remove_current();
}

static void remove_self_global(void *data, const char *signal, calldata_t *params)
{
	record(data, signal, params);
	signal_handler_remove_current();
}

static void disconnect_other(void *data, calldata_t *params)
{
	struct cb_ctx *ctx = data;

	record(ctx, NULL, params);
	signal_handler_disconnect(ctx->handler, "ev", on_signal, ctx->other);
}

static void connect_other(void *data, calldata_t *params)
{
	struct cb_ctx *ctx = data;

	record(ctx, NULL, params);
	signal_handler_connect(ctx->handler, "ev", on_signal, ctx->other);
}

static void nested_then_remove(void *data, calldata_t *params)
{
	struct cb_ctx *ctx = data;

	record(ctx, NULL, params);
	signal_handler_signal(ctx->handler, "inner", params);
	/* The nested signal restores the current callback when it returns,
	 * so this removes this callback. (Supersedes the old
	 * characterization: the nested signal reset it to NULL and this
	 * removed nothing.) */
	signal_handler_remove_current();
}

static int count_calls;

static void count_only(void *data, calldata_t *params)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(params);
	count_calls++;
}

#define MANY_CONNECTS 64
static int many_data[MANY_CONNECTS];

/* Connects enough callbacks to reallocate the callback array, then
 * removes itself: the removal must still reach its own entry. */
static void connect_many_then_remove(void *data, calldata_t *params)
{
	struct cb_ctx *ctx = data;

	record(ctx, NULL, params);
	if (ctx->seen->count == 1) {
		for (int i = 0; i < MANY_CONNECTS; i++)
			signal_handler_connect(ctx->handler, "ev", count_only, &many_data[i]);
	}
	signal_handler_remove_current();
}

/* Signals "ev" again from inside "ev" once. */
static void recurse_once(void *data, calldata_t *params)
{
	struct cb_ctx *ctx = data;
	int *depth = ctx->other;

	record(ctx, NULL, params);
	if (*depth == 0) {
		(*depth)++;
		signal_handler_signal(ctx->handler, "ev", params);
		(*depth)--;
	}
}

static void disconnect_own_global(void *data, const char *signal, calldata_t *params)
{
	struct cb_ctx *ctx = data;

	record(ctx, signal, params);
	signal_handler_disconnect_global(ctx->handler, disconnect_own_global, ctx);
}

static void count_only_global(void *data, const char *signal, calldata_t *params)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(signal);
	UNUSED_PARAMETER(params);
	count_calls++;
}

/* Connects enough global callbacks to reallocate the global callback
 * array, then removes itself: the removal must still reach its own
 * entry. */
static void connect_many_globals_then_remove(void *data, const char *signal, calldata_t *params)
{
	struct cb_ctx *ctx = data;

	record(ctx, signal, params);
	if (ctx->seen->count == 1) {
		for (int i = 0; i < MANY_CONNECTS; i++)
			signal_handler_connect_global(ctx->handler, count_only_global, &many_data[i]);
	}
	signal_handler_remove_current();
}

/* Signals "ev" again from inside a global callback once, then removes
 * itself. */
static void nested_once_then_remove_global(void *data, const char *signal, calldata_t *params)
{
	struct cb_ctx *ctx = data;
	int *depth = ctx->other;

	record(ctx, signal, params);
	if (*depth == 0) {
		(*depth)++;
		signal_handler_signal(ctx->handler, "ev", params);
		(*depth)--;
	}
	signal_handler_remove_current();
}

/* Disconnects the on_global callback with data ctx->other, then removes
 * itself. */
static void disconnect_other_global_then_remove(void *data, const char *signal, calldata_t *params)
{
	struct cb_ctx *ctx = data;

	record(ctx, signal, params);
	signal_handler_disconnect_global(ctx->handler, on_global, ctx->other);
	signal_handler_remove_current();
}

static signal_handler_t *handler_with(const char *decl)
{
	signal_handler_t *handler = signal_handler_create();

	assert_non_null(handler);
	assert_true(signal_handler_add(handler, decl));
	return handler;
}

static void create_destroy_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	signal_handler_t *handler = signal_handler_create();

	assert_non_null(handler);
	signal_handler_destroy(handler);
	signal_handler_destroy(NULL);
	assert_int_equal(bnum_allocs(), allocs);
}

static void add_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct log_record rec = {0};
	log_handler_t prev_handler;
	void *prev_param;
	signal_handler_t *handler = signal_handler_create();

	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(capture_log, &rec);

	assert_true(signal_handler_add(handler, "void ev(int n)"));
	assert_int_equal(rec.calls, 0);

	/* A second declaration of the same name is refused. */
	assert_false(signal_handler_add(handler, "void ev(string s)"));
	assert_int_equal(rec.calls, 1);
	assert_int_equal(rec.level, LOG_WARNING);
	assert_string_equal(rec.message, "Signal declaration 'ev' exists");

	/* An invalid declaration: decl logs its warning, then signal logs. */
	rec.calls = 0;
	assert_false(signal_handler_add(handler, "void (int n)"));
	assert_int_equal(rec.calls, 2);
	assert_int_equal(rec.level, LOG_ERROR);
	assert_string_equal(rec.message, "Signal declaration invalid: void (int n)");

	/* The header inline adds every string and returns false if any one
	 * failed; the strings after a failure are still added. */
	const char *decls[] = {"void a()", "void b()", "void a()", "void c()", NULL};
	rec.calls = 0;
	assert_false(signal_handler_add_array(handler, decls));
	assert_int_equal(rec.calls, 1);
	assert_false(signal_handler_add(handler, "void c()"));
	assert_false(signal_handler_add_array(handler, NULL));

	base_set_log_handler(prev_handler, prev_param);
	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void connect_signal_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	struct cb_ctx a = {1, &seen, NULL, NULL};
	struct cb_ctx b = {2, &seen, NULL, NULL};
	calldata_t params = {0};
	signal_handler_t *handler = handler_with("void ev(int n)");

	signal_handler_connect(handler, "ev", on_signal, &a);
	signal_handler_connect(handler, "ev", on_signal, &b);
	/* The same callback and data again is not added twice. */
	signal_handler_connect(handler, "ev", on_signal, &a);

	calldata_set_int(&params, "n", 7);
	signal_handler_signal(handler, "ev", &params);

	/* In connect order, with the caller's calldata. */
	assert_int_equal(seen.count, 2);
	assert_int_equal(seen.ids[0], 1);
	assert_int_equal(seen.ids[1], 2);
	assert_ptr_equal(seen.params[0], &params);
	assert_ptr_equal(seen.params[1], &params);

	/* An unknown signal calls nothing. */
	signal_handler_signal(handler, "nope", &params);
	assert_int_equal(seen.count, 2);

	/* A null handler is ignored by connect, disconnect, and signal. */
	signal_handler_connect(NULL, "ev", on_signal, &a);
	signal_handler_disconnect(NULL, "ev", on_signal, &a);
	signal_handler_signal(NULL, "ev", &params);
	assert_int_equal(seen.count, 2);

	calldata_free(&params);
	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void connect_unknown_test(void **state)
{
	UNUSED_PARAMETER(state);

	struct log_record rec = {0};
	log_handler_t prev_handler;
	void *prev_param;
	struct seen seen = {0};
	struct cb_ctx a = {1, &seen, NULL, NULL};
	signal_handler_t *handler = handler_with("void ev()");

	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(capture_log, &rec);
	signal_handler_connect(handler, "nope", on_signal, &a);
	base_set_log_handler(prev_handler, prev_param);

	assert_int_equal(rec.calls, 1);
	assert_int_equal(rec.level, LOG_WARNING);
	assert_string_equal(rec.message, "signal_handler_connect: signal 'nope' not found");

	signal_handler_destroy(handler);
}

static void disconnect_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	struct cb_ctx a = {1, &seen, NULL, NULL};
	struct cb_ctx b = {2, &seen, NULL, NULL};
	signal_handler_t *handler = handler_with("void ev()");

	signal_handler_connect(handler, "ev", on_signal, &a);
	signal_handler_connect(handler, "ev", on_signal, &b);
	signal_handler_disconnect(handler, "ev", on_signal, &a);
	/* Not connected, or an unknown signal: nothing happens. */
	signal_handler_disconnect(handler, "ev", on_signal, &a);
	signal_handler_disconnect(handler, "nope", on_signal, &b);

	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 1);
	assert_int_equal(seen.ids[0], 2);
	assert_null(seen.params[0]);

	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void disconnect_during_signal_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	signal_handler_t *handler = handler_with("void ev()");
	struct cb_ctx b = {2, &seen, NULL, NULL};
	struct cb_ctx a = {1, &seen, handler, &b};

	signal_handler_connect(handler, "ev", disconnect_other, &a);
	signal_handler_connect(handler, "ev", on_signal, &b);

	/* A disconnect while signalling only marks the callback; a marked
	 * callback later in the list is skipped in the same pass. */
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 1);
	assert_int_equal(seen.ids[0], 1);

	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 2);
	assert_int_equal(seen.ids[1], 1);

	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void remove_current_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	struct cb_ctx a = {1, &seen, NULL, NULL};
	struct cb_ctx b = {2, &seen, NULL, NULL};
	signal_handler_t *handler = handler_with("void ev()");

	/* Outside a signal there is no current callback. */
	signal_handler_remove_current();

	signal_handler_connect(handler, "ev", remove_self, &a);
	signal_handler_connect(handler, "ev", on_signal, &b);

	signal_handler_signal(handler, "ev", NULL);
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 3);
	assert_int_equal(seen.ids[0], 1);
	assert_int_equal(seen.ids[1], 2);
	assert_int_equal(seen.ids[2], 2);

	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void connect_during_signal_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	signal_handler_t *handler = handler_with("void ev()");
	struct cb_ctx b = {2, &seen, NULL, NULL};
	struct cb_ctx a = {1, &seen, handler, &b};

	signal_handler_connect(handler, "ev", connect_other, &a);

	/* characterized, not endorsed: a callback connected while signalling
	 * is called in the same pass (the loop rereads the count). */
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 2);
	assert_int_equal(seen.ids[0], 1);
	assert_int_equal(seen.ids[1], 2);

	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void nested_signal_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	signal_handler_t *handler = signal_handler_create();
	struct cb_ctx a = {1, &seen, handler, NULL};
	struct cb_ctx b = {2, &seen, NULL, NULL};

	assert_true(signal_handler_add(handler, "void ev()"));
	assert_true(signal_handler_add(handler, "void inner()"));
	signal_handler_connect(handler, "ev", nested_then_remove, &a);
	signal_handler_connect(handler, "inner", on_signal, &b);

	signal_handler_signal(handler, "ev", NULL);
	signal_handler_signal(handler, "ev", NULL);
	/* a removed itself after the nested signal: 1, 2, then nothing.
	 * (Supersedes the old characterization 1, 2, 1, 2.) */
	assert_int_equal(seen.count, 2);
	assert_int_equal(seen.ids[0], 1);
	assert_int_equal(seen.ids[1], 2);

	/* inner is still connected. */
	signal_handler_signal(handler, "inner", NULL);
	assert_int_equal(seen.count, 3);
	assert_int_equal(seen.ids[2], 2);

	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void keep_ref_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	struct cb_ctx a = {1, &seen, NULL, NULL};
	signal_handler_t *handler = handler_with("void ev()");

	/* With keep_ref the same pair is added again, and each holds a
	 * reference to the handler. */
	signal_handler_connect_ref(handler, "ev", on_signal, &a);
	signal_handler_connect_ref(handler, "ev", on_signal, &a);
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 2);

	/* The creator's reference goes; the two callback references keep the
	 * handler alive. */
	signal_handler_destroy(handler);
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 4);

	/* Each disconnect drops one reference (the first match each time).
	 * The last one destroys the handler. */
	signal_handler_disconnect(handler, "ev", on_signal, &a);
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 5);
	signal_handler_disconnect(handler, "ev", on_signal, &a);
	assert_int_equal(bnum_allocs(), allocs);
}

static void keep_ref_remove_current_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	struct cb_ctx a = {1, &seen, NULL, NULL};
	signal_handler_t *handler = handler_with("void ev()");

	/* A keep_ref callback removed while signalling gives its reference
	 * back after the pass; it does not destroy the handler there. */
	signal_handler_connect_ref(handler, "ev", remove_self, &a);
	signal_handler_signal(handler, "ev", NULL);
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 1);

	/* Only the creator's reference is left. */
	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void keep_ref_remove_current_last_ref_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	struct cb_ctx a = {1, &seen, NULL, NULL};
	signal_handler_t *handler = handler_with("void ev()");

	/* When the creator's reference is already gone, the reference a
	 * keep_ref callback gives back after removing itself is the last
	 * one, and the handler is destroyed then, not leaked. */
	signal_handler_connect_ref(handler, "ev", remove_self, &a);
	signal_handler_destroy(handler);
	assert_true(bnum_allocs() > allocs);
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 1);
	assert_int_equal(bnum_allocs(), allocs);
}

static void recursive_remove_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	int depth = 0;
	signal_handler_t *handler = handler_with("void ev()");
	struct cb_ctx r = {1, &seen, NULL, NULL};
	struct cb_ctx a = {2, &seen, handler, &depth};
	struct cb_ctx c = {3, &seen, NULL, NULL};

	signal_handler_connect(handler, "ev", remove_self, &r);
	signal_handler_connect(handler, "ev", recurse_once, &a);
	signal_handler_connect(handler, "ev", on_signal, &c);

	/* r removes itself in the outer pass; the inner pass must not erase
	 * it while the outer pass is still walking the list, or the outer
	 * pass skips c. Outer r, outer a, inner a, inner c, outer c. */
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(depth, 0);
	assert_int_equal(seen.count, 5);
	assert_int_equal(seen.ids[0], 1);
	assert_int_equal(seen.ids[1], 2);
	assert_int_equal(seen.ids[2], 2);
	assert_int_equal(seen.ids[3], 3);
	assert_int_equal(seen.ids[4], 3);

	/* After the outermost pass r is gone. */
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 9);
	assert_int_equal(seen.ids[5], 2);
	assert_int_equal(seen.ids[6], 2);
	assert_int_equal(seen.ids[7], 3);
	assert_int_equal(seen.ids[8], 3);

	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void connect_then_remove_current_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	signal_handler_t *handler = handler_with("void ev()");
	struct cb_ctx x = {1, &seen, handler, NULL};

	count_calls = 0;
	signal_handler_connect(handler, "ev", connect_many_then_remove, &x);

	/* x connects enough callbacks to move the callback array, then
	 * removes itself. The removal must mark x's entry, not the old
	 * array. The new callbacks run in the same pass (see
	 * connect_during_signal_test). */
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 1);
	assert_int_equal(count_calls, MANY_CONNECTS);

	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 1);
	assert_int_equal(count_calls, MANY_CONNECTS * 2);

	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void global_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	struct cb_ctx a = {1, &seen, NULL, NULL};
	struct cb_ctx g = {9, &seen, NULL, NULL};
	calldata_t params = {0};
	signal_handler_t *handler = handler_with("void ev()");

	signal_handler_connect(handler, "ev", on_signal, &a);
	signal_handler_connect_global(handler, on_global, &g);
	/* The same callback and data again is not added twice. */
	signal_handler_connect_global(handler, on_global, &g);
	/* A null handler or callback is ignored. */
	signal_handler_connect_global(NULL, on_global, &g);
	signal_handler_connect_global(handler, NULL, &g);

	/* Global callbacks run after the signal's own, with its name. */
	signal_handler_signal(handler, "ev", &params);
	assert_int_equal(seen.count, 2);
	assert_int_equal(seen.ids[0], 1);
	assert_int_equal(seen.ids[1], 9);
	assert_string_equal(seen.signal[1], "ev");
	assert_ptr_equal(seen.params[1], &params);

	/* characterized, not endorsed: an unknown signal returns before the
	 * global callbacks. */
	signal_handler_signal(handler, "nope", &params);
	assert_int_equal(seen.count, 2);

	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void global_remove_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	signal_handler_t *handler = handler_with("void ev()");
	struct cb_ctx r = {7, &seen, NULL, NULL};
	struct cb_ctx d = {8, &seen, handler, NULL};

	/* signal_handler_remove_current() in a global callback removes it. */
	signal_handler_connect_global(handler, remove_self_global, &r);
	/* A global callback that disconnects itself is found by callback
	 * and data and removed. (Supersedes the old characterization: da_find
	 * compared the whole entry, signaling count included, so it was not
	 * found and stayed connected.) */
	signal_handler_connect_global(handler, disconnect_own_global, &d);

	signal_handler_signal(handler, "ev", NULL);
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 2);
	assert_int_equal(seen.ids[0], 7);
	assert_int_equal(seen.ids[1], 8);

	/* Disconnecting outside a signal also finds the entry. */
	signal_handler_connect_global(handler, on_global, &r);
	signal_handler_disconnect_global(handler, on_global, &r);
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 2);

	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void global_connect_then_remove_current_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	signal_handler_t *handler = handler_with("void ev()");
	struct cb_ctx x = {1, &seen, handler, NULL};

	count_calls = 0;
	signal_handler_connect_global(handler, connect_many_globals_then_remove, &x);

	/* x connects enough global callbacks to move the global callback
	 * array, then removes itself. The removal must mark x's entry, not
	 * the old array. The new global callbacks run in the same pass. */
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 1);
	assert_int_equal(count_calls, MANY_CONNECTS);

	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 1);
	assert_int_equal(count_calls, MANY_CONNECTS * 2);

	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void global_nested_pass_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	int depth = 0;
	signal_handler_t *handler = handler_with("void ev()");
	struct cb_ctx a = {1, &seen, NULL, NULL};
	struct cb_ctx b = {2, &seen, handler, &depth};
	struct cb_ctx c = {3, &seen, NULL, NULL};

	signal_handler_connect_global(handler, remove_self_global, &a);
	signal_handler_connect_global(handler, nested_once_then_remove_global, &b);
	signal_handler_connect_global(handler, on_global, &c);

	/* a removes itself in the outer pass; the inner pass must not erase
	 * it while the outer pass is still walking the list, or b's removal
	 * marks c and the outer pass skips c. Outer a, outer b, inner b,
	 * inner c, outer c. */
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(depth, 0);
	assert_int_equal(seen.count, 5);
	assert_int_equal(seen.ids[0], 1);
	assert_int_equal(seen.ids[1], 2);
	assert_int_equal(seen.ids[2], 2);
	assert_int_equal(seen.ids[3], 3);
	assert_int_equal(seen.ids[4], 3);

	/* a and b are removed, c stays. */
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 6);
	assert_int_equal(seen.ids[5], 3);

	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

static void global_disconnect_during_signal_test(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct seen seen = {0};
	signal_handler_t *handler = handler_with("void ev()");
	struct cb_ctx y = {1, &seen, NULL, NULL};
	struct cb_ctx x = {2, &seen, handler, &y};
	struct cb_ctx z = {3, &seen, NULL, NULL};

	signal_handler_connect_global(handler, on_global, &y);
	signal_handler_connect_global(handler, disconnect_other_global_then_remove, &x);
	signal_handler_connect_global(handler, on_global, &z);

	/* x disconnects y, which already ran, then removes itself. y must
	 * not be erased while the pass is still walking the list, or x's
	 * removal marks z and the pass skips z. */
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 3);
	assert_int_equal(seen.ids[0], 1);
	assert_int_equal(seen.ids[1], 2);
	assert_int_equal(seen.ids[2], 3);

	/* y and x are removed, z stays. */
	signal_handler_signal(handler, "ev", NULL);
	assert_int_equal(seen.count, 4);
	assert_int_equal(seen.ids[3], 3);

	signal_handler_destroy(handler);
	assert_int_equal(bnum_allocs(), allocs);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(create_destroy_test),
		cmocka_unit_test(add_test),
		cmocka_unit_test(connect_signal_test),
		cmocka_unit_test(connect_unknown_test),
		cmocka_unit_test(disconnect_test),
		cmocka_unit_test(disconnect_during_signal_test),
		cmocka_unit_test(remove_current_test),
		cmocka_unit_test(connect_during_signal_test),
		cmocka_unit_test(nested_signal_test),
		cmocka_unit_test(keep_ref_test),
		cmocka_unit_test(keep_ref_remove_current_test),
		cmocka_unit_test(keep_ref_remove_current_last_ref_test),
		cmocka_unit_test(recursive_remove_test),
		cmocka_unit_test(connect_then_remove_current_test),
		cmocka_unit_test(global_test),
		cmocka_unit_test(global_remove_test),
		cmocka_unit_test(global_connect_then_remove_current_test),
		cmocka_unit_test(global_nested_pass_test),
		cmocka_unit_test(global_disconnect_during_signal_test),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
