/* Characterization of obs_source_enum_filters() (#5605), the filter-list
 * walk the filters dialog, the scene collection log, the missing-files list,
 * the scripting wrappers and filter duplication use.
 *
 * What it pins, on test-owned struct obs_source values (no core needed):
 *
 *   - NULL: the call returns and logs one LOG_DEBUG line. A NULL source logs
 *     "obs_source_enum_filters: Null 'source' parameter"; a NULL callback
 *     logs "obs_source_enum_filters: Null 'callback_obj.obj' parameter" (the
 *     validity macro stringizes the union member it was handed, upstream's
 *     quirk, kept). The source is checked first.
 *   - It calls back once per filter, last to first, with
 *     (source, filter, param), filter_mutex held across each call. The count
 *     is read once; each entry is read again at its index from the current
 *     array, so a callback that swaps the array or shrinks the count changes
 *     what is visited next, not how many calls are made.
 *   - It takes no reference, writes neither the source nor a filter, and a
 *     non-NULL call logs nothing. The mutex is released afterwards. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned log_count;
static int log_level;
static char log_message[128];
#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static log_handler_t saved_handler;
static void *saved_param;

static void capture_start(void)
{
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	log_count = 0;
	log_level = -1;
	log_message[0] = '\0';
}

static void capture_stop(void)
{
	base_set_log_handler(saved_handler, saved_param);
}

#define FILTERS 6

/* A parent and its filters, every byte noise-filled except the fields the
 * call may read. The mutex is a plain one, so a trylock from inside the
 * callback tells whether the call holds it. */
struct fixture {
	struct obs_source parent;
	struct obs_source filters[FILTERS];
	struct obs_weak_source weak[FILTERS];
	obs_source_t *list[FILTERS];
	obs_source_t *other[FILTERS];
	struct obs_source parent_before;
	struct obs_source filters_before[FILTERS];
};

static void fixture_init(struct fixture *f, unsigned char fill, size_t count)
{
	memset(f, fill, sizeof(*f));
	assert_int_equal(pthread_mutex_init(&f->parent.filter_mutex, NULL), 0);
	for (size_t i = 0; i < FILTERS; i++) {
		f->filters[i].context.control = (struct obs_weak_object *)&f->weak[i];
		f->weak[i].ref.refs = 0;
		f->weak[i].ref.weak_refs = 0;
		f->weak[i].source = &f->filters[i];
		f->list[i] = &f->filters[i];
		f->other[i] = &f->filters[FILTERS - 1 - i];
	}
	f->parent.filters.array = f->list;
	f->parent.filters.num = count;
	f->parent.filters.capacity = FILTERS;
	memcpy(&f->parent_before, &f->parent, sizeof(f->parent));
	memcpy(f->filters_before, f->filters, sizeof(f->filters));
}

/* The parent's bytes outside its mutex, and every filter's bytes, are as
 * they were. */
static void assert_unwritten(struct fixture *f)
{
	const size_t mutex_at = offsetof(struct obs_source, filter_mutex);
	const size_t mutex_end = mutex_at + sizeof(f->parent.filter_mutex);
	assert_memory_equal(&f->parent_before, &f->parent, mutex_at);
	assert_memory_equal((const unsigned char *)&f->parent_before + mutex_end,
			    (const unsigned char *)&f->parent + mutex_end, sizeof(f->parent) - mutex_end);
	assert_memory_equal(f->filters_before, f->filters, sizeof(f->filters));
}

static void assert_unlocked(struct fixture *f)
{
	assert_int_equal(pthread_mutex_trylock(&f->parent.filter_mutex), 0);
	pthread_mutex_unlock(&f->parent.filter_mutex);
}

static void fixture_free(struct fixture *f)
{
	assert_int_equal(pthread_mutex_destroy(&f->parent.filter_mutex), 0);
}

static void never_called(obs_source_t *parent, obs_source_t *child, void *param)
{
	(void)parent;
	(void)child;
	(void)param;
	fail();
}

static void null_arguments_log_once_and_return(void **state)
{
	(void)state;
	static struct fixture f;
	fixture_init(&f, 0x5a, 3);
	int param = 0;

	capture_start();
	obs_source_enum_filters(NULL, never_called, &param);
	capture_stop();
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_enum_filters: Null 'source' parameter");

	/* The source is checked first. */
	capture_start();
	obs_source_enum_filters(NULL, NULL, &param);
	capture_stop();
	assert_int_equal(log_count, 1);
	assert_string_equal(log_message, "obs_source_enum_filters: Null 'source' parameter");

	capture_start();
	obs_source_enum_filters(&f.parent, NULL, &param);
	capture_stop();
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	/* obs_ptr_valid() stringizes the union member it was handed. */
	assert_string_equal(log_message, "obs_source_enum_filters: Null 'callback_obj.obj' parameter");

	for (size_t i = 0; i < FILTERS; i++)
		assert_int_equal(f.weak[i].ref.refs, 0);
	assert_unwritten(&f);
	assert_unlocked(&f);
	fixture_free(&f);
}

struct visit {
	struct fixture *f;
	size_t calls;
	obs_source_t *parents[2 * FILTERS];
	obs_source_t *children[2 * FILTERS];
	void *params[2 * FILTERS];
	bool locked[2 * FILTERS];
	/* What the callback does on its first call. */
	int mode;
};

enum { MODE_NONE, MODE_SWAP_ARRAY, MODE_SHRINK, MODE_REPLACE_NEXT };

static void record(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct visit *v = param;
	assert_true(v->calls < 2 * FILTERS);
	v->parents[v->calls] = parent;
	v->children[v->calls] = child;
	v->params[v->calls] = param;
	int locked = pthread_mutex_trylock(&v->f->parent.filter_mutex);
	if (locked == 0)
		pthread_mutex_unlock(&v->f->parent.filter_mutex);
	v->locked[v->calls] = locked != 0;
	if (v->calls == 0) {
		struct obs_source *p = &v->f->parent;
		switch (v->mode) {
		case MODE_SWAP_ARRAY:
			p->filters.array = v->f->other;
			break;
		case MODE_SHRINK:
			p->filters.num = 1;
			break;
		case MODE_REPLACE_NEXT:
			/* The next entry is read only when it is visited. */
			p->filters.array[p->filters.num - 2] = &v->f->filters[FILTERS - 1];
			break;
		}
	}
	v->calls++;
}

static void enum_filters_walks_last_to_first_under_the_lock(void **state)
{
	(void)state;
	static const unsigned char fills[] = {0x00, 0xff, 0x5a};
	static struct fixture f;

	capture_start();
	for (size_t fill = 0; fill < sizeof(fills); fill++) {
		for (size_t count = 0; count <= FILTERS; count++) {
			fixture_init(&f, fills[fill], count);
			struct visit v = {.f = &f};
			obs_source_enum_filters(&f.parent, record, &v);
			assert_int_equal(v.calls, count);
			for (size_t i = 0; i < count; i++) {
				assert_ptr_equal(v.parents[i], &f.parent);
				assert_ptr_equal(v.children[i], &f.filters[count - 1 - i]);
				assert_ptr_equal(v.params[i], &v);
				assert_true(v.locked[i]);
			}
			for (size_t i = 0; i < FILTERS; i++)
				assert_int_equal(f.weak[i].ref.refs, 0);
			assert_unwritten(&f);
			assert_unlocked(&f);
			fixture_free(&f);
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

static void enum_filters_reads_the_count_once_and_each_entry_when_visited(void **state)
{
	(void)state;
	static struct fixture f;

	/* The callback swaps the array: the remaining calls read the new one at
	 * the same indexes. */
	fixture_init(&f, 0x5a, 4);
	struct visit v = {.f = &f, .mode = MODE_SWAP_ARRAY};
	obs_source_enum_filters(&f.parent, record, &v);
	assert_int_equal(v.calls, 4);
	assert_ptr_equal(v.children[0], &f.filters[3]);
	assert_ptr_equal(v.children[1], f.other[2]);
	assert_ptr_equal(v.children[2], f.other[1]);
	assert_ptr_equal(v.children[3], f.other[0]);
	assert_unlocked(&f);
	fixture_free(&f);

	/* The callback shrinks the count: every call still happens. */
	fixture_init(&f, 0x00, 4);
	v = (struct visit){.f = &f, .mode = MODE_SHRINK};
	obs_source_enum_filters(&f.parent, record, &v);
	assert_int_equal(v.calls, 4);
	for (size_t i = 0; i < 4; i++)
		assert_ptr_equal(v.children[i], &f.filters[3 - i]);
	assert_unlocked(&f);
	fixture_free(&f);

	/* The callback replaces the next entry: the replacement is visited. */
	fixture_init(&f, 0xff, 3);
	v = (struct visit){.f = &f, .mode = MODE_REPLACE_NEXT};
	obs_source_enum_filters(&f.parent, record, &v);
	assert_int_equal(v.calls, 3);
	assert_ptr_equal(v.children[0], &f.filters[2]);
	assert_ptr_equal(v.children[1], &f.filters[FILTERS - 1]);
	assert_ptr_equal(v.children[2], &f.filters[0]);
	assert_unlocked(&f);
	fixture_free(&f);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_arguments_log_once_and_return),
		cmocka_unit_test(enum_filters_walks_last_to_first_under_the_lock),
		cmocka_unit_test(enum_filters_reads_the_count_once_and_each_entry_when_visited),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
