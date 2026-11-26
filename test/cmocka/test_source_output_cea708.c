/* Characterize obs_source_output_cea708() (#5750) on stack source fixtures:
 * the destroying and NULL-captions early returns (neither touches the list
 * mutex), newest-first dispatch under the mutex with duplicates, and how a
 * callback that grows the list during dispatch is seen. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"
#include "util/threading.h"

#define MAX_CALLS 16

struct call {
	int id;
	void *param;
	obs_source_t *source;
	const struct obs_source_cea_708 *captions;
	int trylock;
};

static struct call calls[MAX_CALLS];
static size_t call_count;

static void record(int id, void *param, obs_source_t *source, const struct obs_source_cea_708 *captions)
{
	assert_true(call_count < MAX_CALLS);
	struct call *c = &calls[call_count++];
	c->id = id;
	c->param = param;
	c->source = source;
	c->captions = captions;
	/* EBUSY while the dispatch holds the list mutex. */
	c->trylock = pthread_mutex_trylock(&source->caption_cb_mutex);
	if (c->trylock == 0)
		pthread_mutex_unlock(&source->caption_cb_mutex);
}

static void caption_a(void *param, obs_source_t *source, const struct obs_source_cea_708 *captions)
{
	record(1, param, source, captions);
}

static void caption_b(void *param, obs_source_t *source, const struct obs_source_cea_708 *captions)
{
	record(2, param, source, captions);
}

/* Appends caption_b to the list it is being called from. */
static void caption_grow(void *param, obs_source_t *source, const struct obs_source_cea_708 *captions)
{
	record(3, param, source, captions);
	struct caption_cb_info info = {caption_b, param};
	for (int i = 0; i < 64; i++)
		da_push_back(source->caption_cb_list, &info);
}

static void caption_fail(void *param, obs_source_t *source, const struct obs_source_cea_708 *captions)
{
	(void)param;
	(void)source;
	(void)captions;
	fail();
}

static int setup(void **state)
{
	struct obs_source *source = bzalloc(sizeof(*source));
	assert_int_equal(pthread_mutex_init(&source->caption_cb_mutex, NULL), 0);
	memset(calls, 0, sizeof(calls));
	call_count = 0;
	*state = source;
	return 0;
}

static int teardown(void **state)
{
	struct obs_source *source = *state;
	da_free(source->caption_cb_list);
	assert_int_equal(pthread_mutex_destroy(&source->caption_cb_mutex), 0);
	bfree(source);
	return 0;
}

static void add(struct obs_source *source, obs_source_caption_t callback, void *param)
{
	struct caption_cb_info info = {callback, param};
	da_push_back(source->caption_cb_list, &info);
}

static void expect_call(size_t index, int id, void *param, struct obs_source *source,
			const struct obs_source_cea_708 *captions)
{
	assert_int_equal(calls[index].id, id);
	assert_ptr_equal(calls[index].param, param);
	assert_ptr_equal(calls[index].source, source);
	assert_ptr_equal(calls[index].captions, captions);
	assert_int_not_equal(calls[index].trylock, 0);
}

static void expect_unlocked(struct obs_source *source)
{
	assert_int_equal(pthread_mutex_trylock(&source->caption_cb_mutex), 0);
	pthread_mutex_unlock(&source->caption_cb_mutex);
}

/* Holds the list mutex on another thread, so an early return that locked
 * would block instead of returning. */
struct holder {
	struct obs_source *source;
	os_event_t *held;
	os_event_t *release;
};

static void *hold_mutex(void *data)
{
	struct holder *h = data;
	pthread_mutex_lock(&h->source->caption_cb_mutex);
	os_event_signal(h->held);
	os_event_wait(h->release);
	pthread_mutex_unlock(&h->source->caption_cb_mutex);
	return NULL;
}

static void call_while_held(struct obs_source *source, const struct obs_source_cea_708 *captions)
{
	struct holder h = {source, NULL, NULL};
	pthread_t thread;
	assert_int_equal(os_event_init(&h.held, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(os_event_init(&h.release, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(pthread_create(&thread, NULL, hold_mutex, &h), 0);
	os_event_wait(h.held);
	obs_source_output_cea708(source, captions);
	os_event_signal(h.release);
	pthread_join(thread, NULL);
	os_event_destroy(h.held);
	os_event_destroy(h.release);
}

static void destroying_returns_first(void **state)
{
	struct obs_source *source = *state;
	const uint8_t data[3] = {1, 2, 3};
	const struct obs_source_cea_708 captions = {data, 1, 42};
	add(source, caption_fail, NULL);
	os_atomic_set_long(&source->destroying, 1);
	call_while_held(source, &captions);
	assert_int_equal(call_count, 0);
	expect_unlocked(source);
}

static void null_captions_returns_unlocked(void **state)
{
	struct obs_source *source = *state;
	add(source, caption_fail, NULL);
	call_while_held(source, NULL);
	assert_int_equal(call_count, 0);
	expect_unlocked(source);
}

static void empty_list(void **state)
{
	struct obs_source *source = *state;
	const struct obs_source_cea_708 captions = {NULL, 0, 0};
	obs_source_output_cea708(source, &captions);
	assert_int_equal(call_count, 0);
	expect_unlocked(source);
}

static void newest_first_under_mutex(void **state)
{
	struct obs_source *source = *state;
	int first = 1, second = 2;
	const uint8_t data[6] = {0xfc, 0x94, 0x2c, 0xfc, 0x80, 0x80};
	const struct obs_source_cea_708 captions = {data, 2, 1000};
	add(source, caption_a, &first);
	add(source, caption_b, &first);
	add(source, caption_a, &second);
	add(source, caption_a, &first);
	add(source, caption_b, NULL);
	obs_source_output_cea708(source, &captions);
	assert_int_equal(call_count, 5);
	expect_call(0, 2, NULL, source, &captions);
	expect_call(1, 1, &first, source, &captions);
	expect_call(2, 1, &second, source, &captions);
	expect_call(3, 2, &first, source, &captions);
	expect_call(4, 1, &first, source, &captions);
	expect_unlocked(source);
	/* The list is left as it was. */
	assert_int_equal(source->caption_cb_list.num, 5);

	call_count = 0;
	obs_source_output_cea708(source, &captions);
	assert_int_equal(call_count, 5);
	expect_unlocked(source);
}

/* The count is read once and each entry is read from the current array:
 * entries appended during the dispatch (and the reallocation they cause)
 * do not change which callbacks run. Characterized, not endorsed. */
static void growth_during_dispatch(void **state)
{
	struct obs_source *source = *state;
	int first = 1, second = 2, third = 3;
	const struct obs_source_cea_708 captions = {NULL, 0, 7};
	add(source, caption_a, &first);
	add(source, caption_a, &second);
	add(source, caption_grow, &third);
	obs_source_output_cea708(source, &captions);
	assert_int_equal(call_count, 3);
	expect_call(0, 3, &third, source, &captions);
	expect_call(1, 1, &second, source, &captions);
	expect_call(2, 1, &first, source, &captions);
	assert_int_equal(source->caption_cb_list.num, 3 + 64);
	expect_unlocked(source);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(destroying_returns_first, setup, teardown),
		cmocka_unit_test_setup_teardown(null_captions_returns_unlocked, setup, teardown),
		cmocka_unit_test_setup_teardown(empty_list, setup, teardown),
		cmocka_unit_test_setup_teardown(newest_first_under_mutex, setup, teardown),
		cmocka_unit_test_setup_teardown(growth_during_dispatch, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
