#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs.h"
#include "obs-source-transition-load.h"
#include "obs-source-transition-start.h"

/*
 * Characterization of the load, scale type and swap checks of the
 * transitions in libobs/obs-source-transition.c, moved to
 * libobs/obs-source-transition-load.c before their Rust port:
 * - obs_source_transition_load_is_transition(): the type check of
 *   transition_valid();
 * - obs_source_transition_load_scale_type(): the scale type rule of
 *   obs_transition_set_scale_type() and obs_transition_load();
 * - obs_source_transition_load_looks_up() and _source(): the source steps
 *   of obs_transition_load();
 * - obs_source_transition_load_swaps(): the same-transition gate of
 *   obs_transition_swap_begin() and _end();
 * - obs_source_transition_load_copy_stores(): the recursion refusal of
 *   copy_source_state().
 * Each is pinned over every combination of its inputs, and the scale type
 * over the edges of int64_t. The last cases run copies of
 * obs_transition_load(), copy_source_state() with the swap, and the getters
 * on a stand-in transition that counts each child's references and active
 * references, so the composition is pinned too: the loaded fields (an
 * alignment, size and mode out of range are truncated or kept: characterized,
 * not endorsed), a missing name, an unknown name, a source that contains the
 * transition, a load onto a slot A that already holds a source, the swap
 * with itself, every slot state of the swap with the refusal, and the
 * getters' answers for a source that is not a transition.
 *
 * The functions are hidden in libobs, so this test compiles the C sources
 * into itself and calls only them (with
 * obs_source_transition_start_child_active(), which the load and the swap
 * also ask). test_transition_load_release, test_transition_scale_type,
 * test_transition_swap, test_transition_swap_active and
 * test_transition_swap_cycle cover the callers through the real libobs.
 */

#define KEEP OBS_SOURCE_TRANSITION_LOAD_KEEP
#define NOT_FOUND OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND
#define REFUSE OBS_SOURCE_TRANSITION_LOAD_REFUSE

#define MAX_ONLY OBS_SOURCE_TRANSITION_LOAD_SCALE_MAX_ONLY
#define ASPECT OBS_SOURCE_TRANSITION_LOAD_SCALE_ASPECT
#define STRETCH OBS_SOURCE_TRANSITION_LOAD_SCALE_STRETCH

static void constants(void **state)
{
	(void)state;

	/* the header's mirror of libobs */
	assert_int_equal(OBS_SOURCE_TRANSITION_LOAD_TYPE_TRANSITION, (int)OBS_SOURCE_TYPE_TRANSITION);
	assert_int_equal(MAX_ONLY, (int)OBS_TRANSITION_SCALE_MAX_ONLY);
	assert_int_equal(ASPECT, (int)OBS_TRANSITION_SCALE_ASPECT);
	assert_int_equal(STRETCH, (int)OBS_TRANSITION_SCALE_STRETCH);
	assert_int_equal(sizeof(enum obs_source_type), sizeof(int));
	assert_int_equal(sizeof(enum obs_transition_scale_type), sizeof(int));
	assert_int_equal((int)KEEP, 0);
	assert_int_equal((int)NOT_FOUND, 1);
	assert_int_equal((int)REFUSE, 2);
}

static void is_transition(void **state)
{
	(void)state;

	assert_true(obs_source_transition_load_is_transition((int)OBS_SOURCE_TYPE_TRANSITION));
	assert_false(obs_source_transition_load_is_transition((int)OBS_SOURCE_TYPE_INPUT));
	assert_false(obs_source_transition_load_is_transition((int)OBS_SOURCE_TYPE_FILTER));
	assert_false(obs_source_transition_load_is_transition((int)OBS_SOURCE_TYPE_SCENE));

	/* only 2, over a range of ints and the ends */
	for (int type = -1000; type <= 1000; type++) {
		if (obs_source_transition_load_is_transition(type) != (type == 2)) {
			fprintf(stderr, "is_transition(%d) = %d\n", type,
				obs_source_transition_load_is_transition(type));
			fail();
		}
	}
	assert_false(obs_source_transition_load_is_transition(INT32_MIN));
	assert_false(obs_source_transition_load_is_transition(INT32_MAX));
	assert_false(obs_source_transition_load_is_transition(2 + 256));
}

static void scale_type(void **state)
{
	(void)state;

	/* the three values as they are */
	assert_int_equal(obs_source_transition_load_scale_type(0), MAX_ONLY);
	assert_int_equal(obs_source_transition_load_scale_type(1), ASPECT);
	assert_int_equal(obs_source_transition_load_scale_type(2), STRETCH);

	/* anything else is Max Only */
	assert_int_equal(obs_source_transition_load_scale_type(3), MAX_ONLY);
	assert_int_equal(obs_source_transition_load_scale_type(-1), MAX_ONLY);
	assert_int_equal(obs_source_transition_load_scale_type(INT64_MAX), MAX_ONLY);
	assert_int_equal(obs_source_transition_load_scale_type(INT64_MIN), MAX_ONLY);
	assert_int_equal(obs_source_transition_load_scale_type(INT32_MAX), MAX_ONLY);
	assert_int_equal(obs_source_transition_load_scale_type(INT32_MIN), MAX_ONLY);

	/* compared in 64 bits: a value whose low 32 bits are 1 or 2 is not
	 * Aspect or Stretch */
	assert_int_equal(obs_source_transition_load_scale_type(((int64_t)1 << 32) + 1), MAX_ONLY);
	assert_int_equal(obs_source_transition_load_scale_type(((int64_t)1 << 32) + 2), MAX_ONLY);
	assert_int_equal(obs_source_transition_load_scale_type(-((int64_t)1 << 32) + 1), MAX_ONLY);
	assert_int_equal(obs_source_transition_load_scale_type((int64_t)UINT32_MAX + 3), MAX_ONLY);

	for (int64_t v = -1000; v <= 1000; v++) {
		int want = (v == 1 || v == 2) ? (int)v : MAX_ONLY;
		if (obs_source_transition_load_scale_type(v) != want) {
			fprintf(stderr, "scale_type(%lld) = %d\n", (long long)v,
				obs_source_transition_load_scale_type(v));
			fail();
		}
	}
}

static void load_source_steps(void **state)
{
	(void)state;

	/* looked up only with a name */
	assert_true(obs_source_transition_load_looks_up(true));
	assert_false(obs_source_transition_load_looks_up(false));

	/* not found wins over the add's result (no add is asked then) */
	assert_int_equal(obs_source_transition_load_source(false, false), NOT_FOUND);
	assert_int_equal(obs_source_transition_load_source(false, true), NOT_FOUND);
	assert_int_equal(obs_source_transition_load_source(true, false), REFUSE);
	assert_int_equal(obs_source_transition_load_source(true, true), KEEP);
}

static void swap_and_copy_steps(void **state)
{
	(void)state;

	/* a transition swapped with itself does nothing */
	assert_true(obs_source_transition_load_swaps(false));
	assert_false(obs_source_transition_load_swaps(true));

	/* the new child is stored unless it was to be added and the add
	 * failed */
	assert_true(obs_source_transition_load_copy_stores(false, false));
	assert_true(obs_source_transition_load_copy_stores(false, true));
	assert_false(obs_source_transition_load_copy_stores(true, false));
	assert_true(obs_source_transition_load_copy_stores(true, true));
}

/* A stand-in transition and its children: each child counts its references
 * and active references, and refuses to be added to a transition it contains
 * (as obs_source_add_active_child() does). */
struct child {
	const char *name;
	bool contains;
	int refs;
	int active_refs;
};

struct tr {
	int type;
	struct child *src[2];
	bool slot_active[2];
	uint32_t alignment;
	uint32_t cx;
	uint32_t cy;
	int mode;
	int scale;
	bool fixed;
	int warnings;
	int resizes;
};

static struct child *g_children[4];

static struct child *lookup(const char *name)
{
	for (size_t i = 0; i < 4; i++) {
		if (g_children[i] && strcmp(g_children[i]->name, name) == 0) {
			g_children[i]->refs++;
			return g_children[i];
		}
	}
	return NULL;
}

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

static bool add_active(struct child *c)
{
	if (c->contains)
		return false;
	c->active_refs++;
	return true;
}

static void remove_active(struct child *c)
{
	c->active_refs--;
}

/* transition_valid() past obs_ptr_valid() */
static bool tr_valid(const struct tr *t)
{
	return t && obs_source_transition_load_is_transition(t->type);
}

/* obs_transition_load(), with the obs_data values passed in */
static void tr_load(struct tr *t, const char *name, int64_t alignment, int64_t mode, int64_t scale, int64_t cx,
		    int64_t cy)
{
	struct child *source = NULL;
	struct child *old;
	bool old_active;

	if (obs_source_transition_load_looks_up(name != NULL)) {
		source = lookup(name);
		bool found = source != NULL;
		bool add_ok = found && add_active(source);

		switch (obs_source_transition_load_source(found, add_ok)) {
		case OBS_SOURCE_TRANSITION_LOAD_REFUSE:
			t->warnings++;
			release(source);
			source = NULL;
			break;
		case OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND:
			t->warnings++;
			break;
		case OBS_SOURCE_TRANSITION_LOAD_KEEP:
			break;
		}
	}

	old = t->src[0];
	old_active = t->slot_active[0];
	t->src[0] = source;
	t->slot_active[0] = true;
	t->alignment = (uint32_t)alignment;
	t->mode = (int)mode;
	t->scale = obs_source_transition_load_scale_type(scale);
	t->cx = (uint32_t)cx;
	t->cy = (uint32_t)cy;

	if (obs_source_transition_start_child_active(old != NULL, old_active))
		remove_active(old);
	release(old);
	t->resizes++;
}

/* copy_source_state() */
static struct child *copy_state(struct tr *dest, struct tr *source, size_t idx)
{
	struct child *old_child = dest->src[idx];
	struct child *new_child = get_ref(source->src[idx]);
	bool active = source->slot_active[idx];

	if (old_child && dest->slot_active[idx])
		remove_active(old_child);

	bool adds = obs_source_transition_start_child_active(new_child != NULL, active);

	if (!obs_source_transition_load_copy_stores(adds, adds && add_active(new_child))) {
		dest->warnings++;
		release(new_child);
		new_child = NULL;
		active = false;
	}

	dest->src[idx] = new_child;
	dest->slot_active[idx] = active;
	return old_child;
}

/* obs_transition_swap_begin() and _end(); _end() clears the source */
static void tr_swap(struct tr *dest, struct tr *source)
{
	struct child *old_children[2];

	if (!obs_source_transition_load_swaps(dest == source))
		return;
	for (size_t i = 0; i < 2; i++)
		old_children[i] = copy_state(dest, source, i);
	dest->mode = source->mode;
	for (size_t i = 0; i < 2; i++)
		release(old_children[i]);

	if (!obs_source_transition_load_swaps(dest == source))
		return;
	for (size_t i = 0; i < 2; i++) {
		struct child *s = source->src[i];
		bool active = source->slot_active[i];
		source->src[i] = NULL;
		source->slot_active[i] = false;
		if (s && active)
			remove_active(s);
		release(s);
	}
}

/* a transition as obs_transition_init() leaves it */
static void tr_init(struct tr *t)
{
	memset(t, 0, sizeof(*t));
	t->type = (int)OBS_SOURCE_TYPE_TRANSITION;
	t->slot_active[0] = true;
	t->alignment = OBS_ALIGN_LEFT | OBS_ALIGN_TOP;
}

static void load_fields(void **state)
{
	(void)state;
	struct tr t;

	tr_init(&t);
	tr_load(&t, NULL, OBS_ALIGN_CENTER, 1, 2, 1920, 1080);
	assert_null(t.src[0]);
	assert_true(t.slot_active[0]);
	assert_int_equal(t.alignment, OBS_ALIGN_CENTER);
	assert_int_equal(t.mode, 1);
	assert_int_equal(t.scale, STRETCH);
	assert_int_equal(t.cx, 1920);
	assert_int_equal(t.cy, 1080);
	assert_int_equal(t.warnings, 0);
	assert_int_equal(t.resizes, 1);

	/* characterized, not endorsed: the alignment and size are truncated
	 * to their low 32 bits, and a negative one wraps */
	tr_load(&t, NULL, ((int64_t)1 << 32) + 5, 0, 1, ((int64_t)1 << 32) + 640, -1);
	assert_int_equal(t.alignment, 5);
	assert_int_equal(t.cx, 640);
	assert_int_equal(t.cy, UINT32_MAX);
	assert_int_equal(t.scale, ASPECT);
	tr_load(&t, NULL, -2, 0, 0, INT64_MAX, INT64_MIN);
	assert_int_equal(t.alignment, UINT32_MAX - 1);
	assert_int_equal(t.cx, UINT32_MAX);
	assert_int_equal(t.cy, 0);
	assert_int_equal(t.scale, MAX_ONLY);

	/* characterized, not endorsed: a mode out of the enum is kept */
	tr_load(&t, NULL, 0, 7, 0, 0, 0);
	assert_int_equal(t.mode, 7);

	/* a scale type out of the enum is Max Only */
	tr_load(&t, NULL, 0, 0, 3, 0, 0);
	assert_int_equal(t.scale, MAX_ONLY);
	tr_load(&t, NULL, 0, 0, -1, 0, 0);
	assert_int_equal(t.scale, MAX_ONLY);
	tr_load(&t, NULL, 0, 0, INT64_MAX, 0, 0);
	assert_int_equal(t.scale, MAX_ONLY);
	tr_load(&t, NULL, 0, 0, ((int64_t)1 << 32) + 2, 0, 0);
	assert_int_equal(t.scale, MAX_ONLY);
	assert_int_equal(t.warnings, 0);
}

static void load_sources(void **state)
{
	(void)state;
	struct child a = {"A", false, 1, 0}, b = {"B", false, 1, 0}, self = {"self", true, 1, 0};
	struct tr t;

	g_children[0] = &a;
	g_children[1] = &b;
	g_children[2] = &self;
	g_children[3] = NULL;

	/* a missing name: none, no warning */
	tr_init(&t);
	tr_load(&t, NULL, 0, 0, 0, 0, 0);
	assert_null(t.src[0]);
	assert_int_equal(t.warnings, 0);

	/* an unknown name: none and a warning */
	tr_load(&t, "nope", 0, 0, 0, 0, 0);
	assert_null(t.src[0]);
	assert_true(t.slot_active[0]);
	assert_int_equal(t.warnings, 1);

	/* an empty name is still looked up */
	tr_load(&t, "", 0, 0, 0, 0, 0);
	assert_null(t.src[0]);
	assert_int_equal(t.warnings, 2);

	/* a known source: kept, with a reference and an active reference */
	tr_load(&t, "A", 0, 0, 0, 0, 0);
	assert_ptr_equal(t.src[0], &a);
	assert_int_equal(a.refs, 2);
	assert_int_equal(a.active_refs, 1);
	assert_int_equal(t.warnings, 2);

	/* a source that contains the transition: released, none and a
	 * warning; the old source A still goes out */
	tr_load(&t, "self", 0, 0, 0, 0, 0);
	assert_null(t.src[0]);
	assert_int_equal(self.refs, 1);
	assert_int_equal(self.active_refs, 0);
	assert_int_equal(a.refs, 1);
	assert_int_equal(a.active_refs, 0);
	assert_int_equal(t.warnings, 3);

	/* a load onto a slot A that holds a source takes it back */
	tr_load(&t, "A", 0, 0, 0, 0, 0);
	tr_load(&t, "B", 0, 0, 0, 0, 0);
	assert_ptr_equal(t.src[0], &b);
	assert_int_equal(a.refs, 1);
	assert_int_equal(a.active_refs, 0);
	assert_int_equal(b.refs, 2);
	assert_int_equal(b.active_refs, 1);

	/* the same source again: one reference and one active reference */
	tr_load(&t, "B", 0, 0, 0, 0, 0);
	assert_int_equal(b.refs, 2);
	assert_int_equal(b.active_refs, 1);

	/* an inactive slot A: its source is released without a removal */
	t.slot_active[0] = false;
	b.active_refs--;
	tr_load(&t, NULL, 0, 0, 0, 0, 0);
	assert_null(t.src[0]);
	assert_true(t.slot_active[0]);
	assert_int_equal(b.refs, 1);
	assert_int_equal(b.active_refs, 0);

	memset(g_children, 0, sizeof(g_children));
}

static void swap_with_itself(void **state)
{
	(void)state;
	struct child a = {"A", false, 1, 0};
	struct tr t;

	tr_init(&t);
	t.src[0] = get_ref(&a);
	add_active(&a);
	t.mode = 1;
	tr_swap(&t, &t);
	assert_ptr_equal(t.src[0], &a);
	assert_true(t.slot_active[0]);
	assert_int_equal(a.refs, 2);
	assert_int_equal(a.active_refs, 1);
	assert_int_equal(t.mode, 1);
}

static void swap_slot_states(void **state)
{
	(void)state;

	/* every slot state: the old child present or not and active or not,
	 * the new child present or not, active or not and contained or not */
	for (int bits = 0; bits < 32; bits++) {
		bool has_old = bits & 1;
		bool old_active = bits & 2;
		bool has_new = bits & 4;
		bool new_active = bits & 8;
		bool contains = bits & 16;
		struct child o = {"O", false, 1, 0}, n = {"N", contains, 1, 0};
		struct tr dest, source;

		tr_init(&dest);
		tr_init(&source);
		if (has_old) {
			dest.src[1] = get_ref(&o);
			if (old_active)
				add_active(&o);
		}
		dest.slot_active[1] = old_active;
		if (has_new) {
			source.src[1] = get_ref(&n);
			/* the source's active slot holds an active reference */
			if (new_active)
				n.active_refs++;
		}
		source.slot_active[1] = new_active;

		tr_swap(&dest, &source);

		bool refused = has_new && new_active && contains;
		/* the old child is gone: no references left */
		assert_int_equal(o.refs, 1);
		assert_int_equal(o.active_refs, 0);
		/* the source transition is cleared */
		assert_null(source.src[1]);
		assert_false(source.slot_active[1]);
		if (refused) {
			assert_null(dest.src[1]);
			assert_false(dest.slot_active[1]);
			assert_int_equal(dest.warnings, 1);
			assert_int_equal(n.refs, 1);
			assert_int_equal(n.active_refs, 0);
		} else {
			assert_ptr_equal(dest.src[1], has_new ? &n : NULL);
			assert_int_equal(dest.slot_active[1], new_active);
			assert_int_equal(dest.warnings, 0);
			assert_int_equal(n.refs, has_new ? 2 : 1);
			assert_int_equal(n.active_refs, has_new && new_active ? 1 : 0);
		}
	}
}

/* obs_transition_get_scale_type(), _get_alignment(), _get_size() and
 * _fixed(): a source that is not a transition (or none) answers the
 * defaults */
static int tr_get_scale_type(const struct tr *t)
{
	return tr_valid(t) ? t->scale : (int)OBS_TRANSITION_SCALE_MAX_ONLY;
}

static uint32_t tr_get_alignment(const struct tr *t)
{
	return tr_valid(t) ? t->alignment : 0;
}

static void tr_get_size(const struct tr *t, uint32_t *cx, uint32_t *cy)
{
	if (!tr_valid(t)) {
		*cx = 0;
		*cy = 0;
		return;
	}
	*cx = t->cx;
	*cy = t->cy;
}

static bool tr_fixed(const struct tr *t)
{
	return tr_valid(t) ? t->fixed : false;
}

static void tr_set_scale_type(struct tr *t, int type)
{
	if (!tr_valid(t))
		return;
	t->scale = obs_source_transition_load_scale_type((int64_t)type);
}

static void getters(void **state)
{
	(void)state;
	static const int types[] = {OBS_SOURCE_TYPE_INPUT,
				    OBS_SOURCE_TYPE_FILTER,
				    OBS_SOURCE_TYPE_TRANSITION,
				    OBS_SOURCE_TYPE_SCENE,
				    -1,
				    99};
	uint32_t cx = 1, cy = 1;

	for (size_t i = 0; i < sizeof(types) / sizeof(types[0]); i++) {
		bool valid = types[i] == (int)OBS_SOURCE_TYPE_TRANSITION;
		struct tr t;

		tr_init(&t);
		t.type = types[i];
		t.scale = STRETCH;
		t.alignment = OBS_ALIGN_RIGHT;
		t.cx = 640;
		t.cy = 480;
		t.fixed = true;

		assert_int_equal(tr_get_scale_type(&t), valid ? STRETCH : MAX_ONLY);
		assert_int_equal(tr_get_alignment(&t), valid ? OBS_ALIGN_RIGHT : 0);
		tr_get_size(&t, &cx, &cy);
		assert_int_equal(cx, valid ? 640 : 0);
		assert_int_equal(cy, valid ? 480 : 0);
		assert_int_equal(tr_fixed(&t), valid);

		/* the setter writes only a transition, and only a valid value */
		tr_set_scale_type(&t, ASPECT);
		assert_int_equal(t.scale, valid ? ASPECT : STRETCH);
		tr_set_scale_type(&t, 3);
		assert_int_equal(t.scale, valid ? MAX_ONLY : STRETCH);
		tr_set_scale_type(&t, -1);
		assert_int_equal(t.scale, valid ? MAX_ONLY : STRETCH);
	}

	/* no source at all */
	assert_int_equal(tr_get_scale_type(NULL), MAX_ONLY);
	assert_int_equal(tr_get_alignment(NULL), 0);
	cx = cy = 1;
	tr_get_size(NULL, &cx, &cy);
	assert_int_equal(cx, 0);
	assert_int_equal(cy, 0);
	assert_false(tr_fixed(NULL));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),           cmocka_unit_test(is_transition),
		cmocka_unit_test(scale_type),          cmocka_unit_test(load_source_steps),
		cmocka_unit_test(swap_and_copy_steps), cmocka_unit_test(load_fields),
		cmocka_unit_test(load_sources),        cmocka_unit_test(swap_with_itself),
		cmocka_unit_test(swap_slot_states),    cmocka_unit_test(getters),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
