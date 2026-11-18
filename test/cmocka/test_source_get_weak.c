/* Characterize obs_source_get_weak_source() before its later Rust swap (Refs #5129).
 * Run unchanged against hybrid libobs and libobs-c-reference. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

#if !__has_include("obs-source-get-weak.c.h")
#error "source weak-handle acquisition extraction seam is missing"
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

/* Calls obs_source_get_weak_source() with logging captured. */
static obs_weak_source_t *get_weak(obs_source_t *source)
{
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	obs_weak_source_t *weak = obs_source_get_weak_source(source);
	base_set_log_handler(old_handler, old_param);
	return weak;
}

/* A NULL source returns NULL and logs nothing. */
static void null_source(void **state)
{
	(void)state;
	log_count = 0;
	assert_null(get_weak(NULL));
	assert_int_equal(log_count, 0);
}

/* The result is the source's own context.control, with exactly one weak
 * reference added per call and the strong count untouched. Nothing in the
 * source itself changes. */
static void returns_control_with_one_weak_ref(void **state)
{
	(void)state;
	struct obs_weak_source control = {0};
	struct obs_source source = {0};
	unsigned char snapshot[sizeof(source)];
	control.ref.refs = 7;
	control.ref.weak_refs = 3;
	control.source = &source;
	source.context.control = (struct obs_weak_object *)&control;
	memcpy(snapshot, &source, sizeof(source));
	log_count = 0;

	for (long i = 1; i <= 3; i++) {
		assert_ptr_equal(get_weak(&source), &control);
		assert_int_equal(control.ref.weak_refs, 3 + i);
		assert_int_equal(control.ref.refs, 7);
		assert_ptr_equal(control.source, &source);
		assert_memory_equal(snapshot, &source, sizeof(source));
	}
	assert_int_equal(log_count, 0);
}

/* A source with no control returns NULL through the no-op addref. */
static void null_control(void **state)
{
	(void)state;
	struct obs_source source = {0};
	log_count = 0;
	assert_null(get_weak(&source));
	assert_null(source.context.control);
	assert_int_equal(log_count, 0);
}

/* On a live source the weak handle does not keep the source alive, and it
 * outlives the source's teardown until every acquisition is released. */
static void live_source(void **state)
{
	(void)state;
	obs_scene_t *scene = obs_scene_create_private("get weak owner");
	assert_non_null(scene);
	obs_source_t *source = obs_scene_get_source(scene);
	struct obs_weak_source *control = (struct obs_weak_source *)source->context.control;
	assert_non_null(control);
	long strong = control->ref.refs;
	long weak_refs = control->ref.weak_refs;

	obs_weak_source_t *first = obs_source_get_weak_source(source);
	obs_weak_source_t *second = obs_source_get_weak_source(source);
	assert_ptr_equal(first, control);
	assert_ptr_equal(second, control);
	assert_int_equal(control->ref.refs, strong);
	assert_int_equal(control->ref.weak_refs, weak_refs + 2);
	assert_false(obs_weak_source_expired(first));

	obs_scene_release(scene);
	while (obs_wait_for_destroy_queue())
		;
	/* The source is gone; both acquired handles stay usable. */
	assert_true(obs_weak_source_expired(first));
	assert_null(obs_weak_source_get_source(second));
	obs_weak_source_release(first);
	assert_true(obs_weak_source_expired(second));
	obs_weak_source_release(second);
}

int main(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return 1;
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_source),
		cmocka_unit_test(returns_control_with_one_weak_ref),
		cmocka_unit_test(null_control),
		cmocka_unit_test(live_source),
	};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return result;
}
