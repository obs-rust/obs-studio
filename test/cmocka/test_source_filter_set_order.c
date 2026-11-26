/* Characterization of obs_source_filter_set_order() (#5775), the filter
 * move behind the filters dialog's up and down buttons and obs-websocket's
 * SetSourceFilterIndex helper.
 *
 * What it pins, on test-owned struct obs_source values (no core needed):
 *
 *   - NULL: a NULL source logs "obs_source_filter_set_order: Null 'source'
 *     parameter", a NULL filter "... Null 'filter' parameter", once each at
 *     LOG_DEBUG; the source is checked first. Nothing of the source is read
 *     (its list is NULL here), written or signalled.
 *   - A filter that is not in the source's list: nothing is moved,
 *     retargeted or signalled, whatever the movement, and filter_mutex is
 *     free afterwards.
 *   - OBS_ORDER_MOVE_UP and _DOWN move the filter to the position of the
 *     nearest filter above or below it of the same async kind (OBS_SOURCE_ASYNC
 *     set or not), skipping the others; _TOP and _BOTTOM move it to the end
 *     or the start of the list, whatever the kinds. With no such filter, or
 *     already at that end, nothing is moved, retargeted or signalled.
 *   - After a move every filter's filter_target is rewritten: the next
 *     filter in the list, the last one the source.
 *   - Only after a move, "reorder_filters" is signalled once on the source's
 *     own signal handler (no global signal), after filter_mutex is released,
 *     with the source as calldata "source".
 *   - Nothing else of the source or of any filter is written, and a non-NULL
 *     call logs nothing.
 *
 * A movement outside enum obs_order_movement moves no filter. The C
 * reference also retargets every filter and signals "reorder_filters" for
 * it, which the port fixes (#2981); only what both share is asserted for
 * those movements: the list is unchanged, and either nothing is retargeted
 * or signalled, or every filter is retargeted and one signal sent. */
#include <limits.h>
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

/* A parent, its filters and one filter that is not in its list, every byte
 * noise-filled except the fields the call may read. The mutex is a plain
 * one, so a trylock from the signal handler tells whether it is held. */
struct fixture {
	struct obs_source parent;
	struct obs_source filters[FILTERS];
	struct obs_source outsider;
	obs_source_t *list[FILTERS];
	size_t count;
	struct obs_source parent_before;
	struct obs_source filters_before[FILTERS];
	struct obs_source outsider_before;

	/* What the "reorder_filters" handler saw. */
	size_t signals;
	void *signal_source;
	bool signal_unlocked;
	obs_source_t *signal_list[FILTERS];
};

static void reorder_filters(void *param, calldata_t *data)
{
	struct fixture *f = param;
	f->signals++;
	f->signal_source = calldata_ptr(data, "source");
	int locked = pthread_mutex_trylock(&f->parent.filter_mutex);
	if (locked == 0)
		pthread_mutex_unlock(&f->parent.filter_mutex);
	f->signal_unlocked = locked == 0;
	memcpy(f->signal_list, f->list, sizeof(f->list));
}

/* async is a bit mask: filter i is async when bit i is set. */
static void fixture_init(struct fixture *f, unsigned char fill, size_t count, unsigned async)
{
	memset(f, fill, sizeof(*f));
	assert_int_equal(pthread_mutex_init(&f->parent.filter_mutex, NULL), 0);
	f->parent.context.signals = signal_handler_create();
	assert_non_null(f->parent.context.signals);
	assert_true(signal_handler_add(f->parent.context.signals, "void reorder_filters(ptr source)"));
	signal_handler_connect(f->parent.context.signals, "reorder_filters", reorder_filters, f);
	for (size_t i = 0; i < FILTERS; i++) {
		uint32_t flags = OBS_SOURCE_VIDEO | ((uint32_t)fill << 8);
		if (async & (1u << i))
			flags |= OBS_SOURCE_ASYNC;
		f->filters[i].info.output_flags = flags;
		f->list[i] = &f->filters[i];
	}
	f->outsider.info.output_flags = OBS_SOURCE_VIDEO;
	f->count = count;
	f->parent.filters.array = f->list;
	f->parent.filters.num = count;
	f->parent.filters.capacity = FILTERS;
	f->signals = 0;
	f->signal_source = NULL;
	f->signal_unlocked = false;
	memcpy(&f->parent_before, &f->parent, sizeof(f->parent));
	memcpy(f->filters_before, f->filters, sizeof(f->filters));
	memcpy(&f->outsider_before, &f->outsider, sizeof(f->outsider));
}

static void fixture_free(struct fixture *f)
{
	signal_handler_destroy(f->parent.context.signals);
	assert_int_equal(pthread_mutex_destroy(&f->parent.filter_mutex), 0);
}

static void assert_bytes_equal_except(const void *before, const void *after, size_t size, size_t at, size_t len)
{
	const unsigned char *b = before;
	const unsigned char *a = after;
	assert_memory_equal(b, a, at);
	assert_memory_equal(b + at + len, a + at + len, size - at - len);
}

/* The parent's bytes outside its mutex are as they were, and so is every
 * filter's, except filter_target when retargeted is true. */
static void assert_unwritten(struct fixture *f, bool retargeted)
{
	assert_bytes_equal_except(&f->parent_before, &f->parent, sizeof(f->parent),
				  offsetof(struct obs_source, filter_mutex), sizeof(f->parent.filter_mutex));
	for (size_t i = 0; i < FILTERS; i++) {
		if (retargeted && i < f->count)
			assert_bytes_equal_except(&f->filters_before[i], &f->filters[i], sizeof(f->filters[i]),
						  offsetof(struct obs_source, filter_target),
						  sizeof(f->filters[i].filter_target));
		else
			assert_memory_equal(&f->filters_before[i], &f->filters[i], sizeof(f->filters[i]));
	}
	assert_memory_equal(&f->outsider_before, &f->outsider, sizeof(f->outsider));
}

static void assert_unlocked(struct fixture *f)
{
	assert_int_equal(pthread_mutex_trylock(&f->parent.filter_mutex), 0);
	pthread_mutex_unlock(&f->parent.filter_mutex);
}

/* Every listed filter targets the next one, the last one the parent. */
static void assert_retargeted(struct fixture *f)
{
	for (size_t i = 0; i < f->count; i++) {
		obs_source_t *expected = i + 1 == f->count ? &f->parent : f->list[i + 1];
		assert_ptr_equal(f->list[i]->filter_target, expected);
	}
}

/* The list holds filters[order[i]] at position i. */
static void assert_order(struct fixture *f, const size_t *order)
{
	for (size_t i = 0; i < f->count; i++)
		assert_ptr_equal(f->list[i], &f->filters[order[i]]);
	for (size_t i = f->count; i < FILTERS; i++)
		assert_ptr_equal(f->list[i], &f->filters[i]);
}

static const int out_of_enum[] = {-1, 4, 5, 1000, INT_MIN, INT_MAX};

static void null_arguments_log_once_and_do_nothing(void **state)
{
	(void)state;
	static struct fixture f;
	fixture_init(&f, 0x5a, 3, 0);
	/* Reading the list would crash. */
	f.parent.filters.array = NULL;
	memcpy(&f.parent_before, &f.parent, sizeof(f.parent));

	capture_start();
	obs_source_filter_set_order(NULL, &f.filters[0], OBS_ORDER_MOVE_UP);
	capture_stop();
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_filter_set_order: Null 'source' parameter");

	/* The source is checked first. */
	capture_start();
	obs_source_filter_set_order(NULL, NULL, OBS_ORDER_MOVE_TOP);
	capture_stop();
	assert_int_equal(log_count, 1);
	assert_string_equal(log_message, "obs_source_filter_set_order: Null 'source' parameter");

	for (int movement = OBS_ORDER_MOVE_UP; movement <= OBS_ORDER_MOVE_BOTTOM; movement++) {
		capture_start();
		obs_source_filter_set_order(&f.parent, NULL, (enum obs_order_movement)movement);
		capture_stop();
		assert_int_equal(log_count, 1);
		assert_int_equal(log_level, LOG_DEBUG);
		assert_string_equal(log_message, "obs_source_filter_set_order: Null 'filter' parameter");
	}

	assert_int_equal(f.signals, 0);
	assert_unwritten(&f, false);
	assert_unlocked(&f);
	f.parent.filters.array = f.list;
	fixture_free(&f);
}

static void unlisted_filter_moves_nothing(void **state)
{
	(void)state;
	static const unsigned char fills[] = {0x00, 0xff, 0x5a};
	static struct fixture f;
	const size_t identity[FILTERS] = {0, 1, 2, 3, 4, 5};

	capture_start();
	for (size_t fill = 0; fill < sizeof(fills); fill++) {
		for (size_t count = 0; count <= FILTERS; count++) {
			for (int movement = OBS_ORDER_MOVE_UP; movement <= OBS_ORDER_MOVE_BOTTOM; movement++) {
				fixture_init(&f, fills[fill], count, 0x15);
				obs_source_filter_set_order(&f.parent, &f.outsider, (enum obs_order_movement)movement);
				assert_int_equal(f.signals, 0);
				assert_order(&f, identity);
				assert_unwritten(&f, false);
				assert_unlocked(&f);
				fixture_free(&f);
			}
			for (size_t i = 0; i < sizeof(out_of_enum) / sizeof(out_of_enum[0]); i++) {
				fixture_init(&f, fills[fill], count, 0x15);
				obs_source_filter_set_order(&f.parent, &f.outsider,
							    (enum obs_order_movement)out_of_enum[i]);
				assert_int_equal(f.signals, 0);
				assert_order(&f, identity);
				assert_unwritten(&f, false);
				assert_unlocked(&f);
				fixture_free(&f);
			}
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

/* One move on a fresh fixture: the list ends as order (NULL: unchanged, no
 * signal). */
static void check_move(unsigned char fill, size_t count, unsigned async, size_t idx, int movement, const size_t *order)
{
	static struct fixture f;
	const size_t identity[FILTERS] = {0, 1, 2, 3, 4, 5};

	fixture_init(&f, fill, count, async);
	capture_start();
	obs_source_filter_set_order(&f.parent, &f.filters[idx], (enum obs_order_movement)movement);
	capture_stop();
	assert_int_equal(log_count, 0);
	if (order) {
		assert_order(&f, order);
		assert_retargeted(&f);
		assert_unwritten(&f, true);
		assert_int_equal(f.signals, 1);
		assert_ptr_equal(f.signal_source, &f.parent);
		assert_true(f.signal_unlocked);
		/* The list was already moved when the signal ran. */
		assert_memory_equal(f.signal_list, f.list, sizeof(f.list));
	} else {
		assert_order(&f, identity);
		assert_unwritten(&f, false);
		assert_int_equal(f.signals, 0);
	}
	assert_unlocked(&f);
	fixture_free(&f);
}

#define A(i) (1u << (i))

static void moves_walk_to_the_same_kind_and_the_ends(void **state)
{
	(void)state;
	/* Up: to the next filter, whatever is past it. */
	check_move(0x5a, 4, 0, 1, OBS_ORDER_MOVE_UP, (const size_t[]){0, 2, 1, 3});
	/* Up skips filters of the other kind: 1 (sync) passes async 2 and 3. */
	check_move(0x5a, 5, A(2) | A(3), 1, OBS_ORDER_MOVE_UP, (const size_t[]){0, 2, 3, 4, 1});
	/* An async filter skips the sync ones. */
	check_move(0x00, 5, A(0) | A(3), 0, OBS_ORDER_MOVE_UP, (const size_t[]){1, 2, 3, 0, 4});
	/* No filter of the same kind above: nothing. */
	check_move(0xff, 4, A(1), 1, OBS_ORDER_MOVE_UP, NULL);
	check_move(0xff, 4, 0, 3, OBS_ORDER_MOVE_UP, NULL);

	/* Down mirrors up. */
	check_move(0x5a, 4, 0, 2, OBS_ORDER_MOVE_DOWN, (const size_t[]){0, 2, 1, 3});
	check_move(0x5a, 5, A(1) | A(2), 3, OBS_ORDER_MOVE_DOWN, (const size_t[]){3, 0, 1, 2, 4});
	check_move(0x00, 5, A(4) | A(1), 4, OBS_ORDER_MOVE_DOWN, (const size_t[]){0, 4, 1, 2, 3});
	check_move(0xff, 4, A(2), 2, OBS_ORDER_MOVE_DOWN, NULL);
	check_move(0xff, 4, 0, 0, OBS_ORDER_MOVE_DOWN, NULL);

	/* Top and bottom go to the ends, whatever the kinds. */
	check_move(0x5a, 5, A(1), 1, OBS_ORDER_MOVE_TOP, (const size_t[]){0, 2, 3, 4, 1});
	check_move(0x5a, 5, A(1), 3, OBS_ORDER_MOVE_BOTTOM, (const size_t[]){3, 0, 1, 2, 4});
	check_move(0x00, 3, 0, 2, OBS_ORDER_MOVE_TOP, NULL);
	check_move(0x00, 3, 0, 0, OBS_ORDER_MOVE_BOTTOM, NULL);

	/* A single filter goes nowhere. */
	for (int movement = OBS_ORDER_MOVE_UP; movement <= OBS_ORDER_MOVE_BOTTOM; movement++)
		check_move(0x5a, 1, 0, 0, movement, NULL);
}

/* Where the filter at idx goes, or false when it stays (the model of the
 * movement rules above). */
static bool model_move(size_t count, unsigned async, size_t idx, int movement, size_t *order)
{
	size_t to = idx;
	bool kind = (async & A(idx)) != 0;

	for (size_t i = 0; i < count; i++)
		order[i] = i;
	if (movement == OBS_ORDER_MOVE_UP) {
		size_t j = idx + 1;
		while (j < count && ((async & A(j)) != 0) != kind)
			j++;
		if (j == count)
			return false;
		to = j;
	} else if (movement == OBS_ORDER_MOVE_DOWN) {
		size_t j = idx;
		while (j > 0 && ((async & A(j - 1)) != 0) != kind)
			j--;
		if (j == 0)
			return false;
		to = j - 1;
	} else if (movement == OBS_ORDER_MOVE_TOP) {
		if (idx == count - 1)
			return false;
		to = count - 1;
	} else {
		if (idx == 0)
			return false;
		to = 0;
	}

	if (to > idx) {
		for (size_t i = idx; i < to; i++)
			order[i] = i + 1;
	} else {
		for (size_t i = idx; i > to; i--)
			order[i] = i - 1;
	}
	order[to] = idx;
	return true;
}

static void every_chain_moves_as_the_rules_say(void **state)
{
	(void)state;
	for (size_t count = 1; count <= FILTERS; count++) {
		for (unsigned async = 0; async < (1u << count); async++) {
			for (size_t idx = 0; idx < count; idx++) {
				for (int movement = OBS_ORDER_MOVE_UP; movement <= OBS_ORDER_MOVE_BOTTOM; movement++) {
					size_t order[FILTERS];
					bool moves = model_move(count, async, idx, movement, order);
					check_move((unsigned char)(async * 37u), count, async, idx, movement,
						   moves ? order : NULL);
				}
			}
		}
	}
}

static void out_of_enum_movement_moves_no_filter(void **state)
{
	(void)state;
	static struct fixture f;
	const size_t identity[FILTERS] = {0, 1, 2, 3, 4, 5};

	capture_start();
	for (size_t count = 1; count <= FILTERS; count++) {
		for (size_t idx = 0; idx < count; idx++) {
			for (size_t i = 0; i < sizeof(out_of_enum) / sizeof(out_of_enum[0]); i++) {
				fixture_init(&f, 0x5a, count, A(1) | A(4));
				obs_source_filter_set_order(&f.parent, &f.filters[idx],
							    (enum obs_order_movement)out_of_enum[i]);
				assert_order(&f, identity);
				/* Characterized as what the C reference and the port
				 * share (#2981). */
				assert_true(f.signals <= 1);
				if (f.signals == 1) {
					assert_retargeted(&f);
					assert_unwritten(&f, true);
					assert_ptr_equal(f.signal_source, &f.parent);
					assert_true(f.signal_unlocked);
				} else {
					assert_unwritten(&f, false);
				}
				assert_unlocked(&f);
				fixture_free(&f);
			}
		}
	}
	capture_stop();
	assert_int_equal(log_count, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_arguments_log_once_and_do_nothing),
		cmocka_unit_test(unlisted_filter_moves_nothing),
		cmocka_unit_test(moves_walk_to_the_same_kind_and_the_ends),
		cmocka_unit_test(every_chain_moves_as_the_rules_say),
		cmocka_unit_test(out_of_enum_movement_moves_no_filter),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
