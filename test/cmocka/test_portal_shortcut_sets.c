#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <util/bmem.h>

#include "obs-nix-portal-shortcuts.h"

/*
 * Characterization tests for the combination set logic of the
 * GlobalShortcuts portal client in libobs/obs-nix-portal-shortcut-sets.c:
 * obs_nix_portal_sets_has, obs_nix_portal_sets_unchanged (whether a sync
 * binds again), obs_nix_portal_sets_request (the keys to ask for) and
 * obs_nix_portal_sets_partition (what bind_approved() binds again after a
 * declined dialog). They pin the current C behavior for the Rust port;
 * cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs. This test calls only them, so the
 * same file can later be linked against the Rust port instead of the C
 * (as test_scene_bounds_place_rust does). Here it compiles the C source:
 * no GIO, no D-Bus and no libobs core. test_portal_shortcuts covers the
 * portal client that uses it, end to end.
 */

static long allocs_before;

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	allocs_before = bnum_allocs();
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	/* The set logic only reads the arrays it is given. */
	if (bnum_allocs() != allocs_before) {
		fprintf(stderr, "allocations: %ld before, %ld after\n", allocs_before, bnum_allocs());
		return -1;
	}
	return 0;
}

#define COUNT(a) (sizeof(a) / sizeof((a)[0]))

static const obs_key_combination_t A = {0, OBS_KEY_A};
static const obs_key_combination_t B = {0, OBS_KEY_B};
static const obs_key_combination_t C = {0, OBS_KEY_C};
static const obs_key_combination_t CTRL_A = {INTERACT_CONTROL_KEY, OBS_KEY_A};
static const obs_key_combination_t SHIFT_A = {INTERACT_SHIFT_KEY, OBS_KEY_A};
static const obs_key_combination_t CAPS_A = {INTERACT_CAPS_KEY, OBS_KEY_A};
static const obs_key_combination_t NONE = {0, OBS_KEY_NONE};
static const obs_key_combination_t MOUSE1 = {0, OBS_KEY_MOUSE1};

static struct obs_nix_portal_combos combos(const obs_key_combination_t *array, size_t num)
{
	struct obs_nix_portal_combos view = {array, num};
	return view;
}

static struct obs_nix_portal_descriptions descriptions(const char *const *array, size_t num)
{
	struct obs_nix_portal_descriptions view = {array, num};
	return view;
}

static const struct obs_nix_portal_combos no_combos = {NULL, 0};
static const struct obs_nix_portal_descriptions no_descriptions = {NULL, 0};

/* ------------------------------------------------------------------------ */
/* obs_nix_portal_sets_has                                                   */

static void test_has(void **state)
{
	UNUSED_PARAMETER(state);

	assert_false(obs_nix_portal_sets_has(&no_combos, A));
	assert_false(obs_nix_portal_sets_has(&no_combos, NONE));

	const obs_key_combination_t set[] = {B, CTRL_A, MOUSE1, NONE};
	struct obs_nix_portal_combos view = combos(set, COUNT(set));
	assert_true(obs_nix_portal_sets_has(&view, B));
	assert_true(obs_nix_portal_sets_has(&view, CTRL_A));
	assert_true(obs_nix_portal_sets_has(&view, MOUSE1));
	assert_true(obs_nix_portal_sets_has(&view, NONE));

	/* The same key with other modifiers is another combination. */
	assert_false(obs_nix_portal_sets_has(&view, A));
	assert_false(obs_nix_portal_sets_has(&view, SHIFT_A));
	/* Every modifier bit counts, not only the four the portal knows. */
	assert_false(obs_nix_portal_sets_has(&view, CAPS_A));
	obs_key_combination_t ctrl_caps_a = {INTERACT_CONTROL_KEY | INTERACT_CAPS_KEY, OBS_KEY_A};
	assert_false(obs_nix_portal_sets_has(&view, ctrl_caps_a));

	/* Only num entries are looked at. */
	struct obs_nix_portal_combos first = combos(set, 1);
	assert_true(obs_nix_portal_sets_has(&first, B));
	assert_false(obs_nix_portal_sets_has(&first, CTRL_A));
}

/* ------------------------------------------------------------------------ */
/* obs_nix_portal_sets_unchanged with a session: the same set               */

/* unchanged(session, no retry) of current against bound. */
static bool same(const obs_key_combination_t *a, size_t a_num, const char *const *a_desc, size_t a_desc_num,
		 const obs_key_combination_t *b, size_t b_num, const char *const *b_desc, size_t b_desc_num)
{
	struct obs_nix_portal_combos current = combos(a, a_num), bound = combos(b, b_num);
	struct obs_nix_portal_descriptions current_desc = descriptions(a_desc, a_desc_num);
	struct obs_nix_portal_descriptions bound_desc = descriptions(b_desc, b_desc_num);
	/* declined is not looked at with a session. */
	const obs_key_combination_t all[] = {A, B, C};
	struct obs_nix_portal_combos declined = combos(all, COUNT(all));
	return obs_nix_portal_sets_unchanged(true, false, false, &current, &current_desc, &bound, &bound_desc,
					     &declined);
}

static void test_same_set_empty(void **state)
{
	UNUSED_PARAMETER(state);

	const obs_key_combination_t one[] = {A};
	const char *const one_desc[] = {"OBS: x"};

	assert_true(same(NULL, 0, NULL, 0, NULL, 0, NULL, 0));
	assert_false(same(one, 1, one_desc, 1, NULL, 0, NULL, 0));
	assert_false(same(NULL, 0, NULL, 0, one, 1, one_desc, 1));
	/* Descriptions without combinations change nothing. */
	assert_true(same(NULL, 0, one_desc, 1, NULL, 0, NULL, 0));
}

static void test_same_set_combinations(void **state)
{
	UNUSED_PARAMETER(state);

	const obs_key_combination_t ab[] = {A, B};
	const obs_key_combination_t ba[] = {B, A};
	const obs_key_combination_t a_ctrl_a[] = {A, CTRL_A};
	const obs_key_combination_t a_shift_a[] = {A, SHIFT_A};
	const obs_key_combination_t abc[] = {A, B, C};
	const char *const xy[] = {"x", "y"};
	const char *const yx[] = {"y", "x"};
	const char *const xyz[] = {"x", "y", "z"};

	assert_true(same(ab, 2, xy, 2, ab, 2, xy, 2));
	/* Order does not matter; each combination keeps its description. */
	assert_true(same(ab, 2, xy, 2, ba, 2, yx, 2));
	assert_false(same(ab, 2, xy, 2, ba, 2, xy, 2));
	/* The same key with other modifiers. */
	assert_false(same(a_ctrl_a, 2, xy, 2, a_shift_a, 2, xy, 2));
	/* One more or one fewer. */
	assert_false(same(ab, 2, xy, 2, abc, 3, xyz, 3));
	assert_false(same(abc, 3, xyz, 3, ab, 2, xy, 2));
}

/* Characterized, not endorsed: the combinations are compared as sets, so a
 * duplicate counts once; each copy is compared with the description of the
 * first copy on the other side. */
static void test_same_set_duplicates(void **state)
{
	UNUSED_PARAMETER(state);

	const obs_key_combination_t a[] = {A};
	const obs_key_combination_t aa[] = {A, A};
	const obs_key_combination_t aab[] = {A, A, B};
	const obs_key_combination_t ab[] = {A, B};
	const char *const x[] = {"x"};
	const char *const xx[] = {"x", "x"};
	const char *const xy[] = {"x", "y"};
	const char *const yx[] = {"y", "x"};
	const char *const xxz[] = {"x", "x", "z"};
	const char *const xz[] = {"x", "z"};

	assert_true(same(aa, 2, xx, 2, a, 1, x, 1));
	assert_true(same(a, 1, x, 1, aa, 2, xx, 2));
	assert_true(same(aab, 3, xxz, 3, ab, 2, xz, 2));
	/* The second copy has another description. */
	assert_false(same(aa, 2, xy, 2, a, 1, x, 1));
	/* Only the first copy on the other side counts. */
	assert_false(same(a, 1, x, 1, aa, 2, yx, 2));
	assert_true(same(a, 1, x, 1, aa, 2, xy, 2));
}

/* Fewer descriptions than combinations (the i < num guards): a combination
 * without one is never the same. */
static void test_same_set_short_descriptions(void **state)
{
	UNUSED_PARAMETER(state);

	const obs_key_combination_t ab[] = {A, B};
	const obs_key_combination_t ba[] = {B, A};
	const char *const x[] = {"x"};
	const char *const xy[] = {"x", "y"};
	const char *const yx[] = {"y", "x"};

	/* Current has no description for B. */
	assert_false(same(ab, 2, x, 1, ab, 2, xy, 2));
	/* Bound has no description for B. */
	assert_false(same(ab, 2, xy, 2, ab, 2, x, 1));
	/* Bound's one description is for its first combination, B: A has none. */
	assert_false(same(ab, 2, xy, 2, ba, 2, x, 1));
	assert_true(same(ab, 2, xy, 2, ba, 2, yx, 2));
	/* No descriptions at all. */
	assert_false(same(ab, 2, NULL, 0, ab, 2, NULL, 0));
	assert_false(same(ab, 2, NULL, 0, ab, 2, xy, 2));
}

/* A NULL description is never the same, not even as another NULL. */
static void test_same_set_null_description(void **state)
{
	UNUSED_PARAMETER(state);

	const obs_key_combination_t a[] = {A};
	const char *const null[] = {NULL};
	const char *const x[] = {"x"};
	const char *const empty[] = {""};

	assert_false(same(a, 1, null, 1, a, 1, x, 1));
	assert_false(same(a, 1, x, 1, a, 1, null, 1));
	/* Characterized, not endorsed. */
	assert_false(same(a, 1, null, 1, a, 1, null, 1));
	/* An empty description is a description. */
	assert_true(same(a, 1, empty, 1, a, 1, empty, 1));
	assert_false(same(a, 1, empty, 1, a, 1, null, 1));
	assert_false(same(a, 1, empty, 1, a, 1, x, 1));
}

/* Descriptions are compared byte by byte (strcmp): no Unicode
 * normalization, no case folding, and any bytes, valid UTF-8 or not. */
static void test_same_set_description_bytes(void **state)
{
	UNUSED_PARAMETER(state);

	const obs_key_combination_t a[] = {A};
	const char *const nfc[] = {"OBS: Mic \xc3\xa9"};
	const char *const nfd[] = {"OBS: Mic e\xcc\x81"};
	const char *const nfc_copy[] = {"OBS: Mic \xc3\xa9"};
	const char *const upper[] = {"OBS: MIC"};
	const char *const lower[] = {"OBS: mic"};
	const char *const prefix[] = {"OBS: Mic"};
	const char *const longer[] = {"OBS: Mic "};
	const char *const raw[] = {"\xff\xfe\x80 not UTF-8"};
	const char *const raw_copy[] = {"\xff\xfe\x80 not UTF-8"};
	const char *const raw_other[] = {"\xff\xfe\x81 not UTF-8"};

	assert_true(same(a, 1, nfc, 1, a, 1, nfc_copy, 1));
	assert_false(same(a, 1, nfc, 1, a, 1, nfd, 1));
	assert_false(same(a, 1, upper, 1, a, 1, lower, 1));
	assert_false(same(a, 1, prefix, 1, a, 1, longer, 1));
	assert_false(same(a, 1, longer, 1, a, 1, prefix, 1));
	assert_true(same(a, 1, raw, 1, a, 1, raw_copy, 1));
	assert_false(same(a, 1, raw, 1, a, 1, raw_other, 1));
}

/* ------------------------------------------------------------------------ */
/* obs_nix_portal_sets_unchanged without a session                          */

static bool unchanged_without_session(const obs_key_combination_t *current, size_t num,
				      const obs_key_combination_t *declined, size_t num_declined)
{
	struct obs_nix_portal_combos current_view = combos(current, num);
	struct obs_nix_portal_combos declined_view = combos(declined, num_declined);
	/* bound and the descriptions are not looked at without a session. */
	const char *const xyz[] = {"x", "y", "z"};
	struct obs_nix_portal_descriptions current_desc = descriptions(xyz, 3);
	const obs_key_combination_t c[] = {C};
	struct obs_nix_portal_combos bound = combos(c, 1);
	return obs_nix_portal_sets_unchanged(false, false, false, &current_view, &current_desc, &bound,
					     &no_descriptions, &declined_view);
}

static void test_unchanged_without_session(void **state)
{
	UNUSED_PARAMETER(state);

	const obs_key_combination_t a[] = {A};
	const obs_key_combination_t ab[] = {A, B};
	const obs_key_combination_t ba[] = {B, A};
	const obs_key_combination_t aa[] = {A, A};
	const obs_key_combination_t abc[] = {A, B, C};
	const obs_key_combination_t ctrl_a[] = {CTRL_A};

	/* No bindings: nothing to bind. */
	assert_true(unchanged_without_session(NULL, 0, NULL, 0));
	assert_true(unchanged_without_session(NULL, 0, ab, 2));
	/* Nothing declined: bind. */
	assert_false(unchanged_without_session(a, 1, NULL, 0));
	/* Every combination declined before: not asked for again. */
	assert_true(unchanged_without_session(a, 1, a, 1));
	assert_true(unchanged_without_session(ab, 2, ba, 2));
	assert_true(unchanged_without_session(a, 1, abc, 3));
	assert_true(unchanged_without_session(aa, 2, a, 1));
	/* A new combination: asked for. */
	assert_false(unchanged_without_session(ab, 2, a, 1));
	assert_false(unchanged_without_session(a, 1, ctrl_a, 1));
}

/* ------------------------------------------------------------------------ */
/* obs_nix_portal_sets_unchanged: the flags                                  */

/* Every combination of session, retry and partly_declined, for a current
 * set that is the bound set and also all declined (so each branch alone
 * says unchanged), and for one that is neither. */
static void test_unchanged_flags(void **state)
{
	UNUSED_PARAMETER(state);

	const obs_key_combination_t ab[] = {A, B};
	const char *const xy[] = {"x", "y"};
	const obs_key_combination_t c[] = {C};
	const char *const z[] = {"z"};

	struct obs_nix_portal_combos current = combos(ab, 2);
	struct obs_nix_portal_descriptions current_desc = descriptions(xy, 2);
	struct obs_nix_portal_combos other = combos(c, 1);
	struct obs_nix_portal_descriptions other_desc = descriptions(z, 1);

	for (int flags = 0; flags < 8; flags++) {
		bool session = flags & 1, retry = flags & 2, partly = flags & 4;
		/* Only a retry of a partly declined session binds anew. */
		bool want = !(session && retry && partly);
		bool got = obs_nix_portal_sets_unchanged(session, retry, partly, &current, &current_desc, &current,
							 &current_desc, &current);
		if (got != want) {
			fprintf(stderr, "same set, session %d retry %d partly %d: unchanged %d, expected %d\n", session,
				retry, partly, got, want);
			fail();
		}
		got = obs_nix_portal_sets_unchanged(session, retry, partly, &current, &current_desc, &other,
						    &other_desc, &other);
		if (got) {
			fprintf(stderr, "other set, session %d retry %d partly %d: unchanged\n", session, retry,
				partly);
			fail();
		}
	}

	/* No bindings and no session: unchanged, whatever the flags. */
	for (int flags = 0; flags < 4; flags++) {
		bool retry = flags & 1, partly = flags & 2;
		assert_true(obs_nix_portal_sets_unchanged(false, retry, partly, &no_combos, &no_descriptions, &other,
							  &other_desc, &no_combos));
	}
}

/* ------------------------------------------------------------------------ */
/* obs_nix_portal_sets_request                                               */

static void expect_positions(const size_t *got, size_t num_got, const size_t *want, size_t num_want, const char *what)
{
	bool equal = num_got == num_want;
	for (size_t i = 0; equal && i < num_got; i++)
		equal = got[i] == want[i];
	if (equal)
		return;
	fprintf(stderr, "%s: got {", what);
	for (size_t i = 0; i < num_got; i++)
		fprintf(stderr, "%s%zu", i ? ", " : "", got[i]);
	fprintf(stderr, "}, expected {");
	for (size_t i = 0; i < num_want; i++)
		fprintf(stderr, "%s%zu", i ? ", " : "", want[i]);
	fprintf(stderr, "}\n");
	fail();
}

#define SENTINEL ((size_t)0xdeadbeef)

/* Asks for current with partly_declined and declined; checks the positions
 * and the return value, and that nothing past current's size was written. */
static void expect_request(const obs_key_combination_t *current, size_t num, bool partly_declined,
			   const obs_key_combination_t *declined, size_t num_declined, const size_t *want,
			   size_t num_want, bool want_partly, const char *what)
{
	size_t request[8];
	for (size_t i = 0; i < COUNT(request); i++)
		request[i] = SENTINEL;
	size_t num_request = SENTINEL;
	struct obs_nix_portal_combos current_view = combos(current, num);
	struct obs_nix_portal_combos declined_view = combos(declined, num_declined);
	bool partly =
		obs_nix_portal_sets_request(&current_view, partly_declined, &declined_view, request, &num_request);
	expect_positions(request, num_request, want, num_want, what);
	if (partly != want_partly) {
		fprintf(stderr, "%s: partly %d, expected %d\n", what, partly, want_partly);
		fail();
	}
	for (size_t i = num; i < COUNT(request); i++)
		assert_true(request[i] == SENTINEL);
}

static void test_request(void **state)
{
	UNUSED_PARAMETER(state);

	const obs_key_combination_t abc[] = {A, B, C};
	const obs_key_combination_t b[] = {B};
	const obs_key_combination_t ca[] = {C, A};
	const obs_key_combination_t cab[] = {C, A, B};
	const obs_key_combination_t aab[] = {A, A, B};
	const obs_key_combination_t a[] = {A};
	const obs_key_combination_t ctrl_a_a[] = {CTRL_A, A};
	const obs_key_combination_t ctrl_a[] = {CTRL_A};
	const size_t all3[] = {0, 1, 2};
	const size_t p1[] = {1};
	const size_t p02[] = {0, 2};
	const size_t p2[] = {2};
	const size_t p0[] = {0};

	/* Not partly declined: all of current, declined or not. */
	expect_request(abc, 3, false, NULL, 0, all3, 3, false, "all, nothing declined");
	expect_request(abc, 3, false, abc, 3, all3, 3, false, "all, declined ignored");
	expect_request(NULL, 0, false, NULL, 0, NULL, 0, false, "empty");

	/* Partly declined: the declined ones are left out, in current's order. */
	expect_request(abc, 3, true, b, 1, p02, 2, true, "B declined");
	expect_request(abc, 3, true, ca, 2, p1, 1, true, "C and A declined");
	expect_request(abc, 3, true, NULL, 0, all3, 3, false, "nothing declined");
	expect_request(abc, 3, true, cab, 3, NULL, 0, true, "everything declined");
	/* Each copy of a declined duplicate. */
	expect_request(aab, 3, true, a, 1, p2, 1, true, "duplicate declined");
	/* The same key with other modifiers is not declined. */
	expect_request(ctrl_a_a, 2, true, ctrl_a, 1, p1, 1, true, "Ctrl+A declined");
	expect_request(ctrl_a_a, 2, true, a, 1, p0, 1, true, "A declined");
	expect_request(NULL, 0, true, a, 1, NULL, 0, false, "empty, partly declined");
}

/* ------------------------------------------------------------------------ */
/* obs_nix_portal_sets_partition                                             */

static void expect_partition(const obs_key_combination_t *current, size_t num, const obs_key_combination_t *approved,
			     size_t num_approved, const obs_key_combination_t *request, size_t num_request,
			     const size_t *want_part, size_t num_want_part, const size_t *want_declined,
			     size_t num_want_declined, bool want_bind, const char *what)
{
	size_t part[8], declined[8];
	for (size_t i = 0; i < COUNT(part); i++)
		part[i] = declined[i] = SENTINEL;
	size_t num_part = SENTINEL, num_declined = SENTINEL;
	struct obs_nix_portal_combos current_view = combos(current, num);
	struct obs_nix_portal_combos approved_view = combos(approved, num_approved);
	struct obs_nix_portal_combos request_view = combos(request, num_request);
	bool bind = obs_nix_portal_sets_partition(&current_view, &approved_view, &request_view, part, &num_part,
						  declined, &num_declined);

	char label[128];
	snprintf(label, sizeof(label), "%s: part", what);
	expect_positions(part, num_part, want_part, num_want_part, label);
	snprintf(label, sizeof(label), "%s: declined", what);
	expect_positions(declined, num_declined, want_declined, num_want_declined, label);
	if (bind != want_bind) {
		fprintf(stderr, "%s: bind %d, expected %d\n", what, bind, want_bind);
		fail();
	}
	/* Each has room for current's size, and nothing past it is written. */
	for (size_t i = num; i < COUNT(part); i++) {
		assert_true(part[i] == SENTINEL);
		assert_true(declined[i] == SENTINEL);
	}
}

static void test_partition(void **state)
{
	UNUSED_PARAMETER(state);

	const obs_key_combination_t a[] = {A};
	const obs_key_combination_t b[] = {B};
	const obs_key_combination_t ab[] = {A, B};
	const obs_key_combination_t ac[] = {A, C};
	const obs_key_combination_t abc[] = {A, B, C};
	const obs_key_combination_t ca[] = {C, A};
	const obs_key_combination_t ctrl_a[] = {CTRL_A};
	const obs_key_combination_t ctrl_a_b[] = {CTRL_A, B};
	const size_t p0[] = {0};
	const size_t p1[] = {1};
	const size_t p01[] = {0, 1};
	const size_t p02[] = {0, 2};
	const size_t p12[] = {1, 2};

	/* A approved before, B new and declined: A is bound again. */
	expect_partition(ab, 2, a, 1, ab, 2, p0, 1, p1, 1, true, "new key declined");
	/* Part keeps current's order, not approved's. */
	expect_partition(abc, 3, ca, 2, abc, 3, p02, 2, p1, 1, true, "order");
	/* Nothing approved before: nothing to bind, all declined. */
	expect_partition(ab, 2, NULL, 0, ab, 2, NULL, 0, p01, 2, false, "nothing approved");
	expect_partition(ab, 2, ctrl_a, 1, ab, 2, NULL, 0, p01, 2, false, "other modifiers approved");
	/* Every requested key approved before (only a description changed):
	 * part is the whole request, so nothing is bound again. */
	expect_partition(ab, 2, ab, 2, ab, 2, p01, 2, NULL, 0, false, "whole request approved");
	/* An approved key left out of the request (declined while others
	 * stayed bound) is not in part. */
	expect_partition(abc, 3, ab, 2, ac, 2, p0, 1, p12, 2, true, "approved, not requested");
	expect_partition(ab, 2, ab, 2, b, 1, p1, 1, p0, 1, false, "approved, not requested, rest of request");
	/* Nothing current. */
	expect_partition(NULL, 0, ab, 2, ab, 2, NULL, 0, NULL, 0, false, "nothing current");
	/* The same key with other modifiers. */
	expect_partition(ctrl_a_b, 2, ab, 2, ctrl_a_b, 2, p1, 1, p0, 1, true, "Ctrl+A is not A");
}

/* Characterized, not endorsed: |part| counts duplicates in current and
 * |request| is taken as given, also with combinations that are not in
 * current. The portal client never passes either: its bindings hold each
 * combination once, and its request is a subset of them. */
static void test_partition_counts(void **state)
{
	UNUSED_PARAMETER(state);

	const obs_key_combination_t a[] = {A};
	const obs_key_combination_t ab[] = {A, B};
	const obs_key_combination_t aab[] = {A, A, B};
	const obs_key_combination_t abc[] = {A, B, C};
	const size_t p0[] = {0};
	const size_t p01[] = {0, 1};
	const size_t p2[] = {2};

	/* Two copies of approved A make part as large as the request. */
	expect_partition(aab, 3, a, 1, ab, 2, p01, 2, p2, 1, false, "duplicate approved");
	/* A request with C, which is not current. */
	expect_partition(a, 1, a, 1, abc, 3, p0, 1, NULL, 0, true, "request beyond current");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(test_has, setup, teardown),
		cmocka_unit_test_setup_teardown(test_same_set_empty, setup, teardown),
		cmocka_unit_test_setup_teardown(test_same_set_combinations, setup, teardown),
		cmocka_unit_test_setup_teardown(test_same_set_duplicates, setup, teardown),
		cmocka_unit_test_setup_teardown(test_same_set_short_descriptions, setup, teardown),
		cmocka_unit_test_setup_teardown(test_same_set_null_description, setup, teardown),
		cmocka_unit_test_setup_teardown(test_same_set_description_bytes, setup, teardown),
		cmocka_unit_test_setup_teardown(test_unchanged_without_session, setup, teardown),
		cmocka_unit_test_setup_teardown(test_unchanged_flags, setup, teardown),
		cmocka_unit_test_setup_teardown(test_request, setup, teardown),
		cmocka_unit_test_setup_teardown(test_partition, setup, teardown),
		cmocka_unit_test_setup_teardown(test_partition_counts, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
