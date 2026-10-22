#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-internal.h"

/*
 * Characterization of the two context name helpers in
 * libobs/obs-context-names.c, before their Rust port:
 * obs_context_name_dup() (the "__unnamed%04lld" name for a NULL or empty
 * name, from the counter obs.c passes in) and obs_context_name_deduplicate()
 * (the "%s %d" name from 2 up that makes a taken name unique, asking obs.c's
 * exists() which names are taken). They pin the current C behavior; cases
 * marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs. This test calls only them, so the
 * same file can later be linked against the Rust port instead of the C (as
 * test_scene_bounds_place_rust does). Here it compiles the C source: no
 * libobs core and no name tables. test_context_names covers the callers
 * through the real libobs.
 *
 * Names come from scene collections, obs-websocket and scripts, so they are
 * any bytes: '%' sequences, bytes that are not UTF-8, names that already end
 * in a number, a long name, many taken names.
 */

static long allocs_before;

static int setup(void **state)
{
	(void)state;
	allocs_before = bnum_allocs();
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	/* Every name the helpers return is freed by the test. */
	if (bnum_allocs() != allocs_before) {
		fprintf(stderr, "allocations: %ld before, %ld after\n", allocs_before, bnum_allocs());
		return -1;
	}
	return 0;
}

/* ------------------------------------------------------------------------ */
/* exists(): a set of taken names, and a log of what was asked               */

#define MAX_ASKED 16

struct names {
	const char *const *taken;
	size_t num_taken;
	/* taken: the first num_always_taken questions, whatever the name */
	size_t num_always_taken;
	size_t num_asked;
	bool asked_null;
	char asked[MAX_ASKED][64];
};

static bool exists(void *param, const char *name)
{
	struct names *names = param;
	size_t n = names->num_asked++;

	if (!name) {
		names->asked_null = true;
		return false;
	}
	if (n < MAX_ASKED)
		snprintf(names->asked[n], sizeof(names->asked[n]), "%s", name);
	if (n < names->num_always_taken)
		return true;
	for (size_t i = 0; i < names->num_taken; i++) {
		if (strcmp(names->taken[i], name) == 0)
			return true;
	}
	return false;
}

#define COUNT(a) (sizeof(a) / sizeof((a)[0]))
#define TAKEN(...)                                                              \
	static const char *const taken_[] = {__VA_ARGS__};                      \
	struct names names = {.taken = taken_, .num_taken = COUNT(taken_)}

/* deduplicate(name) with a fresh log, freeing the result into buf. */
static bool dedup(struct names *names, const char *name, char *buf, size_t size)
{
	names->num_asked = 0;
	char *result = obs_context_name_deduplicate(name, exists, names);
	if (!result)
		return false;
	snprintf(buf, size, "%s", result);
	bfree(result);
	return true;
}

/* ------------------------------------------------------------------------ */
/* obs_context_name_deduplicate                                              */

static void test_dedup_free_name(void **state)
{
	(void)state;
	TAKEN("A", "B 2");
	char buf[64];

	/* A free name: NULL (keep the name), after one question. */
	assert_false(dedup(&names, "B", buf, sizeof(buf)));
	assert_int_equal(names.num_asked, 1);
	assert_string_equal(names.asked[0], "B");

	assert_false(dedup(&names, "a", buf, sizeof(buf)));
	assert_false(dedup(&names, "A ", buf, sizeof(buf)));
	assert_false(dedup(&names, " A", buf, sizeof(buf)));
	assert_false(dedup(&names, "", buf, sizeof(buf)));
}

static void test_dedup_suffix(void **state)
{
	(void)state;
	char buf[64];

	{
		TAKEN("A");
		assert_true(dedup(&names, "A", buf, sizeof(buf)));
		assert_string_equal(buf, "A 2");
		assert_int_equal(names.num_asked, 2);
		assert_string_equal(names.asked[0], "A");
		assert_string_equal(names.asked[1], "A 2");
	}
	{
		/* The first free number from 2 up, gaps included. */
		TAKEN("A", "A 2", "A 4");
		assert_true(dedup(&names, "A", buf, sizeof(buf)));
		assert_string_equal(buf, "A 3");
		assert_int_equal(names.num_asked, 3);
		assert_string_equal(names.asked[2], "A 3");
	}
	{
		TAKEN("A", "A 3");
		assert_true(dedup(&names, "A", buf, sizeof(buf)));
		assert_string_equal(buf, "A 2");
	}
	{
		/* Only the name itself decides whether a suffix is needed. */
		TAKEN("A 2", "A 3");
		assert_false(dedup(&names, "A", buf, sizeof(buf)));
	}
	{
		/* " 1" and " 0" are never tried. */
		TAKEN("A", "A 1", "A 0");
		assert_true(dedup(&names, "A", buf, sizeof(buf)));
		assert_string_equal(buf, "A 2");
	}
}

/* A name is not parsed: a trailing number or " (2)" is more bytes. */
static void test_dedup_names_ending_in_numbers(void **state)
{
	(void)state;
	static const struct {
		const char *name;
		const char *result;
	} cases[] = {
		{"Cam 2", "Cam 2 2"},
		{"X (2)", "X (2) 2"},
		{"Y2", "Y2 2"},
		{"Z 0", "Z 0 2"},
		{"N -1", "N -1 2"},
		{"9", "9 2"},
		{"Big 2147483647", "Big 2147483647 2"},
		{"A 02", "A 02 2"},
	};
	char buf[64];

	for (size_t i = 0; i < COUNT(cases); i++) {
		const char *taken[] = {cases[i].name};
		struct names names = {.taken = taken, .num_taken = 1};
		assert_true(dedup(&names, cases[i].name, buf, sizeof(buf)));
		assert_string_equal(buf, cases[i].result);
	}

	TAKEN("Cam 2", "Cam 2 2");
	assert_true(dedup(&names, "Cam 2", buf, sizeof(buf)));
	assert_string_equal(buf, "Cam 2 3");
}

/* '%' is never a format, and bytes are kept as they are. */
static void test_dedup_bytes(void **state)
{
	(void)state;
	char buf[64];
	{
		TAKEN("%s %d %n %%");
		assert_true(dedup(&names, "%s %d %n %%", buf, sizeof(buf)));
		assert_string_equal(buf, "%s %d %n %% 2");
	}
	{
		TAKEN("\xff\xfe\x80", "\xff\xfe\x80 2");
		assert_true(dedup(&names, "\xff\xfe\x80", buf, sizeof(buf)));
		assert_string_equal(buf, "\xff\xfe\x80 3");
	}
	{
		/* Characterized, not endorsed: an empty name that is taken gets
		 * " 2" (libobs gives a context an unnamed name first, so no
		 * table holds "" and this is not reached through it). */
		TAKEN("");
		assert_true(dedup(&names, "", buf, sizeof(buf)));
		assert_string_equal(buf, " 2");
	}
	{
		TAKEN("\t\n");
		assert_true(dedup(&names, "\t\n", buf, sizeof(buf)));
		assert_string_equal(buf, "\t\n 2");
	}
}

static void test_dedup_long_name(void **state)
{
	(void)state;
	const size_t len = 1024 * 1024;
	char *name = bmalloc(len + 1);
	memset(name, 'L', len);
	name[len] = 0;

	struct names names = {.num_always_taken = 1};
	char *result = obs_context_name_deduplicate(name, exists, &names);
	assert_non_null(result);
	assert_int_equal(strlen(result), len + 2);
	assert_memory_equal(result, name, len);
	assert_string_equal(result + len, " 2");
	assert_int_equal(names.num_asked, 2);

	bfree(result);
	bfree(name);
}

/* Many taken names: one question per number, in order. */
static void test_dedup_many(void **state)
{
	(void)state;
	struct names names = {.num_always_taken = 100000};
	char *result = obs_context_name_deduplicate("M", exists, &names);
	assert_string_equal(result, "M 100001");
	assert_int_equal(names.num_asked, 100001);
	assert_string_equal(names.asked[0], "M");
	assert_string_equal(names.asked[1], "M 2");
	assert_string_equal(names.asked[15], "M 16");
	bfree(result);
}

static void test_dedup_param(void **state)
{
	(void)state;
	/* exists() gets param as it is, and the name it was asked for. */
	TAKEN("P");
	char *result = obs_context_name_deduplicate("P", exists, &names);
	assert_string_equal(result, "P 2");
	assert_int_equal(names.num_asked, 2);
	bfree(result);
}

/* Characterized, not endorsed: a NULL name goes to exists() as it is. The
 * exists() of obs.c (HASH_FIND_STR) does not take NULL; only a rename of a
 * public source to NULL reaches it. */
static void test_dedup_null_name(void **state)
{
	(void)state;
	struct names names = {0};
	assert_null(obs_context_name_deduplicate(NULL, exists, &names));
	assert_true(names.asked_null);
	assert_int_equal(names.num_asked, 1);
}

/* ------------------------------------------------------------------------ */
/* obs_context_name_dup                                                      */

static void check_dup(const char *name, bool private, long long index, const char *expected, long long next)
{
	long long counter = index;
	char *result = obs_context_name_dup(name, private, &counter);
	if (expected) {
		assert_non_null(result);
		assert_string_equal(result, expected);
		assert_ptr_not_equal(result, name);
	} else {
		assert_null(result);
	}
	assert_true(counter == next);
	bfree(result);
}

static void test_dup_named(void **state)
{
	(void)state;
	/* A name is copied, and the counter is not used. */
	check_dup("A", false, 0, "A", 0);
	check_dup("A", true, 7, "A", 7);
	check_dup(" ", false, 0, " ", 0);
	check_dup("%s %d", false, 0, "%s %d", 0);
	check_dup("\xff\xfe\x80", false, 0, "\xff\xfe\x80", 0);
	check_dup("__unnamed0000", false, 0, "__unnamed0000", 0);
}

static void test_dup_unnamed(void **state)
{
	(void)state;
	check_dup(NULL, false, 0, "__unnamed0000", 1);
	check_dup("", false, 0, "__unnamed0000", 1);
	check_dup("", false, 1, "__unnamed0001", 2);
	check_dup(NULL, false, 42, "__unnamed0042", 43);
	check_dup(NULL, false, 9999, "__unnamed9999", 10000);
	check_dup(NULL, false, 10000, "__unnamed10000", 10001);

	/* A private context keeps a NULL name and does not use the counter;
	 * an empty name still gets an unnamed name. */
	check_dup(NULL, true, 5, NULL, 5);
	check_dup("", true, 5, "__unnamed0005", 6);

	/* The counter in sequence. */
	long long counter = 0;
	char *a = obs_context_name_dup(NULL, false, &counter);
	char *b = obs_context_name_dup("", false, &counter);
	char *c = obs_context_name_dup("C", false, &counter);
	char *d = obs_context_name_dup(NULL, true, &counter);
	char *e = obs_context_name_dup("", true, &counter);
	assert_string_equal(a, "__unnamed0000");
	assert_string_equal(b, "__unnamed0001");
	assert_string_equal(c, "C");
	assert_null(d);
	assert_string_equal(e, "__unnamed0002");
	assert_true(counter == 3);
	bfree(a);
	bfree(b);
	bfree(c);
	bfree(e);
}

/* Characterized, not endorsed: the counter is printed as a signed number,
 * the width is a minimum, and nothing bounds it. libobs starts it at 0 and
 * only counts up, so the negative values are not reached; LLONG_MAX itself
 * would overflow the increment (not driven here). */
static void test_dup_counter_range(void **state)
{
	(void)state;
	check_dup(NULL, false, -1, "__unnamed-001", 0);
	check_dup(NULL, false, -12345, "__unnamed-12345", -12344);
	check_dup(NULL, false, LLONG_MAX - 1, "__unnamed9223372036854775806", LLONG_MAX);
	check_dup(NULL, false, LLONG_MIN, "__unnamed-9223372036854775808", LLONG_MIN + 1);
}

static void test_dup_long_name(void **state)
{
	(void)state;
	const size_t len = 1024 * 1024;
	char *name = bmalloc(len + 1);
	memset(name, 'L', len);
	name[len] = 0;

	long long counter = 3;
	char *result = obs_context_name_dup(name, false, &counter);
	assert_int_equal(strlen(result), len);
	assert_memory_equal(result, name, len + 1);
	assert_true(counter == 3);

	bfree(result);
	bfree(name);
}

#define CASE(fn) cmocka_unit_test_setup_teardown(fn, setup, teardown)

int main(void)
{
	const struct CMUnitTest tests[] = {
		CASE(test_dedup_free_name), CASE(test_dedup_suffix),      CASE(test_dedup_names_ending_in_numbers),
		CASE(test_dedup_bytes),     CASE(test_dedup_long_name),   CASE(test_dedup_many),
		CASE(test_dedup_param),     CASE(test_dedup_null_name),   CASE(test_dup_named),
		CASE(test_dup_unnamed),     CASE(test_dup_counter_range), CASE(test_dup_long_name),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
