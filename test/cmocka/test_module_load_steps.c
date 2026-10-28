/* Characterization of the module load rules of libobs/obs-module.c.h
 * (obs-rust/obs-studio#2801), moved to libobs/obs-module-load-rules.c before
 * their port to rust/obs-core (module_load_rules). It pins the CURRENT
 * behaviour; cases marked "characterized, not endorsed" are odd but real.
 *
 * Two parts:
 * - the steps: the hidden functions of obs-module-load-rules.c, compiled into
 *   this test from the moved file (with ASan and UBSan where the platform
 *   has them): the open gates, the required exports, the
 *   version rule, the safe and disabled list rules, the core and legacy
 *   types, the open and init routes, the failure count and the core load
 *   result. Then load_all_callback()'s whole decision, built on the steps the
 *   way obs-module.c.h calls them, for every combination of what it reads
 *   (a plugin or not, each list empty or not and the name on it or not, each
 *   module type, a core module's name or not, already loaded or not, every
 *   open code, the init result, a failure list or none), against the
 *   decisions read off the C before the move. That also pins which lookups
 *   run: a list's name match only when the list is not empty, the disabled
 *   list only for a module that is not core, the core list only for a legacy
 *   plugin, the module type after obs_open_module() only for a module that
 *   failed to open, nothing after the first skip;
 * - the callers: obs_open_module(), obs_get_module_allow_disable(),
 *   obs_is_core_module() and obs_is_legacy_module() of the real libobs,
 *   through the public API and the log handler. These run on libobs with the
 *   C backend, the hybrid libobs and libobs-c-reference.
 *
 * test_core_module_load (Linux) loads real stub modules through
 * obs_load_core_modules(): a core module that can't be opened, one built
 * for a newer libobs, and a retry after a failure.
 *
 * obs_startup() needs a display on Linux, so the test runs under xvfb-run
 * there. The caller cases that need the core skip only when no display is
 * set at all; with one set, a failed obs_startup() fails the test. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <limits.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <obs-module.h>
#include <obs-module-load-rules.h>
#include <util/base.h>

/* ================================================================ steps */

static void test_arg_missing(void **state)
{
	(void)state;
	assert_true(obs_module_load_rules_arg_missing(false));
	assert_false(obs_module_load_rules_arg_missing(true));

	/* obs_open_module()'s gate, asked as its || chain asks: the path only
	 * with a module pointer, the core only with both. */
	for (int bits = 0; bits < 8; bits++) {
		bool m = bits & 1, p = bits & 2, c = bits & 4;
		int asked = 0;
		bool missing = (asked++, obs_module_load_rules_arg_missing(m)) ||
			       (asked++, obs_module_load_rules_arg_missing(p)) ||
			       (asked++, obs_module_load_rules_arg_missing(c));
		assert_int_equal(missing, bits != 7);
		assert_int_equal(asked, !m ? 1 : !p ? 2 : 3);
	}
}

static void test_export_missing(void **state)
{
	(void)state;
	assert_true(obs_module_load_rules_export_missing(false));
	assert_false(obs_module_load_rules_export_missing(true));
}

static void test_version_word(void **state)
{
	(void)state;
	assert_int_equal(obs_module_load_rules_version_word(0), 0);
	assert_int_equal(obs_module_load_rules_version_word(0xFFFF), 0);
	assert_int_equal(obs_module_load_rules_version_word(0x10000), 0x10000);
	assert_int_equal(obs_module_load_rules_version_word(0x1F0042), 0x1F0000);
	assert_int_equal(obs_module_load_rules_version_word(0xFFFFFFFFu), 0xFFFF0000u);
	assert_int_equal(obs_module_load_rules_version_word(LIBOBS_API_VER), LIBOBS_API_VER & 0xFFFF0000u);
}

static void test_version_ok(void **state)
{
	(void)state;
	const uint32_t api = LIBOBS_API_VER;
	const uint32_t major_minor = api & 0xFFFF0000u;

	/* Same major and minor, any patch: the word has no patch, so it is never
	 * above a libobs with the same major and minor. */
	for (uint32_t patch = 0; patch <= 0xFFFF; patch += 0x1111) {
		uint32_t word = obs_module_load_rules_version_word(major_minor | patch);
		assert_true(obs_module_load_rules_version_ok(word, api));
	}
	/* Older minors and majors load. */
	assert_true(obs_module_load_rules_version_ok(major_minor - 0x10000, api));
	assert_true(obs_module_load_rules_version_ok(0, api));
	/* One minor above, one major above and the top word don't. */
	assert_false(obs_module_load_rules_version_ok(major_minor + 0x10000, api));
	assert_false(obs_module_load_rules_version_ok(major_minor + 0x1000000, api));
	assert_false(obs_module_load_rules_version_ok(0xFFFF0000u, api));
	/* The comparison is the whole 32 bits against the libobs version as
	 * given, patch included: a word with a patch above it is refused
	 * (characterized, not endorsed: obs-module.c only passes words without
	 * a patch). */
	assert_true(obs_module_load_rules_version_ok(api, api));
	assert_false(obs_module_load_rules_version_ok(api + 1, api));
	assert_true(obs_module_load_rules_version_ok(0, 0));
	assert_false(obs_module_load_rules_version_ok(1, 0));
	assert_true(obs_module_load_rules_version_ok(0xFFFFFFFFu, 0xFFFFFFFFu));
}

static void test_lists(void **state)
{
	(void)state;
	/* An empty safe list allows every module; an empty disabled list
	 * disables none. Otherwise the name decides. */
	assert_int_equal(obs_module_load_rules_safe_list(0), OBS_MODULE_LOAD_RULES_LIST_ALL);
	assert_int_equal(obs_module_load_rules_disabled_list(0), OBS_MODULE_LOAD_RULES_LIST_NONE);
	const size_t nums[] = {1, 2, 3, 1000, SIZE_MAX};
	for (size_t i = 0; i < sizeof(nums) / sizeof(nums[0]); i++) {
		assert_int_equal(obs_module_load_rules_safe_list(nums[i]), OBS_MODULE_LOAD_RULES_LIST_MATCH);
		assert_int_equal(obs_module_load_rules_disabled_list(nums[i]), OBS_MODULE_LOAD_RULES_LIST_MATCH);
	}
}

static void test_allow_disable(void **state)
{
	(void)state;
	assert_false(obs_module_load_rules_allow_disable(true));
	assert_true(obs_module_load_rules_allow_disable(false));
}

static void test_types(void **state)
{
	(void)state;
	const uint32_t types[] = {OBS_MODULE_TYPE_UNDEFINED,
				  OBS_MODULE_TYPE_CORE,
				  OBS_MODULE_TYPE_PLUGIN,
				  OBS_MODULE_TYPE_LEGACY_PLUGIN,
				  4,
				  0x101,
				  0x103,
				  0xFFFFFFFFu};
	for (size_t i = 0; i < sizeof(types) / sizeof(types[0]); i++) {
		assert_int_equal(obs_module_load_rules_is_core(types[i]), types[i] == OBS_MODULE_TYPE_CORE);
		assert_int_equal(obs_module_load_rules_is_legacy(types[i]), types[i] == OBS_MODULE_TYPE_LEGACY_PLUGIN);
	}
}

#define R_DISABLE OBS_MODULE_LOAD_RULES_DISABLE
#define R_KEEP OBS_MODULE_LOAD_RULES_KEEP_TYPE
#define R_SET OBS_MODULE_LOAD_RULES_SET_TYPE
#define R_FREE OBS_MODULE_LOAD_RULES_FREE
#define R_FAIL OBS_MODULE_LOAD_RULES_FAIL
#define R_STOP OBS_MODULE_LOAD_RULES_STOP
#define R_CORE OBS_MODULE_LOAD_RULES_CORE_CONTINUES

static const uint32_t all_types[] = {OBS_MODULE_TYPE_UNDEFINED,
				     OBS_MODULE_TYPE_CORE,
				     OBS_MODULE_TYPE_PLUGIN,
				     OBS_MODULE_TYPE_LEGACY_PLUGIN,
				     4,
				     0x101,
				     0xFFFFFFFFu};
#define NUM_TYPES (sizeof(all_types) / sizeof(all_types[0]))

static void test_open_route(void **state)
{
	(void)state;
	uint32_t st = 1234;

	/* Not an OBS plugin: skipped, nothing disabled or counted. */
	assert_int_equal(obs_module_load_rules_open_route(MODULE_MISSING_EXPORTS, &st), R_STOP);
	assert_int_equal(st, OBS_MODULE_INVALID);

	/* Failed to open: disabled, then counted, unless it is a core module,
	 * which keeps its type and is not counted (a missing libcef.so must not
	 * stop startup). The type is not an input: the C reads it only here,
	 * after making the disabled module. */
	st = 1234;
	assert_int_equal(obs_module_load_rules_open_route(MODULE_FAILED_TO_OPEN, &st),
			 R_DISABLE | R_CORE | R_FAIL | R_STOP);
	assert_int_equal(st, OBS_MODULE_FAILED_TO_OPEN);

	/* An error is counted, with no disabled module. */
	st = 1234;
	assert_int_equal(obs_module_load_rules_open_route(MODULE_ERROR, &st), R_FAIL | R_STOP);
	assert_int_equal(st, OBS_MODULE_INVALID);

	/* A newer libobs: disabled as failed to open, and counted, core module
	 * or not. */
	st = 1234;
	assert_int_equal(obs_module_load_rules_open_route(MODULE_INCOMPATIBLE_VER, &st), R_DISABLE | R_FAIL | R_STOP);
	assert_int_equal(st, OBS_MODULE_FAILED_TO_OPEN);

	/* The host skips it: nothing at all. */
	st = 1234;
	assert_int_equal(obs_module_load_rules_open_route(MODULE_HARDCODED_SKIP, &st), R_STOP);
	assert_int_equal(st, OBS_MODULE_INVALID);

	/* Success goes on to obs_init_module(). So does a code obs_open_module()
	 * never returns: the switch has no default (characterized, not
	 * endorsed). */
	const int codes[] = {MODULE_SUCCESS, 1, 2, -6, -100, INT_MIN, INT_MAX};
	for (size_t c = 0; c < sizeof(codes) / sizeof(codes[0]); c++) {
		st = 1234;
		assert_int_equal(obs_module_load_rules_open_route(codes[c], &st), 0);
		assert_int_equal(st, OBS_MODULE_INVALID);
	}
}

static void test_init_route(void **state)
{
	(void)state;
	uint32_t st = 1234;
	assert_int_equal(obs_module_load_rules_init_route(true, &st), R_SET);
	assert_int_equal(st, OBS_MODULE_INVALID);

	/* A module whose obs_module_load() fails is freed and recorded as a
	 * disabled module with its type, and is not counted as a failure. */
	st = 1234;
	assert_int_equal(obs_module_load_rules_init_route(false, &st), R_FREE | R_DISABLE | R_KEEP);
	assert_int_equal(st, OBS_MODULE_FAILED_TO_INITIALIZE);
}

static void test_counts(void **state)
{
	(void)state;
	assert_true(obs_module_load_rules_count_failure(true));
	assert_false(obs_module_load_rules_count_failure(false));
	assert_true(obs_module_load_rules_core_loaded(0));
	assert_false(obs_module_load_rules_core_loaded(1));
	assert_false(obs_module_load_rules_core_loaded(2));
	assert_false(obs_module_load_rules_core_loaded(SIZE_MAX));
}

/* ------------------------------------------- load_all_callback() decision */

/* What load_all_callback() reads, as the stand-ins below answer it. */
struct world {
	bool is_plugin;
	size_t safe_num;
	bool safe_listed;
	size_t disabled_num;
	bool disabled_listed;
	uint32_t type;
	bool core_name;
	bool existing;
	int code;
	bool init_ok;
	bool has_fail_info;
};

/* What it did, and which lookups ran. */
struct outcome {
	int skip;            /* why it stopped before any module was made */
	uint32_t disabled;   /* the disabled module's state, 0 for none */
	bool disabled_typed; /* that disabled module got the module's type */
	bool init;           /* obs_init_module() ran */
	bool freed;
	bool typed; /* the loaded module got its type */
	bool counted;
	bool core_warning; /* "Core module ... continuing without it" */
	bool asked_type;   /* the type read after obs_open_module() */
	/* the lookups */
	bool asked_safe, asked_disabled, asked_core, asked_existing, opened;
};

enum skip {
	SKIP_NONE,
	SKIP_NOT_PLUGIN,
	SKIP_NOT_SAFE,
	SKIP_DISABLED,
	SKIP_LEGACY_CORE,
	SKIP_CORE_LOADED,
	SKIP_LEGACY_LOADED,
	SKIP_PLUGIN_LOADED,
	SKIP_OPEN, /* after obs_open_module(), before obs_init_module() */
};

static bool list_answer(enum obs_module_load_rules_list answer, bool listed, bool *asked)
{
	switch (answer) {
	case OBS_MODULE_LOAD_RULES_LIST_ALL:
		return true;
	case OBS_MODULE_LOAD_RULES_LIST_NONE:
		return false;
	case OBS_MODULE_LOAD_RULES_LIST_MATCH:
		break;
	}
	*asked = true;
	return listed;
}

/* load_all_callback(), on the steps, as obs-module.c.h calls them. */
static struct outcome by_steps(const struct world *w)
{
	struct outcome o = {0};

	if (!w->is_plugin) {
		o.skip = SKIP_NOT_PLUGIN;
		return o;
	}
	if (!list_answer(obs_module_load_rules_safe_list(w->safe_num), w->safe_listed, &o.asked_safe)) {
		o.skip = SKIP_NOT_SAFE;
		o.disabled = OBS_MODULE_DISABLED_SAFE;
		return o;
	}
	if (!obs_module_load_rules_is_core(w->type) &&
	    list_answer(obs_module_load_rules_disabled_list(w->disabled_num), w->disabled_listed, &o.asked_disabled)) {
		o.skip = SKIP_DISABLED;
		o.disabled = OBS_MODULE_DISABLED;
		return o;
	}
	if (obs_module_load_rules_is_legacy(w->type)) {
		o.asked_core = true;
		if (w->core_name) {
			o.skip = SKIP_LEGACY_CORE;
			return o;
		}
	}
	o.asked_existing = true;
	if (w->existing) {
		if (obs_module_load_rules_is_core(w->type))
			o.skip = SKIP_CORE_LOADED;
		else if (obs_module_load_rules_is_legacy(w->type))
			o.skip = SKIP_LEGACY_LOADED;
		else
			o.skip = SKIP_PLUGIN_LOADED;
		return o;
	}

	o.opened = true;
	uint32_t st;
	uint32_t route = obs_module_load_rules_open_route(w->code, &st);
	if (route & R_DISABLE)
		o.disabled = st;
	if (route & R_CORE) {
		o.asked_type = true;
		if (obs_module_load_rules_is_core(w->type)) {
			o.disabled_typed = true;
			o.core_warning = true;
			o.skip = SKIP_OPEN;
			return o;
		}
	}
	if (route & R_FAIL) {
		o.skip = SKIP_OPEN;
		o.counted = obs_module_load_rules_count_failure(w->has_fail_info);
		return o;
	}
	if (route & R_STOP) {
		o.skip = SKIP_OPEN;
		return o;
	}

	o.init = true;
	route = obs_module_load_rules_init_route(w->init_ok, &st);
	o.freed = (route & R_FREE) != 0;
	if (route & R_DISABLE)
		o.disabled = st;
	if (route & R_KEEP)
		o.disabled_typed = true;
	o.typed = (route & R_SET) != 0;
	return o;
}

/* load_all_callback() as the C read before the move. */
static struct outcome by_c(const struct world *w)
{
	struct outcome o = {0};

	if (!w->is_plugin) {
		o.skip = SKIP_NOT_PLUGIN;
		return o;
	}
	bool safe = true;
	if (w->safe_num) {
		o.asked_safe = true;
		safe = w->safe_listed;
	}
	if (!safe) {
		o.skip = SKIP_NOT_SAFE;
		o.disabled = OBS_MODULE_DISABLED_SAFE;
		return o;
	}
	if (w->type != OBS_MODULE_TYPE_CORE) {
		bool disabled = false;
		if (w->disabled_num != 0) {
			o.asked_disabled = true;
			disabled = w->disabled_listed;
		}
		if (disabled) {
			o.skip = SKIP_DISABLED;
			o.disabled = OBS_MODULE_DISABLED;
			return o;
		}
	}
	if (w->type == OBS_MODULE_TYPE_LEGACY_PLUGIN) {
		o.asked_core = true;
		if (w->core_name) {
			o.skip = SKIP_LEGACY_CORE;
			return o;
		}
	}
	o.asked_existing = true;
	if (w->existing) {
		o.skip = w->type == OBS_MODULE_TYPE_CORE            ? SKIP_CORE_LOADED
			 : w->type == OBS_MODULE_TYPE_LEGACY_PLUGIN ? SKIP_LEGACY_LOADED
								    : SKIP_PLUGIN_LOADED;
		return o;
	}

	o.opened = true;
	bool fail = false;
	switch (w->code) {
	case MODULE_MISSING_EXPORTS:
		o.skip = SKIP_OPEN;
		return o;
	case MODULE_FAILED_TO_OPEN:
		o.disabled = OBS_MODULE_FAILED_TO_OPEN;
		o.asked_type = true;
		if (w->type == OBS_MODULE_TYPE_CORE) {
			o.disabled_typed = true;
			o.core_warning = true;
			o.skip = SKIP_OPEN;
			return o;
		}
		fail = true;
		break;
	case MODULE_ERROR:
		fail = true;
		break;
	case MODULE_INCOMPATIBLE_VER:
		o.disabled = OBS_MODULE_FAILED_TO_OPEN;
		fail = true;
		break;
	case MODULE_HARDCODED_SKIP:
		o.skip = SKIP_OPEN;
		return o;
	}
	if (fail) {
		o.skip = SKIP_OPEN;
		o.counted = w->has_fail_info;
		return o;
	}

	o.init = true;
	if (!w->init_ok) {
		o.freed = true;
		o.disabled = OBS_MODULE_FAILED_TO_INITIALIZE;
		o.disabled_typed = true;
	} else {
		o.typed = true;
	}
	return o;
}

static void expect_same(const struct world *w)
{
	struct outcome a = by_steps(w), b = by_c(w);
	if (memcmp(&a, &b, sizeof(a)) != 0) {
		fprintf(stderr,
			"plugin %d safe %zu/%d disabled %zu/%d type %u core %d existing %d code %d init %d fail %d: "
			"skip %d/%d disabled %u/%u\n",
			w->is_plugin, w->safe_num, w->safe_listed, w->disabled_num, w->disabled_listed, w->type,
			w->core_name, w->existing, w->code, w->init_ok, w->has_fail_info, a.skip, b.skip, a.disabled,
			b.disabled);
	}
	assert_int_equal(a.skip, b.skip);
	assert_int_equal(a.disabled, b.disabled);
	assert_int_equal(a.disabled_typed, b.disabled_typed);
	assert_int_equal(a.init, b.init);
	assert_int_equal(a.freed, b.freed);
	assert_int_equal(a.typed, b.typed);
	assert_int_equal(a.counted, b.counted);
	assert_int_equal(a.core_warning, b.core_warning);
	assert_int_equal(a.asked_type, b.asked_type);
	assert_int_equal(a.asked_safe, b.asked_safe);
	assert_int_equal(a.asked_disabled, b.asked_disabled);
	assert_int_equal(a.asked_core, b.asked_core);
	assert_int_equal(a.asked_existing, b.asked_existing);
	assert_int_equal(a.opened, b.opened);
}

static const int all_codes[] = {MODULE_SUCCESS,
				MODULE_ERROR,
				MODULE_FAILED_TO_OPEN,
				MODULE_MISSING_EXPORTS,
				MODULE_INCOMPATIBLE_VER,
				MODULE_HARDCODED_SKIP,
				1,
				-6};
#define NUM_CODES (sizeof(all_codes) / sizeof(all_codes[0]))

static void test_route_every_combination(void **state)
{
	(void)state;
	size_t cases = 0;
	for (int bits = 0; bits < (1 << 9); bits++) {
		for (size_t t = 0; t < NUM_TYPES; t++) {
			for (size_t c = 0; c < NUM_CODES; c++) {
				struct world w = {
					.is_plugin = bits & 1,
					.safe_num = (bits & 2) ? 2 : 0,
					.safe_listed = bits & 4,
					.disabled_num = (bits & 8) ? 1 : 0,
					.disabled_listed = bits & 16,
					.type = all_types[t],
					.core_name = bits & 32,
					.existing = bits & 64,
					.code = all_codes[c],
					.init_ok = bits & 128,
					.has_fail_info = bits & 256,
				};
				expect_same(&w);
				cases++;
			}
		}
	}
	assert_int_equal(cases, 512 * NUM_TYPES * NUM_CODES);
}

/* A few routes spelled out. */
static void test_route_examples(void **state)
{
	(void)state;
	struct world base = {.is_plugin = true, .type = OBS_MODULE_TYPE_PLUGIN, .init_ok = true, .has_fail_info = true};
	struct world w;
	struct outcome o;

	/* A plugin that loads gets its type. */
	o = by_steps(&base);
	assert_int_equal(o.skip, SKIP_NONE);
	assert_true(o.init);
	assert_true(o.typed);
	assert_int_equal(o.disabled, 0);

	/* Not on a non-empty safe list: disabled (safe), even a core module. */
	w = base;
	w.safe_num = 1;
	w.type = OBS_MODULE_TYPE_CORE;
	o = by_steps(&w);
	assert_int_equal(o.skip, SKIP_NOT_SAFE);
	assert_int_equal(o.disabled, OBS_MODULE_DISABLED_SAFE);
	assert_true(o.asked_safe);

	/* A core module on the disabled list loads anyway: the list is not
	 * even looked at. */
	w = base;
	w.type = OBS_MODULE_TYPE_CORE;
	w.disabled_num = 1;
	w.disabled_listed = true;
	o = by_steps(&w);
	assert_int_equal(o.skip, SKIP_NONE);
	assert_false(o.asked_disabled);
	assert_true(o.typed);

	/* A legacy plugin with a core module's name is skipped, nothing made. */
	w = base;
	w.type = OBS_MODULE_TYPE_LEGACY_PLUGIN;
	w.core_name = true;
	o = by_steps(&w);
	assert_int_equal(o.skip, SKIP_LEGACY_CORE);
	assert_int_equal(o.disabled, 0);
	assert_false(o.asked_existing);

	/* A plugin with a core module's name is not: only legacy plugins ask. */
	w = base;
	w.core_name = true;
	o = by_steps(&w);
	assert_int_equal(o.skip, SKIP_NONE);
	assert_false(o.asked_core);

	/* A core module that can't be opened: disabled with its type, not
	 * counted. */
	w = base;
	w.type = OBS_MODULE_TYPE_CORE;
	w.code = MODULE_FAILED_TO_OPEN;
	o = by_steps(&w);
	assert_int_equal(o.disabled, OBS_MODULE_FAILED_TO_OPEN);
	assert_true(o.disabled_typed);
	assert_true(o.core_warning);
	assert_false(o.counted);
	assert_false(o.init);

	/* A plugin that can't be opened is counted, but only with a failure
	 * list. */
	w = base;
	w.code = MODULE_FAILED_TO_OPEN;
	o = by_steps(&w);
	assert_true(o.counted);
	assert_false(o.disabled_typed);
	w.has_fail_info = false;
	o = by_steps(&w);
	assert_false(o.counted);
	assert_int_equal(o.disabled, OBS_MODULE_FAILED_TO_OPEN);

	/* A failed obs_module_load(): freed, disabled with its type, not
	 * counted. */
	w = base;
	w.init_ok = false;
	o = by_steps(&w);
	assert_true(o.freed);
	assert_int_equal(o.disabled, OBS_MODULE_FAILED_TO_INITIALIZE);
	assert_true(o.disabled_typed);
	assert_false(o.typed);
	assert_false(o.counted);
}

/* obs_load_core_modules(): the cached result, else loaded only when no
 * failure was counted; a failure is not cached, so a retry walks the list
 * again. */
static bool core_cached;
static bool load_core(size_t fails, bool *walked)
{
	*walked = false;
	if (core_cached)
		return true;
	*walked = true;
	core_cached = obs_module_load_rules_core_loaded(fails);
	return core_cached;
}

static void test_core_load_twice(void **state)
{
	(void)state;
	bool walked;

	/* After a success: cached, the second call walks nothing. */
	core_cached = false;
	assert_true(load_core(0, &walked));
	assert_true(walked);
	assert_true(load_core(5, &walked));
	assert_false(walked);

	/* After a failure: walked again, and loaded once nothing fails. */
	core_cached = false;
	assert_false(load_core(1, &walked));
	assert_true(walked);
	assert_false(load_core(3, &walked));
	assert_true(walked);
	assert_true(load_core(0, &walked));
	assert_true(walked);
	assert_true(load_core(1, &walked));
	assert_false(walked);
}

/* ================================================================ callers */

#define LOG_MAX 32
struct logged {
	int level;
	char msg[512];
};
static struct logged logged[LOG_MAX];
static size_t logged_num;
static bool capturing;
static log_handler_t prev_handler;
static void *prev_param;

static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	if (capturing && strstr(format, "Module")) {
		if (logged_num < LOG_MAX) {
			logged[logged_num].level = level;
			vsnprintf(logged[logged_num].msg, sizeof(logged[0].msg), format, args);
		}
		logged_num++;
		return;
	}
	if (prev_handler)
		prev_handler(level, format, args, prev_param);
}

static void start_capture(void)
{
	logged_num = 0;
	capturing = true;
}

static bool obs_up;

static int group_setup(void **state)
{
	(void)state;
	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(capture_log, NULL);

	/* Before obs_startup(): no core, so obs_open_module() refuses. */
	obs_module_t *m = NULL;
	if (obs_open_module(&m, "module-load-steps-none", NULL) != MODULE_ERROR || m != NULL) {
		fprintf(stderr, "obs_open_module without a core did not return MODULE_ERROR\n");
		return -1;
	}

	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		if (getenv("DISPLAY") || getenv("WAYLAND_DISPLAY")) {
			fprintf(stderr, "obs_startup failed with a display set\n");
			return -1;
		}
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	obs_up = false;
	capturing = false;
	base_set_log_handler(prev_handler, prev_param);
	return 0;
}

#define REQUIRE_OBS()           \
	do {                    \
		if (!obs_up)    \
			skip(); \
	} while (false)

static void test_open_module_gates(void **state)
{
	(void)state;
	REQUIRE_OBS();
	obs_module_t *m = NULL;

	start_capture();
	assert_int_equal(obs_open_module(NULL, "module-load-steps-none", NULL), MODULE_ERROR);
	assert_int_equal(obs_open_module(&m, NULL, NULL), MODULE_ERROR);
	assert_null(m);
	assert_int_equal(logged_num, 0);
	capturing = false;
}

static void test_open_module_missing_file(void **state)
{
	(void)state;
	REQUIRE_OBS();
	obs_module_t *m = NULL;
	const char *path = "module-load-steps-does-not-exist";

	start_capture();
	assert_int_equal(obs_open_module(&m, path, NULL), MODULE_FAILED_TO_OPEN);
	capturing = false;
	assert_null(m);
	assert_int_equal(logged_num, 1);
	assert_int_equal(logged[0].level, LOG_WARNING);
	assert_string_equal(logged[0].msg, "Module 'module-load-steps-does-not-exist' not loaded");
	assert_null(obs_get_module("module-load-steps-does-not-exist"));
}

static void test_allow_disable_caller(void **state)
{
	(void)state;
	/* A name that is no core module's can be disabled. */
	assert_true(obs_get_module_allow_disable("module-load-steps-not-core"));
	assert_true(obs_get_module_allow_disable(""));
}

static void test_is_core_legacy_null(void **state)
{
	(void)state;
	assert_false(obs_is_core_module(NULL));
	assert_false(obs_is_legacy_module(NULL));
}

#define CASE(fn) cmocka_unit_test(fn)

int main(void)
{
	const struct CMUnitTest tests[] = {
		CASE(test_arg_missing),
		CASE(test_export_missing),
		CASE(test_version_word),
		CASE(test_version_ok),
		CASE(test_lists),
		CASE(test_allow_disable),
		CASE(test_types),
		CASE(test_open_route),
		CASE(test_init_route),
		CASE(test_counts),
		CASE(test_route_every_combination),
		CASE(test_route_examples),
		CASE(test_core_load_twice),
		CASE(test_open_module_gates),
		CASE(test_open_module_missing_file),
		CASE(test_allow_disable_caller),
		CASE(test_is_core_legacy_null),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
