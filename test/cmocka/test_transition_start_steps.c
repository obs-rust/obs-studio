#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs.h"
#include "obs-source-transition-start.h"

/*
 * Characterization of the start checks and child handover of the
 * transitions in libobs/obs-source-transition.c, moved to
 * libobs/obs-source-transition-start.c before their Rust port:
 * - obs_source_transition_start_refused(), _walks(), _admit(),
 *   _duration_ns() and _null_dest(): the steps of obs_transition_start()
 *   between its three lock sections;
 * - obs_source_transition_start_restart(), _adds(), _removes(),
 *   _calls_back() and _unsized(): set_source();
 * - obs_source_transition_start_activates(): activate_transition();
 * - obs_source_transition_start_active_index():
 *   obs_transition_get_active_source() and obs_transition_save();
 * - obs_source_transition_start_manual_reset():
 *   obs_transition_set_manual_time();
 * - obs_source_transition_start_child_active(): obs_transition_set() and
 *   copy_source_state().
 * Each gate is pinned over every combination of its inputs. The last cases
 * run copies of obs_transition_start(), set_source(), activate_transition(),
 * obs_transition_set(), obs_transition_set_manual_time() and the swap on a
 * stand-in transition that counts each child's references and active
 * references, so the composition is pinned too: the null destination, a
 * re-entering transition_start callback, a failed add, the fixed duration,
 * the manual times, the swap and the set. Cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_transition_recursive_destination,
 * test_transition_swap, test_studio_mode_swap, test_studio_mode_exit,
 * test_saved_transition, test_tbar_position, test_fade_to_color_mix,
 * test_visibility_transition and test_stinger_media_duration cover the
 * callers through the real libobs.
 */

#define MANUAL OBS_SOURCE_TRANSITION_START_MODE_MANUAL
#define AUTO 0 /* OBS_TRANSITION_MODE_AUTO */

#define GO OBS_SOURCE_TRANSITION_START_GO
#define REFUSE OBS_SOURCE_TRANSITION_START_REFUSE
#define DONE OBS_SOURCE_TRANSITION_START_DONE

static const int MODES[] = {AUTO, MANUAL, 2, -1, INT32_MIN, INT32_MAX};
#define MODE_COUNT (sizeof(MODES) / sizeof(MODES[0]))

static void constants(void **state)
{
	(void)state;

	/* the header's mirror of the core */
	assert_int_equal(OBS_SOURCE_TRANSITION_START_MODE_MANUAL, (int)OBS_TRANSITION_MODE_MANUAL);
	assert_int_equal(AUTO, (int)OBS_TRANSITION_MODE_AUTO);
	assert_int_equal(sizeof(enum obs_transition_mode), sizeof(int));
	assert_int_equal((int)GO, 0);
	assert_int_equal((int)REFUSE, 1);
	assert_int_equal((int)DONE, 2);
}

static void refused_and_walks(void **state)
{
	(void)state;

	/* the first refusal: the destination is source A and nothing runs */
	assert_true(obs_source_transition_start_refused(true, false));
	assert_false(obs_source_transition_start_refused(true, true));
	assert_false(obs_source_transition_start_refused(false, false));
	assert_false(obs_source_transition_start_refused(false, true));

	/* the walk runs for a destination that is not the transition itself */
	assert_true(obs_source_transition_start_walks(true, false));
	assert_false(obs_source_transition_start_walks(true, true));
	assert_false(obs_source_transition_start_walks(false, false));
	assert_false(obs_source_transition_start_walks(false, true));
}

static void admit_every_combination(void **state)
{
	(void)state;

	for (int bits = 0; bits < 16; bits++) {
		bool same_as_source = bits & 1;
		bool same_as_dest = bits & 2;
		bool same_mode = bits & 4;
		bool active = bits & 8;

		for (size_t m = 0; m < MODE_COUNT; m++) {
			int mode = MODES[m];
			enum obs_source_transition_start_admission want = GO;
			if (same_as_source && !active)
				want = REFUSE;
			else if (active && mode == MANUAL && same_mode && same_as_dest)
				want = DONE;
			enum obs_source_transition_start_admission got = obs_source_transition_start_admit(
				same_as_source, same_as_dest, same_mode, mode, active);
			if (got != want) {
				fprintf(stderr, "admit(%d, %d, %d, %d, %d) = %d, expected %d\n", same_as_source,
					same_as_dest, same_mode, mode, active, (int)got, (int)want);
				fail();
			}
		}
	}

	/* the refusal wins: source A that is also B, manual, not active */
	assert_int_equal(obs_source_transition_start_admit(true, true, true, MANUAL, false), REFUSE);
	/* source A that is also B while running manually: already there */
	assert_int_equal(obs_source_transition_start_admit(true, true, true, MANUAL, true), DONE);
	/* an auto start to the running destination goes again */
	assert_int_equal(obs_source_transition_start_admit(false, true, true, AUTO, true), GO);
	/* manual to the running destination from auto mode goes again */
	assert_int_equal(obs_source_transition_start_admit(false, true, false, MANUAL, true), GO);
	/* manual to a new destination goes */
	assert_int_equal(obs_source_transition_start_admit(false, false, true, MANUAL, true), GO);
}

static void duration(void **state)
{
	(void)state;

	assert_int_equal(obs_source_transition_start_duration_ns(0), 0);
	assert_int_equal(obs_source_transition_start_duration_ns(1), 1000000ULL);
	assert_int_equal(obs_source_transition_start_duration_ns(300), 300000000ULL);
	assert_int_equal(obs_source_transition_start_duration_ns(4294), 4294000000ULL);
	/* past 32 bits: the product is taken in 64 bits */
	assert_int_equal(obs_source_transition_start_duration_ns(4295), 4295000000ULL);
	assert_int_equal(obs_source_transition_start_duration_ns(UINT32_MAX), 4294967295000000ULL);
}

static void set_source_rules(void **state)
{
	(void)state;

	for (int bits = 0; bits < 8; bits++) {
		bool a = bits & 1, b = bits & 2, c = bits & 4;
		/* restart(active, same_as_dest, same_as_source) */
		assert_int_equal(obs_source_transition_start_restart(a, b, c), !a || (!b && !c));
		/* removes(already_active, has_old, add_ok) */
		assert_int_equal(obs_source_transition_start_removes(a, b, c), a && b && c);
		/* null_dest(has_dest, same_as_dest, same_as_source) */
		assert_int_equal(obs_source_transition_start_null_dest(a, b, c), !a && b && !c);
	}
	for (int bits = 0; bits < 4; bits++) {
		bool a = bits & 1, b = bits & 2;
		assert_int_equal(obs_source_transition_start_adds(a, b), a && b);
		assert_int_equal(obs_source_transition_start_calls_back(a, b), a && b);
		assert_int_equal(obs_source_transition_start_child_active(a, b), a && b);
	}

	/* a running transition restarts its clock only for a new child that is
	 * not source A either */
	assert_true(obs_source_transition_start_restart(true, false, false));
	assert_false(obs_source_transition_start_restart(true, true, false));
	assert_false(obs_source_transition_start_restart(true, false, true));
	/* an idle one always restarts */
	assert_true(obs_source_transition_start_restart(false, true, true));

	assert_true(obs_source_transition_start_unsized(0));
	assert_false(obs_source_transition_start_unsized(1));
	assert_false(obs_source_transition_start_unsized(1920));
	assert_false(obs_source_transition_start_unsized(UINT32_MAX));

	assert_true(obs_source_transition_start_activates(false));
	assert_false(obs_source_transition_start_activates(true));

	assert_int_equal(obs_source_transition_start_active_index(false), 0);
	assert_int_equal(obs_source_transition_start_active_index(true), 1);
}

static void manual_reset(void **state)
{
	(void)state;

	const float zero = 0.0f;
	const float minus_zero = -0.0f;
	const float nan = NAN;
	const float tiny = 1e-45f; /* the smallest denormal */

	assert_true(obs_source_transition_start_manual_reset(MANUAL, zero));
	/* -0.0 compares equal to 0.0: it resets */
	assert_true(obs_source_transition_start_manual_reset(MANUAL, minus_zero));
	/* NaN compares unequal: it does not */
	assert_false(obs_source_transition_start_manual_reset(MANUAL, nan));
	assert_false(obs_source_transition_start_manual_reset(MANUAL, 1.0f));
	assert_false(obs_source_transition_start_manual_reset(MANUAL, tiny));
	assert_false(obs_source_transition_start_manual_reset(MANUAL, -tiny));
	assert_false(obs_source_transition_start_manual_reset(MANUAL, -1.0f));
	assert_false(obs_source_transition_start_manual_reset(MANUAL, INFINITY));

	/* never outside manual mode */
	for (size_t m = 0; m < MODE_COUNT; m++) {
		if (MODES[m] == MANUAL)
			continue;
		assert_false(obs_source_transition_start_manual_reset(MODES[m], zero));
		assert_false(obs_source_transition_start_manual_reset(MODES[m], minus_zero));
		assert_false(obs_source_transition_start_manual_reset(MODES[m], nan));
	}
}

/* ---- the callers, copied onto a stand-in transition ---- */

struct child {
	const char *name;
	int refs;        /* obs_source_get_ref() minus obs_source_release() */
	int active_refs; /* obs_source_add_active_child() minus _remove_ */
	bool fail_add;   /* obs_source_add_active_child() fails (a cycle) */
	bool is_self;    /* the destination is the transition itself */
	bool contains;   /* the destination's tree contains the transition */
};

struct tr;
typedef void (*plugin_start_t)(struct tr *t);

struct tr {
	struct child *src[2];
	bool slot_active[2];
	bool audio, video;
	int mode;
	float val, target;
	uint64_t start_time, duration;
	bool use_fixed;
	uint32_t fixed_ms;
	uint32_t cx, cy;
	uint64_t now;
	int resizes, start_signals, stop_signals, warnings, plugin_calls;
	plugin_start_t plugin_start;
	/* what the re-entering plugin does */
	int nested_mode;
	uint32_t nested_ms;
	struct child *nested_dest;
	bool nested_result;
};

static struct child *get_ref(struct child *c)
{
	if (c)
		c->refs++;
	return c;
}

static void release(struct child *c)
{
	if (c)
		c->refs--;
}

/* obs_source_add_active_child(): false for NULL (obs_ptr_valid()) and for a
 * child that would make a cycle */
static bool add_active(struct child *c)
{
	if (!c || c->fail_add)
		return false;
	c->active_refs++;
	return true;
}

static void remove_active(struct child *c)
{
	c->active_refs--;
}

struct start_clock {
	uint64_t duration;
	bool same_as_source;
	bool same_as_dest;
	bool restart;
};

static void write_clock(struct tr *t, const struct start_clock *clock)
{
	if (clock->restart) {
		t->start_time = ++t->now;
		t->duration = clock->duration;
	}
}

static bool tr_active(struct tr *t)
{
	return t->audio || t->video;
}

static bool tr_set_source(struct tr *t, size_t idx, struct child *new_child,
			  bool (*callback)(struct tr *t, size_t idx, struct child *c), struct start_clock *clock)
{
	struct child *old_child;
	bool add_success = true;
	bool already_active;

	new_child = get_ref(new_child);
	old_child = t->src[idx];

	clock->same_as_source = new_child == t->src[0];
	clock->same_as_dest = new_child == old_child;
	clock->restart = obs_source_transition_start_restart(tr_active(t), clock->same_as_dest, clock->same_as_source);

	if (new_child == old_child) {
		write_clock(t, clock);
		release(new_child);
		return true;
	}

	already_active = t->slot_active[idx];
	if (obs_source_transition_start_adds(already_active, new_child != NULL))
		add_success = add_active(new_child);
	if (obs_source_transition_start_removes(already_active, old_child != NULL, add_success))
		remove_active(old_child);
	if (obs_source_transition_start_calls_back(callback != NULL, add_success))
		add_success = callback(t, idx, new_child);
	if (add_success) {
		write_clock(t, clock);
		t->src[idx] = new_child;
	}
	if (!add_success) {
		release(new_child);
		return false;
	}
	if (obs_source_transition_start_unsized(t->cx) || obs_source_transition_start_unsized(t->cy))
		t->resizes++;
	release(old_child);
	return true;
}

static bool tr_activate(struct tr *t, size_t idx, struct child *child)
{
	if (obs_source_transition_start_activates(t->slot_active[idx])) {
		if (!add_active(child))
			return false;
		t->slot_active[idx] = true;
	}
	t->video = true;
	t->audio = true;
	return true;
}

static void tr_set(struct tr *t, struct child *source)
{
	struct child *s[2];
	bool active[2];

	source = get_ref(source);
	if (tr_active(t))
		t->stop_signals++;
	for (size_t i = 0; i < 2; i++) {
		s[i] = t->src[i];
		active[i] = t->slot_active[i];
		t->src[i] = NULL;
		t->slot_active[i] = false;
	}
	t->slot_active[0] = true;
	t->src[0] = source;
	t->video = false;
	t->audio = false;
	t->val = 0.0f;
	t->target = 0.0f;
	for (size_t i = 0; i < 2; i++) {
		if (obs_source_transition_start_child_active(s[i] != NULL, active[i]))
			remove_active(s[i]);
		release(s[i]);
	}
	if (source)
		add_active(source);
}

static bool tr_start(struct tr *t, int mode, uint32_t duration_ms, struct child *dest)
{
	bool same_as_source = dest == t->src[0];
	bool active = tr_active(t);
	if (obs_source_transition_start_refused(same_as_source, active))
		return false;

	bool recursive = dest && dest->is_self;
	if (obs_source_transition_start_walks(dest != NULL, recursive))
		recursive = dest->contains;
	if (recursive) {
		t->warnings++;
		return false;
	}

	if (tr_active(t))
		tr_set(t, t->src[1]);

	same_as_source = dest == t->src[0];
	bool same_as_dest = dest == t->src[1];
	bool same_mode = mode == t->mode;
	active = tr_active(t);
	switch (obs_source_transition_start_admit(same_as_source, same_as_dest, same_mode, mode, active)) {
	case OBS_SOURCE_TRANSITION_START_REFUSE:
		return false;
	case OBS_SOURCE_TRANSITION_START_DONE:
		return true;
	case OBS_SOURCE_TRANSITION_START_GO:
		break;
	}

	t->mode = mode;
	t->val = 0.0f;
	t->target = 0.0f;

	if (t->plugin_start)
		t->plugin_start(t);

	if (t->use_fixed)
		duration_ms = t->fixed_ms;

	struct start_clock clock = {obs_source_transition_start_duration_ns(duration_ms), false, false, false};
	if (!tr_set_source(t, 1, dest, tr_activate, &clock)) {
		t->warnings++;
		return false;
	}
	if (obs_source_transition_start_null_dest(dest != NULL, clock.same_as_dest, clock.same_as_source)) {
		t->video = true;
		t->audio = true;
	}
	t->start_signals++;
	t->resizes++;
	return true;
}

static void tr_set_manual_time(struct tr *t, float f)
{
	t->target = f;
	if (obs_source_transition_start_manual_reset(t->mode, f))
		tr_set(t, t->src[0]);
}

static struct child *tr_get_active(struct tr *t)
{
	return t->src[obs_source_transition_start_active_index(tr_active(t))];
}

static struct child *copy_state(struct tr *dest, struct tr *source, size_t idx)
{
	struct child *old_child = dest->src[idx];
	struct child *new_child = get_ref(source->src[idx]);
	bool active = source->slot_active[idx];

	if (old_child && dest->slot_active[idx])
		remove_active(old_child);
	dest->src[idx] = new_child;
	dest->slot_active[idx] = active;
	if (obs_source_transition_start_child_active(new_child != NULL, active))
		add_active(new_child);
	return old_child;
}

/* obs_transition_swap_begin() and _end(); _end() clears the source */
static void tr_swap(struct tr *dest, struct tr *source)
{
	struct child *old_children[2];

	if (dest == source)
		return;
	for (size_t i = 0; i < 2; i++)
		old_children[i] = copy_state(dest, source, i);
	dest->video = source->video;
	dest->audio = source->audio;
	dest->start_time = source->start_time;
	dest->duration = source->duration;
	dest->mode = source->mode;
	dest->val = source->val;
	dest->target = source->target;
	for (size_t i = 0; i < 2; i++)
		release(old_children[i]);

	for (size_t i = 0; i < 2; i++) {
		struct child *s = source->src[i];
		bool active = source->slot_active[i];
		source->src[i] = NULL;
		source->slot_active[i] = false;
		if (s && active)
			remove_active(s);
		release(s);
	}
	source->video = false;
	source->audio = false;
}

/* a transition showing A, idle, as obs_transition_init() and a set leave it */
static void tr_init(struct tr *t, struct child *a)
{
	memset(t, 0, sizeof(*t));
	t->slot_active[0] = true;
	t->cx = 1920;
	t->cy = 1080;
	t->mode = AUTO;
	if (a)
		tr_set(t, a);
}

static void start_from_idle(void **state)
{
	(void)state;
	struct child a = {"A"}, b = {"B"};
	struct tr t;
	tr_init(&t, &a);
	assert_int_equal(a.refs, 1);
	assert_int_equal(a.active_refs, 1);

	assert_true(tr_start(&t, AUTO, 300, &b));
	assert_ptr_equal(t.src[1], &b);
	assert_true(t.slot_active[1]);
	assert_true(t.audio && t.video);
	assert_int_equal(t.duration, 300000000ULL);
	assert_int_equal(t.start_time, 1);
	assert_int_equal(b.refs, 1);
	assert_int_equal(b.active_refs, 1);
	assert_int_equal(t.start_signals, 1);
	/* sized: only the start's own recalculation */
	assert_int_equal(t.resizes, 1);
	assert_ptr_equal(tr_get_active(&t), &b);

	/* an unsized transition also recalculates in set_source() */
	struct tr u;
	struct child c = {"C"};
	tr_init(&u, &a);
	u.cy = 0;
	assert_true(tr_start(&u, AUTO, 300, &c));
	assert_int_equal(u.resizes, 2);
}

static void start_to_source_a_refused(void **state)
{
	(void)state;
	struct child a = {"A"};
	struct tr t;
	tr_init(&t, &a);

	assert_false(tr_start(&t, AUTO, 300, &a));
	assert_null(t.src[1]);
	assert_false(t.audio || t.video);
	assert_int_equal(t.start_signals, 0);
	assert_int_equal(t.mode, AUTO);

	/* the transition itself, or a tree holding it, is refused before
	 * anything changes */
	struct child self = {"self", 0, 0, false, true, false};
	struct child holder = {"holder", 0, 0, false, false, true};
	assert_false(tr_start(&t, MANUAL, 300, &self));
	assert_false(tr_start(&t, MANUAL, 300, &holder));
	assert_int_equal(t.warnings, 2);
	assert_int_equal(t.mode, AUTO);
	assert_int_equal(self.refs + holder.refs, 0);
}

static void start_while_running(void **state)
{
	(void)state;
	struct child a = {"A"}, b = {"B"}, c = {"C"};
	struct tr t;
	tr_init(&t, &a);
	assert_true(tr_start(&t, AUTO, 300, &b));

	/* the running transition finishes on B first, then goes to C */
	assert_true(tr_start(&t, AUTO, 500, &c));
	assert_int_equal(t.stop_signals, 1);
	assert_ptr_equal(t.src[0], &b);
	assert_ptr_equal(t.src[1], &c);
	assert_int_equal(a.refs, 0);
	assert_int_equal(a.active_refs, 0);
	assert_int_equal(b.refs, 1);
	assert_int_equal(b.active_refs, 1);
	assert_int_equal(c.active_refs, 1);
	assert_int_equal(t.duration, 500000000ULL);

	/* to B again (now source A after the finish) while running: the finish
	 * puts C in A, so B is a new destination */
	assert_true(tr_start(&t, AUTO, 700, &b));
	assert_ptr_equal(t.src[0], &c);
	assert_ptr_equal(t.src[1], &b);
	assert_int_equal(b.refs, 1);
	assert_int_equal(b.active_refs, 1);
}

static void null_destination(void **state)
{
	(void)state;
	struct child a = {"A"};
	struct tr t;

	/* with a source A: B was none, so the start to none runs, setting both
	 * flags itself (set_source() took its shortcut) */
	tr_init(&t, &a);
	assert_true(tr_start(&t, AUTO, 300, NULL));
	assert_true(t.audio && t.video);
	assert_null(t.src[1]);
	assert_false(t.slot_active[1]);
	assert_int_equal(t.duration, 300000000ULL);
	assert_ptr_equal(tr_get_active(&t), NULL);

	/* without a source A: the destination is source A, refused */
	struct tr e;
	tr_init(&e, NULL);
	assert_false(tr_start(&e, AUTO, 300, NULL));
	assert_false(e.audio || e.video);

	/* characterized, not endorsed: to none with a source B in a slot that
	 * is not active, activate_transition() adds NULL, which fails, and the
	 * start is refused with the recursion warning; libobs never leaves a
	 * source B in an inactive slot */
	struct child b = {"B"};
	struct tr f;
	tr_init(&f, &a);
	f.src[1] = get_ref(&b);
	f.slot_active[1] = false;
	assert_false(tr_start(&f, AUTO, 300, NULL));
	assert_int_equal(f.warnings, 1);
	assert_ptr_equal(f.src[1], &b);
	assert_false(f.audio || f.video);
}

static void manual_same_target(void **state)
{
	(void)state;
	struct child a = {"A"}, b = {"B"};
	struct tr t;
	tr_init(&t, &a);
	assert_true(tr_start(&t, MANUAL, 300, &b));
	t.val = 0.25f;
	t.target = 0.5f;

	/* characterized, not endorsed: the finish step (upstream 0df94d5ca)
	 * runs before the second read, so a running manual transition started
	 * again to the same B first finishes on B. B is then source A and
	 * nothing runs, so the start is refused: the transition jumps to B. */
	assert_false(tr_start(&t, MANUAL, 300, &b));
	assert_ptr_equal(t.src[0], &b);
	assert_null(t.src[1]);
	assert_false(t.audio || t.video);
	assert_int_equal(a.active_refs, 0);
	assert_int_equal(b.active_refs, 1);
	assert_int_equal(t.stop_signals, 1);

	/* DONE needs the transition still active at the second read, which
	 * only another thread's start between the finish and that read gives;
	 * the gate alone pins it */
	assert_int_equal(obs_source_transition_start_admit(false, true, true, MANUAL, true), DONE);
}

static void reenter_plugin(struct tr *t)
{
	t->plugin_calls++;
	if (t->plugin_calls == 1) {
		t->nested_result = tr_start(t, t->nested_mode, t->nested_ms, t->nested_dest);
	}
}

static void reentering_callback(void **state)
{
	(void)state;
	struct child a = {"A"}, b = {"B"}, c = {"C"};
	struct tr t;
	tr_init(&t, &a);
	t.plugin_start = reenter_plugin;
	t.nested_mode = MANUAL;
	t.nested_ms = 900;
	t.nested_dest = &c;

	/* the outer start writes AUTO, then the plugin starts this transition
	 * again, to C in manual mode, which completes. The outer one goes on to
	 * B: the transition is active and B is neither C nor source A, so the
	 * clock restarts with the outer duration */
	assert_true(tr_start(&t, AUTO, 300, &b));
	assert_true(t.nested_result);
	assert_int_equal(t.plugin_calls, 2);
	assert_ptr_equal(t.src[1], &b);
	assert_int_equal(t.duration, 300000000ULL);
	/* characterized, not endorsed: the mode the nested start wrote stays */
	assert_int_equal(t.mode, MANUAL);
	/* C was handed over: its active reference went with it */
	assert_int_equal(c.refs, 0);
	assert_int_equal(c.active_refs, 0);
	assert_int_equal(b.active_refs, 1);
	assert_int_equal(t.start_signals, 2);

	/* the nested start goes to the same E: the outer one finds E already
	 * in B while running, so set_source() takes its shortcut and the clock
	 * stays the nested one */
	struct tr u;
	struct child d = {"D"}, e = {"E"};
	tr_init(&u, &d);
	u.plugin_start = reenter_plugin;
	u.nested_mode = AUTO;
	u.nested_ms = 900;
	u.nested_dest = &e;
	assert_true(tr_start(&u, AUTO, 300, &e));
	assert_true(u.nested_result);
	assert_int_equal(u.duration, 900000000ULL);
	assert_int_equal(u.start_time, 1);
	assert_int_equal(e.refs, 1);
	assert_int_equal(e.active_refs, 1);
	assert_int_equal(u.start_signals, 2);
}

static void failed_add_keeps_nested_clock(void **state)
{
	(void)state;
	struct child a = {"A"}, b = {"B"}, c = {"C"};
	struct tr t;
	tr_init(&t, &a);
	t.plugin_start = reenter_plugin;
	t.nested_mode = AUTO;
	t.nested_ms = 900;
	t.nested_dest = &c;
	/* the outer destination makes a cycle the walk did not see (it was
	 * made by the plugin): the add fails late */
	b.fail_add = true;

	assert_false(tr_start(&t, AUTO, 300, &b));
	assert_true(t.nested_result);
	assert_int_equal(t.warnings, 1);
	/* the nested start's clock and destination stay */
	assert_ptr_equal(t.src[1], &c);
	assert_int_equal(t.duration, 900000000ULL);
	assert_int_equal(t.start_time, 1);
	assert_int_equal(b.refs, 0);
	assert_int_equal(b.active_refs, 0);
	assert_int_equal(c.active_refs, 1);
	/* and C stays active: the old child is removed only if the add worked */
	assert_true(t.slot_active[1]);
}

static void fixed_duration(void **state)
{
	(void)state;
	static const uint32_t MS[] = {0, 1, 300, UINT32_MAX};

	for (size_t f = 0; f < 4; f++) {
		for (size_t r = 0; r < 4; r++) {
			struct child a = {"A"}, b = {"B"};
			struct tr t;

			tr_init(&t, &a);
			t.use_fixed = true;
			t.fixed_ms = MS[f];
			assert_true(tr_start(&t, AUTO, MS[r], &b));
			assert_int_equal(t.duration, (uint64_t)MS[f] * 1000000ULL);

			tr_init(&t, &a);
			t.use_fixed = false;
			t.fixed_ms = MS[f];
			assert_true(tr_start(&t, AUTO, MS[r], &b));
			assert_int_equal(t.duration, (uint64_t)MS[r] * 1000000ULL);
		}
	}
}

static void manual_times(void **state)
{
	(void)state;
	const float times[] = {0.0f, -0.0f, NAN, 1.0f};
	const bool resets[] = {true, true, false, false};

	for (size_t i = 0; i < 4; i++) {
		struct child a = {"A"}, b = {"B"};
		struct tr t;
		tr_init(&t, &a);
		assert_true(tr_start(&t, MANUAL, 300, &b));
		t.val = 0.5f;

		tr_set_manual_time(&t, times[i]);
		if (resets[i]) {
			/* back to A, idle: B goes out */
			assert_false(t.audio || t.video);
			assert_ptr_equal(t.src[0], &a);
			assert_null(t.src[1]);
			assert_int_equal(b.refs, 0);
			assert_int_equal(b.active_refs, 0);
			assert_int_equal(a.active_refs, 1);
			assert_true(t.val == 0.0f);
		} else {
			assert_true(t.audio && t.video);
			assert_ptr_equal(t.src[1], &b);
			assert_true(t.val == 0.5f);
		}

		/* never in auto mode */
		struct tr u;
		tr_init(&u, &a);
		assert_true(tr_start(&u, AUTO, 300, &b));
		tr_set_manual_time(&u, times[i]);
		assert_true(u.audio && u.video);
	}
}

static void swap(void **state)
{
	(void)state;
	struct child a = {"A"}, b = {"B"};
	struct tr t, n;

	/* the same transition: nothing */
	tr_init(&t, &a);
	assert_true(tr_start(&t, AUTO, 300, &b));
	tr_swap(&t, &t);
	assert_ptr_equal(t.src[1], &b);
	assert_int_equal(b.active_refs, 1);
	assert_true(t.audio);

	/* a running one: its sources, slots and clock move over; each child
	 * keeps one reference and one active reference */
	tr_init(&n, NULL);
	tr_swap(&n, &t);
	assert_ptr_equal(n.src[0], &a);
	assert_ptr_equal(n.src[1], &b);
	assert_true(n.slot_active[0] && n.slot_active[1]);
	assert_true(n.audio && n.video);
	assert_int_equal(n.duration, 300000000ULL);
	assert_int_equal(a.refs, 1);
	assert_int_equal(a.active_refs, 1);
	assert_int_equal(b.refs, 1);
	assert_int_equal(b.active_refs, 1);
	assert_null(t.src[0]);
	assert_false(t.audio);

	/* an inactive slot: the child comes in without an active reference,
	 * and an old child in an inactive slot goes out without a removal */
	struct child c = {"C"}, d = {"D"};
	struct tr p, q;
	tr_init(&p, NULL);
	tr_init(&q, NULL);
	p.src[1] = get_ref(&c);
	p.slot_active[1] = false;
	q.src[1] = get_ref(&d);
	q.slot_active[1] = false;
	tr_swap(&q, &p);
	assert_ptr_equal(q.src[1], &c);
	assert_false(q.slot_active[1]);
	assert_int_equal(c.refs, 1);
	assert_int_equal(c.active_refs, 0);
	assert_int_equal(d.refs, 0);
	assert_int_equal(d.active_refs, 0);
}

static void set_active_and_idle(void **state)
{
	(void)state;
	struct child a = {"A"}, b = {"B"}, c = {"C"};
	struct tr t;

	/* while running: the stop signal, both children go out as active */
	tr_init(&t, &a);
	assert_true(tr_start(&t, AUTO, 300, &b));
	tr_set(&t, &c);
	assert_int_equal(t.stop_signals, 1);
	assert_int_equal(a.active_refs, 0);
	assert_int_equal(b.active_refs, 0);
	assert_int_equal(c.active_refs, 1);
	assert_int_equal(a.refs + b.refs, 0);
	assert_ptr_equal(t.src[0], &c);
	assert_ptr_equal(tr_get_active(&t), &c);

	/* idle: no stop signal */
	tr_set(&t, &a);
	assert_int_equal(t.stop_signals, 1);
	assert_int_equal(c.active_refs, 0);
	assert_int_equal(a.active_refs, 1);

	/* to none: slot A is still marked active */
	tr_set(&t, NULL);
	assert_true(t.slot_active[0]);
	assert_null(t.src[0]);
	assert_int_equal(a.active_refs, 0);

	/* characterized, not endorsed: a set whose add fails still marks slot
	 * A active, so the next set removes an active reference it never got */
	struct child e = {"E", 0, 0, true};
	tr_set(&t, &e);
	assert_true(t.slot_active[0]);
	assert_int_equal(e.active_refs, 0);
	tr_set(&t, NULL);
	assert_int_equal(e.active_refs, -1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(refused_and_walks),
		cmocka_unit_test(admit_every_combination),
		cmocka_unit_test(duration),
		cmocka_unit_test(set_source_rules),
		cmocka_unit_test(manual_reset),
		cmocka_unit_test(start_from_idle),
		cmocka_unit_test(start_to_source_a_refused),
		cmocka_unit_test(start_while_running),
		cmocka_unit_test(null_destination),
		cmocka_unit_test(manual_same_target),
		cmocka_unit_test(reentering_callback),
		cmocka_unit_test(failed_add_keeps_nested_clock),
		cmocka_unit_test(fixed_duration),
		cmocka_unit_test(manual_times),
		cmocka_unit_test(swap),
		cmocka_unit_test(set_active_and_idle),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
