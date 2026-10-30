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
 * Characterization tests for the status decisions of the GlobalShortcuts
 * portal client in libobs/obs-nix-portal-shortcut-status.c:
 * obs_nix_portal_status_action and _more (how a shortcut's description
 * names its actions), _has_description (the "OBS" fallback), _rebind and
 * _keep_approved (bind_approved() after a declined dialog), _after_bind
 * and _declined (what sync_shortcuts() does after its bind), _bound and
 * _desktop_triggers (what the desktop's triggers mark bound), and
 * _can_configure (the desktop's shortcut editor). They
 * pin the current C behavior for the Rust port; cases marked
 * "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs. This test calls only them and
 * compiles the C source: no GIO, no D-Bus and no libobs core.
 * test_portal_shortcuts covers the portal client that uses them, end to
 * end.
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
	/* The decisions take and return plain values. */
	if (bnum_allocs() != allocs_before) {
		fprintf(stderr, "allocations: %ld before, %ld after\n", allocs_before, bnum_allocs());
		return -1;
	}
	return 0;
}

/* The values cross to the Rust port as they are. */
static void test_values(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(OBS_NIX_PORTAL_ACTION_FIRST, 0);
	assert_int_equal(OBS_NIX_PORTAL_ACTION_NEXT, 1);
	assert_int_equal(OBS_NIX_PORTAL_ACTION_UNNAMED, 2);
	assert_int_equal(OBS_NIX_PORTAL_REBIND_NONE, 0);
	assert_int_equal(OBS_NIX_PORTAL_REBIND_KEYS_DECLINED, 1);
	assert_int_equal(OBS_NIX_PORTAL_REBIND_DESCRIPTION_DECLINED, 2);
	assert_int_equal(OBS_NIX_PORTAL_CHANGE_KEEP, 0);
	assert_int_equal(OBS_NIX_PORTAL_CHANGE_SET, 1);
	assert_int_equal(OBS_NIX_PORTAL_CHANGE_CLEAR, 2);
}

/* The count-th action of a combination, counted from 1 as collect_binding()
 * counts them. */
static void test_action_counts(void **state)
{
	UNUSED_PARAMETER(state);

	/* Characterized, not endorsed: 0 is named after ", ", but the client
	 * counts before it asks, so it never asks for 0. */
	assert_int_equal(obs_nix_portal_status_action(0), OBS_NIX_PORTAL_ACTION_NEXT);
	assert_int_equal(obs_nix_portal_status_action(1), OBS_NIX_PORTAL_ACTION_FIRST);
	assert_int_equal(obs_nix_portal_status_action(2), OBS_NIX_PORTAL_ACTION_NEXT);
	assert_int_equal(obs_nix_portal_status_action(3), OBS_NIX_PORTAL_ACTION_NEXT);
	assert_int_equal(obs_nix_portal_status_action(4), OBS_NIX_PORTAL_ACTION_UNNAMED);
	assert_int_equal(obs_nix_portal_status_action(5), OBS_NIX_PORTAL_ACTION_UNNAMED);
	assert_int_equal(obs_nix_portal_status_action(6), OBS_NIX_PORTAL_ACTION_UNNAMED);
	assert_int_equal(obs_nix_portal_status_action(SIZE_MAX), OBS_NIX_PORTAL_ACTION_UNNAMED);

	assert_int_equal(obs_nix_portal_status_more(0), 0);
	assert_int_equal(obs_nix_portal_status_more(1), 0);
	assert_int_equal(obs_nix_portal_status_more(2), 0);
	assert_int_equal(obs_nix_portal_status_more(3), 0);
	assert_int_equal(obs_nix_portal_status_more(4), 1);
	assert_int_equal(obs_nix_portal_status_more(5), 2);
	assert_int_equal(obs_nix_portal_status_more(6), 3);
	assert_true(obs_nix_portal_status_more(SIZE_MAX) == SIZE_MAX - 3);
}

/* The description collect_binding() and collect_bindings() build for a
 * combination with count actions, from the decisions alone. */
static void describe(size_t count, char *out, size_t size)
{
	snprintf(out, size, "OBS:");
	for (size_t n = 1; n <= count; n++) {
		enum obs_nix_portal_action action = obs_nix_portal_status_action(n);
		if (action == OBS_NIX_PORTAL_ACTION_UNNAMED)
			continue;
		size_t len = strlen(out);
		snprintf(out + len, size - len, "%sa%zu", action == OBS_NIX_PORTAL_ACTION_FIRST ? " " : ", ", n);
	}
	size_t more = obs_nix_portal_status_more(count);
	if (more) {
		size_t len = strlen(out);
		snprintf(out + len, size - len, " and %zu more", more);
	}
}

static void test_descriptions(void **state)
{
	UNUSED_PARAMETER(state);

	static const char *const expected[] = {
		"OBS:",
		"OBS: a1",
		"OBS: a1, a2",
		"OBS: a1, a2, a3",
		"OBS: a1, a2, a3 and 1 more",
		"OBS: a1, a2, a3 and 2 more",
		"OBS: a1, a2, a3 and 3 more",
	};
	char description[128];
	for (size_t count = 0; count < sizeof(expected) / sizeof(expected[0]); count++) {
		describe(count, description, sizeof(description));
		assert_string_equal(description, expected[count]);
	}
}

/* take_combos() describes a combination without a description "OBS";
 * approved_description() reads only while both arrays have entries. */
static void test_has_description(void **state)
{
	UNUSED_PARAMETER(state);

	assert_false(obs_nix_portal_status_has_description(0, 0));
	assert_true(obs_nix_portal_status_has_description(0, 1));
	assert_false(obs_nix_portal_status_has_description(1, 1));
	assert_true(obs_nix_portal_status_has_description(1, 3));
	assert_false(obs_nix_portal_status_has_description(3, 3));
	assert_false(obs_nix_portal_status_has_description(SIZE_MAX, SIZE_MAX));
	assert_true(obs_nix_portal_status_has_description(SIZE_MAX - 1, SIZE_MAX));

	/* Three approved combinations with two descriptions: approved_description()
	 * stops after the second, so the third is taken as "OBS". */
	size_t compared = 0;
	for (size_t i = 0; obs_nix_portal_status_has_description(i, 3) && obs_nix_portal_status_has_description(i, 2);
	     i++)
		compared++;
	assert_int_equal(compared, 2);
}

static void test_rebind(void **state)
{
	UNUSED_PARAMETER(state);

	/* Nothing approved before is in the request: nothing is bound again. */
	assert_int_equal(obs_nix_portal_status_rebind(0, 0), OBS_NIX_PORTAL_REBIND_NONE);
	assert_int_equal(obs_nix_portal_status_rebind(0, 2), OBS_NIX_PORTAL_REBIND_NONE);
	/* New keys were declined: the approved part is bound again. */
	assert_int_equal(obs_nix_portal_status_rebind(1, 1), OBS_NIX_PORTAL_REBIND_KEYS_DECLINED);
	assert_int_equal(obs_nix_portal_status_rebind(3, 5), OBS_NIX_PORTAL_REBIND_KEYS_DECLINED);
	/* No key is new, only a description changed: the approved keys are
	 * bound again with the descriptions they were approved with. */
	assert_int_equal(obs_nix_portal_status_rebind(1, 0), OBS_NIX_PORTAL_REBIND_DESCRIPTION_DECLINED);
	assert_int_equal(obs_nix_portal_status_rebind(SIZE_MAX, 0), OBS_NIX_PORTAL_REBIND_DESCRIPTION_DECLINED);
	assert_int_equal(obs_nix_portal_status_rebind(SIZE_MAX, SIZE_MAX), OBS_NIX_PORTAL_REBIND_KEYS_DECLINED);
}

static void test_keep_approved(void **state)
{
	UNUSED_PARAMETER(state);

	/* Only a request the desktop declined, and that asked for something. */
	assert_true(obs_nix_portal_status_keep_approved(true, false));
	assert_false(obs_nix_portal_status_keep_approved(true, true));
	assert_false(obs_nix_portal_status_keep_approved(false, false));
	assert_false(obs_nix_portal_status_keep_approved(false, true));
}

static void expect_outcome(struct obs_nix_portal_outcome outcome, bool mark_declined, bool clear_declined,
			   bool set_partly, bool partly)
{
	assert_int_equal(outcome.mark_declined, mark_declined);
	assert_int_equal(outcome.clear_declined, clear_declined);
	assert_int_equal(outcome.set_partly, set_partly);
	assert_int_equal(outcome.partly, partly);
}

/* After a failed bind. partly is not read on this path. */
static void test_after_failed_bind(void **state)
{
	UNUSED_PARAMETER(state);

	for (int partly = 0; partly < 2; partly++) {
		/* bind_approved() bound the approved part again and set the
		 * status itself: nothing more changes. */
		expect_outcome(obs_nix_portal_status_after_bind(false, partly, true), false, false, false, false);
		/* Otherwise every current combination becomes declined, and the
		 * partly-declined flag is cleared. */
		expect_outcome(obs_nix_portal_status_after_bind(false, partly, false), true, false, true, false);
	}
}

/* The state a failed bind that kept nothing shows. */
static void test_declined(void **state)
{
	UNUSED_PARAMETER(state);

	/* Declined by the desktop: declined. */
	assert_true(obs_nix_portal_status_declined(true, false));
	/* Nothing to ask for (every combination declined before): declined. */
	assert_true(obs_nix_portal_status_declined(false, true));
	assert_true(obs_nix_portal_status_declined(true, true));
	/* An error: failed, unless the portal left the bus or the client is
	 * stopping (the client reads those two only then). */
	assert_false(obs_nix_portal_status_declined(false, false));
}

/* After a bind. kept is not read on this path. */
static void test_after_bind(void **state)
{
	UNUSED_PARAMETER(state);

	for (int kept = 0; kept < 2; kept++) {
		/* The whole request: nothing is left declined. */
		expect_outcome(obs_nix_portal_status_after_bind(true, false, kept), false, true, true, false);
		/* Only part of it (keys declined before are left out until Ask
		 * again): they stay declined, and the status says so. */
		expect_outcome(obs_nix_portal_status_after_bind(true, true, kept), false, false, true, true);
	}
}

/* A desktop that picks the keys: OBS_HOTKEYS_BACKGROUND_CONFIGURABLE
 * (portal version 2, KDE), or one that reports triggers (GNOME). A shortcut
 * without a trigger is not bound (left without a key in the dialog, or the
 * dialog was cancelled before), on a bind reply and on ShortcutsChanged. */
static void test_trigger_desktop_keys(void **state)
{
	UNUSED_PARAMETER(state);

	for (int reply = 0; reply < 2; reply++) {
		assert_int_equal(obs_nix_portal_status_bound(true, reply, true), OBS_NIX_PORTAL_CHANGE_SET);
		assert_int_equal(obs_nix_portal_status_bound(true, reply, false), OBS_NIX_PORTAL_CHANGE_CLEAR);
		assert_int_equal(obs_nix_portal_status_desktop_triggers(true, reply), OBS_NIX_PORTAL_CHANGE_SET);
	}
}

/* A desktop that reports no trigger and has no editor (Hyprland): every
 * returned shortcut is bound on a bind reply, with the keys from its
 * config; ShortcutsChanged changes nothing. */
static void test_trigger_no_desktop_keys(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(obs_nix_portal_status_bound(false, true, false), OBS_NIX_PORTAL_CHANGE_SET);
	assert_int_equal(obs_nix_portal_status_bound(false, false, false), OBS_NIX_PORTAL_CHANGE_KEEP);
	assert_int_equal(obs_nix_portal_status_desktop_triggers(false, true), OBS_NIX_PORTAL_CHANGE_CLEAR);
	assert_int_equal(obs_nix_portal_status_desktop_triggers(false, false), OBS_NIX_PORTAL_CHANGE_KEEP);

	/* Characterized, not endorsed: a trigger on a desktop that does not
	 * pick the keys is ignored. The client never asks: any trigger makes
	 * it a desktop that picks the keys. */
	assert_int_equal(obs_nix_portal_status_bound(false, true, true), OBS_NIX_PORTAL_CHANGE_SET);
	assert_int_equal(obs_nix_portal_status_bound(false, false, true), OBS_NIX_PORTAL_CHANGE_KEEP);
}

static void test_configure(void **state)
{
	UNUSED_PARAMETER(state);

	static const uint32_t versions[] = {0, 1, 2, 3, UINT32_MAX};
	for (size_t v = 0; v < sizeof(versions) / sizeof(versions[0]); v++) {
		uint32_t version = versions[v];
		bool editor = version >= 2;
		/* take_configure() takes a pending request with a session, or
		 * without one when the portal has no editor (then
		 * configure_shortcuts() refuses it); otherwise it waits for the
		 * bind a probe is about to make. */
		assert_int_equal(obs_nix_portal_status_can_configure(version), editor);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(test_values, setup, teardown),
		cmocka_unit_test_setup_teardown(test_action_counts, setup, teardown),
		cmocka_unit_test_setup_teardown(test_descriptions, setup, teardown),
		cmocka_unit_test_setup_teardown(test_has_description, setup, teardown),
		cmocka_unit_test_setup_teardown(test_rebind, setup, teardown),
		cmocka_unit_test_setup_teardown(test_keep_approved, setup, teardown),
		cmocka_unit_test_setup_teardown(test_after_failed_bind, setup, teardown),
		cmocka_unit_test_setup_teardown(test_declined, setup, teardown),
		cmocka_unit_test_setup_teardown(test_after_bind, setup, teardown),
		cmocka_unit_test_setup_teardown(test_trigger_desktop_keys, setup, teardown),
		cmocka_unit_test_setup_teardown(test_trigger_no_desktop_keys, setup, teardown),
		cmocka_unit_test_setup_teardown(test_configure, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
