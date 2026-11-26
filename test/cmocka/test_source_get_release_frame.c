/* Characterize obs_source_get_frame() and obs_source_release_frame() (#5757)
 * on stack source fixtures: the NULL early returns (none touches the async
 * mutex), the take of cur_async_frame with one added reference, and the
 * release: a NULL source destroys the frame whatever its references, the
 * last reference destroys it without touching the cache, any other count
 * gives the frame back to the async cache (remove_async_frame()). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "util/threading.h"

static int setup(void **state)
{
	struct obs_source *source = bzalloc(sizeof(*source));
	/* As obs_source_init(). */
	assert_int_equal(pthread_mutex_init_recursive(&source->async_mutex), 0);
	*state = source;
	return 0;
}

static int teardown(void **state)
{
	struct obs_source *source = *state;
	da_free(source->async_cache);
	assert_int_equal(pthread_mutex_destroy(&source->async_mutex), 0);
	bfree(source);
	return 0;
}

/* Two allocations, as obs_source_frame_create(): the frame and its data. */
static struct obs_source_frame *new_frame(long refs, bool prev_frame)
{
	struct obs_source_frame *frame = bzalloc(sizeof(*frame));
	frame->data[0] = bmalloc(16);
	frame->refs = refs;
	frame->prev_frame = prev_frame;
	return frame;
}

static void free_frame(struct obs_source_frame *frame)
{
	bfree(frame->data[0]);
	bfree(frame);
}

static void cache_push(struct obs_source *source, struct obs_source_frame *frame, bool used, long unused_count)
{
	struct async_frame entry = {frame, unused_count, used};
	da_push_back(source->async_cache, &entry);
}

/* async_mutex is recursive, so probe it from another thread. */
struct probe {
	struct obs_source *source;
	int result;
};

static void *trylock_async(void *data)
{
	struct probe *p = data;
	p->result = pthread_mutex_trylock(&p->source->async_mutex);
	if (p->result == 0)
		pthread_mutex_unlock(&p->source->async_mutex);
	return NULL;
}

static void expect_unlocked(struct obs_source *source)
{
	struct probe p = {source, -1};
	pthread_t thread;
	assert_int_equal(pthread_create(&thread, NULL, trylock_async, &p), 0);
	pthread_join(thread, NULL);
	assert_int_equal(p.result, 0);
}

/* Holds async_mutex on another thread, so an early return that locked would
 * block instead of returning. */
struct holder {
	struct obs_source *source;
	os_event_t *held;
	os_event_t *release;
};

static void *hold_mutex(void *data)
{
	struct holder *h = data;
	pthread_mutex_lock(&h->source->async_mutex);
	os_event_signal(h->held);
	os_event_wait(h->release);
	pthread_mutex_unlock(&h->source->async_mutex);
	return NULL;
}

static void release_while_held(struct obs_source *held, obs_source_t *source, struct obs_source_frame *frame)
{
	struct holder h = {held, NULL, NULL};
	pthread_t thread;
	assert_int_equal(os_event_init(&h.held, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(os_event_init(&h.release, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(pthread_create(&thread, NULL, hold_mutex, &h), 0);
	os_event_wait(h.held);
	obs_source_release_frame(source, frame);
	os_event_signal(h.release);
	pthread_join(thread, NULL);
	os_event_destroy(h.held);
	os_event_destroy(h.release);
}

static void get_null_source(void **state)
{
	(void)state;
	assert_null(obs_source_get_frame(NULL));
}

static void get_without_frame(void **state)
{
	struct obs_source *source = *state;
	assert_null(obs_source_get_frame(source));
	assert_null(source->cur_async_frame);
	expect_unlocked(source);
}

static void get_takes_current_with_a_reference(void **state)
{
	struct obs_source *source = *state;
	struct obs_source_frame *frame = new_frame(1, true);
	struct obs_source_frame *prev = new_frame(1, false);
	source->cur_async_frame = frame;
	source->prev_async_frame = prev;
	cache_push(source, frame, true, 3);

	assert_ptr_equal(obs_source_get_frame(source), frame);
	assert_null(source->cur_async_frame);
	assert_int_equal(frame->refs, 2);
	/* Nothing else is touched. */
	assert_true(frame->prev_frame);
	assert_ptr_equal(source->prev_async_frame, prev);
	assert_true(source->async_cache.array[0].used);
	assert_int_equal(source->async_cache.array[0].unused_count, 3);
	expect_unlocked(source);

	/* Taken once: the slot is empty now. */
	assert_null(obs_source_get_frame(source));
	assert_int_equal(frame->refs, 2);

	free_frame(frame);
	free_frame(prev);
}

static void release_null_frame(void **state)
{
	struct obs_source *source = *state;
	struct obs_source_frame *frame = new_frame(2, true);
	cache_push(source, frame, true, 0);
	long allocs = bnum_allocs();

	release_while_held(source, source, NULL);
	release_while_held(source, NULL, NULL);
	assert_int_equal(bnum_allocs(), allocs);
	assert_int_equal(frame->refs, 2);
	assert_true(frame->prev_frame);
	assert_true(source->async_cache.array[0].used);
	expect_unlocked(source);

	free_frame(frame);
}

/* A NULL source destroys the frame, whatever its references. */
static void release_null_source_destroys(void **state)
{
	struct obs_source *source = *state;
	struct obs_source_frame *frame = new_frame(5, true);
	long allocs = bnum_allocs();

	release_while_held(source, NULL, frame);
	assert_int_equal(bnum_allocs(), allocs - 2);
}

/* The last reference destroys the frame and leaves the cache alone
 * (characterized, not endorsed: a cached frame keeps its slot). */
static void release_last_reference_destroys(void **state)
{
	struct obs_source *source = *state;
	struct obs_source_frame *frame = new_frame(1, true);
	cache_push(source, frame, true, 4);
	long allocs = bnum_allocs();

	obs_source_release_frame(source, frame);
	assert_int_equal(bnum_allocs(), allocs - 2);
	assert_true(source->async_cache.array[0].used);
	assert_int_equal(source->async_cache.array[0].unused_count, 4);
	expect_unlocked(source);

	source->async_cache.array[0].frame = NULL;
}

/* Any other count gives the frame back to the cache: prev_frame cleared,
 * the first slot that holds it marked unused, nothing else changed. */
static void release_returns_to_cache(void **state)
{
	struct obs_source *source = *state;
	struct obs_source_frame *other = new_frame(1, true);
	struct obs_source_frame *frame = new_frame(3, true);
	cache_push(source, other, true, 1);
	cache_push(source, frame, true, 2);
	cache_push(source, frame, true, 5);
	long allocs = bnum_allocs();

	obs_source_release_frame(source, frame);
	assert_int_equal(bnum_allocs(), allocs);
	assert_int_equal(frame->refs, 2);
	assert_false(frame->prev_frame);
	assert_true(other->prev_frame);
	assert_true(source->async_cache.array[0].used);
	assert_false(source->async_cache.array[1].used);
	assert_int_equal(source->async_cache.array[1].unused_count, 2);
	assert_true(source->async_cache.array[2].used);
	assert_int_equal(source->async_cache.array[2].unused_count, 5);
	expect_unlocked(source);

	/* Again: the first holder is already unused and stays so; the walk
	 * still stops there. */
	obs_source_release_frame(source, frame);
	assert_int_equal(frame->refs, 1);
	assert_false(source->async_cache.array[1].used);
	assert_true(source->async_cache.array[2].used);

	free_frame(other);
	free_frame(frame);
}

/* A frame the cache does not hold only loses its reference and
 * prev_frame. */
static void release_uncached(void **state)
{
	struct obs_source *source = *state;
	struct obs_source_frame *other = new_frame(1, true);
	struct obs_source_frame *frame = new_frame(2, true);
	cache_push(source, other, true, 0);

	obs_source_release_frame(source, frame);
	assert_int_equal(frame->refs, 1);
	assert_false(frame->prev_frame);
	assert_true(source->async_cache.array[0].used);
	assert_true(other->prev_frame);
	expect_unlocked(source);

	free_frame(other);
	free_frame(frame);
}

/* Zero references go below zero and are not destroyed (characterized, not
 * endorsed). */
static void release_unreferenced(void **state)
{
	struct obs_source *source = *state;
	struct obs_source_frame *frame = new_frame(0, true);
	cache_push(source, frame, true, 0);
	long allocs = bnum_allocs();

	obs_source_release_frame(source, frame);
	assert_int_equal(bnum_allocs(), allocs);
	assert_int_equal(frame->refs, -1);
	assert_false(frame->prev_frame);
	assert_false(source->async_cache.array[0].used);

	free_frame(frame);
}

/* The pair as obs_source_update_async_video() uses it. */
static void get_then_release(void **state)
{
	struct obs_source *source = *state;
	struct obs_source_frame *frame = new_frame(1, false);
	source->cur_async_frame = frame;
	cache_push(source, frame, true, 0);

	struct obs_source_frame *got = obs_source_get_frame(source);
	assert_ptr_equal(got, frame);
	obs_source_release_frame(source, got);
	assert_int_equal(frame->refs, 1);
	assert_false(source->async_cache.array[0].used);
	assert_null(source->cur_async_frame);
	expect_unlocked(source);

	free_frame(frame);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(get_null_source),
		cmocka_unit_test_setup_teardown(get_without_frame, setup, teardown),
		cmocka_unit_test_setup_teardown(get_takes_current_with_a_reference, setup, teardown),
		cmocka_unit_test_setup_teardown(release_null_frame, setup, teardown),
		cmocka_unit_test_setup_teardown(release_null_source_destroys, setup, teardown),
		cmocka_unit_test_setup_teardown(release_last_reference_destroys, setup, teardown),
		cmocka_unit_test_setup_teardown(release_returns_to_cache, setup, teardown),
		cmocka_unit_test_setup_teardown(release_uncached, setup, teardown),
		cmocka_unit_test_setup_teardown(release_unreferenced, setup, teardown),
		cmocka_unit_test_setup_teardown(get_then_release, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
