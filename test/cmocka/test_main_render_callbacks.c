/* Characterize obs_add_main_render_callback and obs_remove_main_render_callback
 * before the Rust swap (Refs #5172): the exact extracted bodies run against a
 * stack core with lock probes. No OBS startup, graphics context or render
 * thread is needed, so nothing walks the arrays and every stored callback must
 * stay uncalled. A NULL core is outside the C contract and is not exercised. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <errno.h>
#include <string.h>
#include <cmocka.h>
#include "libobs/build/libobs.pch.h"

#if !__has_include("libobs/obs-main-render-callbacks.c.h")
#error "main-render callback extraction seam is missing"
#endif

/* The bodies under test read this core, not the library's hidden global. */
struct obs_core *obs;

/* Probe copies of the extracted bodies: lock and unlock become observable. */
int probe_lock(pthread_mutex_t *mutex);
int probe_unlock(pthread_mutex_t *mutex);
#define obs_add_main_render_callback add_body
#define obs_remove_main_render_callback remove_body
#define pthread_mutex_lock probe_lock
#define pthread_mutex_unlock probe_unlock
#include "libobs/obs-main-render-callbacks.c.h"
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
#undef obs_remove_main_render_callback
#undef obs_add_main_render_callback

static struct obs_core core;
static bool seeded_tail;
static unsigned draw_calls;
static int token_a, token_b, token_tail;

static void draw_one(void *param, uint32_t cx, uint32_t cy)
{
	(void)param;
	(void)cx;
	(void)cy;
	draw_calls++;
}

/* A different body, so identical-code folding cannot merge the two. */
static void draw_two(void *param, uint32_t cx, uint32_t cy)
{
	(void)param;
	(void)cx;
	(void)cy;
	draw_calls += 2;
}

static void rendered_tail(void *param)
{
	(void)param;
	draw_calls += 4;
}

/* Lock probes. The mutex is recursive, so "held" is checked from another
 * thread. Every call must lock once, change the list only while locked, and
 * unlock once. */
static unsigned locks, unlocks;
static struct draw_callback snapshot[16];
static size_t snapshot_num;

static void take_snapshot(void)
{
	snapshot_num = core.data.draw_callbacks.num;
	assert_true(snapshot_num <= 16);
	if (snapshot_num)
		memcpy(snapshot, core.data.draw_callbacks.array, snapshot_num * sizeof(*snapshot));
}

static void *trylock_thread(void *arg)
{
	pthread_mutex_t *mutex = arg;
	int result = pthread_mutex_trylock(mutex);
	if (result == 0)
		pthread_mutex_unlock(mutex);
	return (void *)(intptr_t)result;
}

static int trylock_elsewhere(pthread_mutex_t *mutex)
{
	pthread_t thread;
	void *result = NULL;
	assert_int_equal(pthread_create(&thread, NULL, trylock_thread, mutex), 0);
	assert_int_equal(pthread_join(thread, &result), 0);
	return (int)(intptr_t)result;
}

int probe_lock(pthread_mutex_t *mutex)
{
	assert_int_equal(locks++, unlocks);
	assert_ptr_equal(mutex, &core.data.draw_callbacks_mutex);
	/* Nothing changed before the lock is taken. */
	assert_int_equal(core.data.draw_callbacks.num, snapshot_num);
	if (snapshot_num)
		assert_memory_equal(core.data.draw_callbacks.array, snapshot, snapshot_num * sizeof(*snapshot));
	int result = pthread_mutex_lock(mutex);
	assert_int_equal(trylock_elsewhere(mutex), EBUSY);
	return result;
}

int probe_unlock(pthread_mutex_t *mutex)
{
	assert_int_equal(++unlocks, locks);
	assert_ptr_equal(mutex, &core.data.draw_callbacks_mutex);
	/* The list change is complete while the mutex is still held. */
	assert_int_equal(trylock_elsewhere(mutex), EBUSY);
	take_snapshot();
	return pthread_mutex_unlock(mutex);
}

static void add(void (*draw)(void *, uint32_t, uint32_t), void *param)
{
	unsigned before = locks;
	add_body(draw, param);
	assert_int_equal(locks, before + 1);
	assert_int_equal(unlocks, locks);
}

static void remove_one(void (*draw)(void *, uint32_t, uint32_t), void *param)
{
	unsigned before = locks;
	remove_body(draw, param);
	assert_int_equal(locks, before + 1);
	assert_int_equal(unlocks, locks);
}

/* The other array sharing the mutex; no draw-callback call may touch it. */
static const struct rendered_callback rendered_seed = {rendered_tail, &token_tail};

static int setup_with(void **state, bool seeded)
{
	(void)state;
	memset(&core, 0, sizeof(core));
	if (pthread_mutex_init_recursive(&core.data.draw_callbacks_mutex) != 0)
		return -1;
	da_push_back(core.data.rendered_callbacks, &rendered_seed);
	if (seeded) {
		/* An entry registered earlier must stay at the tail. */
		struct draw_callback tail = {draw_one, &token_tail};
		da_push_back(core.data.draw_callbacks, &tail);
	}
	seeded_tail = seeded;
	obs = &core;
	locks = unlocks = 0;
	draw_calls = 0;
	take_snapshot();
	return 0;
}

static int setup_empty(void **state)
{
	return setup_with(state, false);
}

static int setup_seeded(void **state)
{
	return setup_with(state, true);
}

static int teardown(void **state)
{
	(void)state;
	assert_int_equal(draw_calls, 0);
	assert_int_equal(core.data.rendered_callbacks.num, 1);
	assert_memory_equal(core.data.rendered_callbacks.array, &rendered_seed, sizeof(rendered_seed));
	da_free(core.data.draw_callbacks);
	da_free(core.data.rendered_callbacks);
	pthread_mutex_destroy(&core.data.draw_callbacks_mutex);
	obs = NULL;
	return 0;
}

/* Exact list contents, then the seeded tail entry if any. */
static void expect_list(const struct draw_callback *expected, size_t count)
{
	size_t tail = seeded_tail ? 1 : 0;
	assert_int_equal(core.data.draw_callbacks.num, count + tail);
	const struct draw_callback *array = core.data.draw_callbacks.array;
	for (size_t i = 0; i < count; i++) {
		assert_ptr_equal(array[i].draw, expected[i].draw);
		assert_ptr_equal(array[i].param, expected[i].param);
	}
	if (tail) {
		assert_ptr_equal(array[count].draw, draw_one);
		assert_ptr_equal(array[count].param, &token_tail);
	}
	assert_int_equal(draw_calls, 0);
	assert_int_equal(core.data.rendered_callbacks.num, 1);
	assert_memory_equal(core.data.rendered_callbacks.array, &rendered_seed, sizeof(rendered_seed));
}

static void prepends_without_calling(void **state)
{
	(void)state;
	add(draw_one, &token_a);
	expect_list((struct draw_callback[]){{draw_one, &token_a}}, 1);
	add(draw_two, &token_a);
	add(draw_one, &token_b);
	expect_list((struct draw_callback[]){{draw_one, &token_b}, {draw_two, &token_a}, {draw_one, &token_a}}, 3);

	remove_one(draw_one, &token_a);
	remove_one(draw_two, &token_a);
	remove_one(draw_one, &token_b);
	expect_list(NULL, 0);
}

static void duplicates_and_first_match_removal(void **state)
{
	(void)state;
	/* Stored order: C, A, B, A. */
	add(draw_one, &token_a);
	add(draw_two, &token_a);
	add(draw_one, &token_a);
	add(draw_two, &token_b);
	expect_list((struct draw_callback[]){{draw_two, &token_b},
					     {draw_one, &token_a},
					     {draw_two, &token_a},
					     {draw_one, &token_a}},
		    4);

	/* Only the first byte-equal pair goes; survivors keep their order. */
	remove_one(draw_one, &token_a);
	expect_list((struct draw_callback[]){{draw_two, &token_b}, {draw_two, &token_a}, {draw_one, &token_a}}, 3);

	/* Same function with another param, or another function with the same
	 * param, does not match. */
	remove_one(draw_one, &token_b);
	remove_one(draw_two, NULL);
	expect_list((struct draw_callback[]){{draw_two, &token_b}, {draw_two, &token_a}, {draw_one, &token_a}}, 3);

	remove_one(draw_one, &token_a);
	remove_one(draw_one, &token_a);
	expect_list((struct draw_callback[]){{draw_two, &token_b}, {draw_two, &token_a}}, 2);
	remove_one(draw_two, &token_b);
	remove_one(draw_two, &token_a);
	expect_list(NULL, 0);
}

static void absent_removal_is_a_noop(void **state)
{
	(void)state;
	/* On an empty list, and on one holding only an unrelated entry. */
	for (int i = 0; i < 3; i++) {
		remove_one(draw_one, &token_a);
		remove_one(NULL, NULL);
		expect_list(NULL, 0);
	}
}

static void null_tokens_are_data(void **state)
{
	(void)state;
	add(NULL, NULL);
	add(draw_one, NULL);
	add(NULL, &token_a);
	expect_list((struct draw_callback[]){{NULL, &token_a}, {draw_one, NULL}, {NULL, NULL}}, 3);

	remove_one(NULL, NULL);
	expect_list((struct draw_callback[]){{NULL, &token_a}, {draw_one, NULL}}, 2);
	remove_one(NULL, &token_a);
	remove_one(draw_one, NULL);
	expect_list(NULL, 0);
}

static void removal_can_reach_the_tail(void **state)
{
	(void)state;
	add(draw_two, &token_a);
	/* The earlier registration is the first match for its own pair. */
	remove_one(draw_one, &token_tail);
	assert_int_equal(core.data.draw_callbacks.num, 1);
	assert_ptr_equal(core.data.draw_callbacks.array[0].draw, draw_two);
	assert_ptr_equal(core.data.draw_callbacks.array[0].param, &token_a);
	remove_one(draw_two, &token_a);
	assert_int_equal(core.data.draw_callbacks.num, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(prepends_without_calling, setup_empty, teardown),
		cmocka_unit_test_setup_teardown(prepends_without_calling, setup_seeded, teardown),
		cmocka_unit_test_setup_teardown(duplicates_and_first_match_removal, setup_empty, teardown),
		cmocka_unit_test_setup_teardown(duplicates_and_first_match_removal, setup_seeded, teardown),
		cmocka_unit_test_setup_teardown(absent_removal_is_a_noop, setup_empty, teardown),
		cmocka_unit_test_setup_teardown(absent_removal_is_a_noop, setup_seeded, teardown),
		cmocka_unit_test_setup_teardown(null_tokens_are_data, setup_empty, teardown),
		cmocka_unit_test_setup_teardown(null_tokens_are_data, setup_seeded, teardown),
		cmocka_unit_test_setup_teardown(removal_can_reach_the_tail, setup_seeded, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
