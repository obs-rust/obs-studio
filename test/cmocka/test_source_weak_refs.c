/* Characterize obs_weak_source_addref() and obs_weak_source_release() before
 * their Rust swap (Refs #5665). Run unchanged against hybrid libobs and
 * libobs-c-reference. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#if !__has_include("obs-source-weak-refs.c.h")
#error "source weak-reference extraction seam is missing"
#endif

static unsigned log_count;

#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)level;
	(void)format;
	(void)args;
	(void)param;
	log_count++;
}

/* NULL handles are no-ops and log nothing. */
static void null_handles(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	const long before = bnum_allocs();
	obs_weak_source_addref(NULL);
	obs_weak_source_release(NULL);
	base_set_log_handler(old_handler, old_param);
	assert_int_equal(log_count, 0);
	assert_int_equal(bnum_allocs(), before);
}

/* Each addref raises only the weak count by one, each release lowers it by
 * one; the strong count and the source pointer stay as they were. */
static void counts_only_the_weak_refs(void **state)
{
	(void)state;
	struct obs_source source = {0};
	struct obs_weak_source control;
	memset(&control, 0xa5, sizeof(control));
	control.ref.refs = 7;
	control.ref.weak_refs = 3;
	control.source = &source;

	for (long i = 1; i <= 3; i++) {
		obs_weak_source_addref(&control);
		assert_int_equal(control.ref.weak_refs, 3 + i);
		assert_int_equal(control.ref.refs, 7);
		assert_ptr_equal(control.source, &source);
	}
	for (long i = 1; i <= 6; i++) {
		obs_weak_source_release(&control);
		assert_int_equal(control.ref.weak_refs, 6 - i);
		assert_int_equal(control.ref.refs, 7);
		assert_ptr_equal(control.source, &source);
	}
	/* weak_refs 0 is the last weak owner; one more release would free it. */
	assert_int_equal(control.ref.weak_refs, 0);
}

/* The release that takes the weak count from 0 to -1 frees the control,
 * whatever the strong count is; no earlier release frees it. */
static void final_release_frees(void **state)
{
	(void)state;
	const long counts[] = {-2, -1, 0, 5};
	for (size_t i = 0; i < sizeof(counts) / sizeof(counts[0]); i++) {
		const long before = bnum_allocs();
		struct obs_weak_source *control = bzalloc(sizeof(*control));
		control->ref.refs = counts[i];
		control->ref.weak_refs = 1;
		assert_int_equal(bnum_allocs(), before + 1);
		obs_weak_source_release(control);
		assert_int_equal(control->ref.weak_refs, 0);
		assert_int_equal(control->ref.refs, counts[i]);
		assert_int_equal(bnum_allocs(), before + 1);
		obs_weak_source_addref(control);
		obs_weak_source_release(control);
		assert_int_equal(bnum_allocs(), before + 1);
		obs_weak_source_release(control);
		assert_int_equal(bnum_allocs(), before);
	}
}

/* Characterized, not endorsed: only the exact -1 frees. A count already below
 * zero is lowered further and the control stays allocated. */
static void below_zero_never_frees(void **state)
{
	(void)state;
	const long before = bnum_allocs();
	struct obs_weak_source *control = bzalloc(sizeof(*control));
	control->ref.weak_refs = -1;
	obs_weak_source_release(control);
	assert_int_equal(control->ref.weak_refs, -2);
	assert_int_equal(bnum_allocs(), before + 1);
	obs_weak_source_addref(control);
	obs_weak_source_addref(control);
	assert_int_equal(control->ref.weak_refs, 0);
	assert_int_equal(bnum_allocs(), before + 1);
	obs_weak_source_release(control);
	assert_int_equal(bnum_allocs(), before);
}

/* On a live source, weak references taken with addref keep the control (not
 * the source) alive after the source is destroyed; the last release frees it. */
static void live_source(void **state)
{
	(void)state;
	obs_scene_t *scene = obs_scene_create_private("weak refs owner");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	assert_non_null(weak);
	long strong = weak->ref.refs;
	long weak_refs = weak->ref.weak_refs;
	/* The source's own weak reference (0) plus this acquisition. */
	assert_int_equal(weak_refs, 1);

	obs_weak_source_addref(weak);
	assert_int_equal(weak->ref.weak_refs, weak_refs + 1);
	assert_int_equal(weak->ref.refs, strong);
	obs_weak_source_release(weak);
	assert_int_equal(weak->ref.weak_refs, weak_refs);
	assert_int_equal(weak->ref.refs, strong);
	assert_false(obs_weak_source_expired(weak));

	obs_weak_source_addref(weak);
	obs_scene_release(scene);
	while (obs_wait_for_destroy_queue())
		;
	assert_true(obs_weak_source_expired(weak));
	assert_null(obs_weak_source_get_source(weak));
	/* The source dropped its own; the acquisition and the addref remain. */
	assert_int_equal(weak->ref.weak_refs, 1);
	const long before = bnum_allocs();
	obs_weak_source_release(weak);
	assert_int_equal(weak->ref.weak_refs, 0);
	assert_int_equal(bnum_allocs(), before);
	obs_weak_source_release(weak);
	assert_int_equal(bnum_allocs(), before - 1);
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 1;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_handles),        cmocka_unit_test(counts_only_the_weak_refs),
		cmocka_unit_test(final_release_frees), cmocka_unit_test(below_zero_never_frees),
		cmocka_unit_test(live_source),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return result;
}
