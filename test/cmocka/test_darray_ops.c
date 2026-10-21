#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

/*
 * Characterization tests for the util/darray.h operations the output
 * interleaver uses (#1594), before the Rust equivalent in obs-util gains
 * them: da_insert, da_erase_range, da_pop_front and da_push_back_new, plus
 * da_erase and da_free, which it uses on the same arrays. test_darray covers
 * only da_push_back_array. These pin the current C behavior: where items
 * move, the capacity after each step (the darray_ensure_capacity growth
 * rule: the first allocation takes the new size, then the capacity doubles,
 * or becomes the new size when that is larger), that removal never shrinks
 * or frees the buffer, and the bmalloc accounting. Cases marked
 * "characterized, not endorsed" are odd but real.
 *
 * Only calls the header allows are made: every index and range satisfies
 * the asserts in darray.h, so the results do not depend on NDEBUG.
 *
 * The functions are static inline, so this tests the C header only in both
 * libobs builds; the Rust equivalent is checked against oracle wrappers of
 * these functions (Tier 3).
 */

#include <util/bmem.h>
#include <util/darray.h>

/* Shaped like the interleaver's struct encoder_packet_time: more than one
 * word, so the moves are checked on element size, not on bytes. */
struct record {
	int64_t pts;
	uint32_t track;
	uint32_t tag;
};

static void assert_ints(const int *array, size_t num, const int *expected, size_t expected_num)
{
	assert_int_equal(num, expected_num);
	for (size_t i = 0; i < num; i++)
		assert_int_equal(array[i], expected[i]);
}

#define ASSERT_INTS(v, ...)                                                                   \
	do {                                                                                  \
		const int expected_[] = {__VA_ARGS__};                                        \
		assert_ints((v).array, (v).num, expected_, sizeof(expected_) / sizeof(int)); \
	} while (false)

/* Appends first, first + 1, ... to an int darray with darray_push_back. */
static void fill_ints(struct darray *da, int first, int count)
{
	for (int i = 0; i < count; i++) {
		int value = first + i;
		darray_push_back(sizeof(int), da, &value);
	}
}

static void test_layout(void **state)
{
	UNUSED_PARAMETER(state);

	struct align_probe {
		char c;
		struct darray da;
	};
	DARRAY(struct record) v;

	assert_int_equal(sizeof(void *), sizeof(size_t));
	assert_int_equal(sizeof(struct darray), 3 * sizeof(size_t));
	assert_int_equal(offsetof(struct align_probe, da), sizeof(size_t));
	assert_int_equal(offsetof(struct darray, array), 0);
	assert_int_equal(offsetof(struct darray, num), sizeof(size_t));
	assert_int_equal(offsetof(struct darray, capacity), 2 * sizeof(size_t));

	/* DARRAY(type) is a union: the typed fields alias struct darray. */
	assert_int_equal(sizeof(v), sizeof(struct darray));
	assert_ptr_equal((void *)&v.array, (void *)&v.da.array);
	assert_ptr_equal((void *)&v.num, (void *)&v.da.num);
	assert_ptr_equal((void *)&v.capacity, (void *)&v.da.capacity);
}

static void test_insert_into_empty(void **state)
{
	UNUSED_PARAMETER(state);

	DARRAY(int) v;
	int value = 7;

	/* resort_interleaved_packets zeroes the array with memset, not da_init. */
	memset(&v, 0, sizeof(v));

	/* idx == num takes the push_back path; the first allocation is 1. */
	da_insert(v, 0, &value);
	assert_non_null(v.array);
	assert_int_equal(v.capacity, 1);
	ASSERT_INTS(v, 7);

	da_free(v);
}

static void test_insert_front_middle_end(void **state)
{
	UNUSED_PARAMETER(state);

	DARRAY(int) v;
	da_init(v);
	da_reserve(v, 8);
	fill_ints(&v.da, 1, 4);
	ASSERT_INTS(v, 1, 2, 3, 4);

	int a = 10, b = 20, c = 30;
	da_insert(v, 0, &a);
	ASSERT_INTS(v, 10, 1, 2, 3, 4);
	da_insert(v, 3, &b);
	ASSERT_INTS(v, 10, 1, 2, 20, 3, 4);
	da_insert(v, v.num, &c);
	ASSERT_INTS(v, 10, 1, 2, 20, 3, 4, 30);
	da_insert(v, v.num - 1, &a);
	ASSERT_INTS(v, 10, 1, 2, 20, 3, 4, 10, 30);

	/* No growth while the reserved capacity holds. */
	assert_int_equal(v.capacity, 8);

	da_free(v);
}

static void test_insert_growth(void **state)
{
	UNUSED_PARAMETER(state);

	DARRAY(int) v;
	int value = 0;
	da_init(v);

	/* Inserting at the front each time: capacity 1, 2, 4, 4, 8. */
	const size_t caps[] = {1, 2, 4, 4, 8, 8, 8, 8, 16};
	for (size_t i = 0; i < sizeof(caps) / sizeof(caps[0]); i++) {
		value = (int)i;
		da_insert(v, 0, &value);
		assert_int_equal(v.num, i + 1);
		assert_int_equal(v.capacity, caps[i]);
	}
	ASSERT_INTS(v, 8, 7, 6, 5, 4, 3, 2, 1, 0);

	/* A middle insert into a full array grows it and keeps every item. */
	da_free(v);
	fill_ints(&v.da, 1, 4);
	assert_int_equal(v.capacity, 4);
	value = 99;
	da_insert(v, 2, &value);
	assert_int_equal(v.capacity, 8);
	ASSERT_INTS(v, 1, 2, 99, 3, 4);

	/* After a reserve that is not a power of two, growth doubles it. */
	da_free(v);
	da_reserve(v, 3);
	fill_ints(&v.da, 1, 3);
	da_insert(v, 1, &value);
	assert_int_equal(v.capacity, 6);
	ASSERT_INTS(v, 1, 99, 2, 3);

	da_free(v);
}

static void test_insert_records(void **state)
{
	UNUSED_PARAMETER(state);

	DARRAY(struct record) v;
	da_init(v);

	/* Sorted insertion by pts, as insert_interleaved_packet does. */
	const int64_t order[] = {50, 10, 40, 20, 30, 60, 0};
	for (size_t i = 0; i < sizeof(order) / sizeof(order[0]); i++) {
		struct record r = {order[i], (uint32_t)i, 0xA5A5A5A5u};
		size_t idx = 0;
		while (idx < v.num && v.array[idx].pts <= r.pts)
			idx++;
		da_insert(v, idx, &r);
	}

	assert_int_equal(v.num, 7);
	assert_int_equal(v.capacity, 8);
	for (size_t i = 0; i < v.num; i++) {
		assert_int_equal(v.array[i].pts, (int64_t)i * 10);
		assert_int_equal(v.array[i].tag, 0xA5A5A5A5u);
	}
	assert_int_equal(v.array[0].track, 6);
	assert_int_equal(v.array[5].track, 0);
	assert_int_equal(v.array[6].track, 5);

	da_free(v);
}

static void test_erase(void **state)
{
	UNUSED_PARAMETER(state);

	DARRAY(int) v;
	da_init(v);
	fill_ints(&v.da, 1, 5);
	void *buffer = v.array;

	da_erase(v, 2);
	ASSERT_INTS(v, 1, 2, 4, 5);
	da_erase(v, 0);
	ASSERT_INTS(v, 2, 4, 5);
	da_erase(v, v.num - 1);
	ASSERT_INTS(v, 2, 4);
	da_erase(v, 1);
	da_erase(v, 0);
	assert_int_equal(v.num, 0);

	/* Erasing never shrinks or frees the buffer. */
	assert_ptr_equal(v.array, buffer);
	assert_int_equal(v.capacity, 8);

	da_free(v);
}

static void test_erase_range(void **state)
{
	UNUSED_PARAMETER(state);

	DARRAY(int) v;
	da_init(v);
	fill_ints(&v.da, 1, 8);
	void *buffer = v.array;

	/* From the front, as discard_to_idx does. */
	da_erase_range(v, 0, 3);
	ASSERT_INTS(v, 4, 5, 6, 7, 8);

	/* From the middle. */
	da_erase_range(v, 1, 3);
	ASSERT_INTS(v, 4, 7, 8);

	/* A range of one is darray_erase. */
	da_erase_range(v, 1, 2);
	ASSERT_INTS(v, 4, 8);

	/* At the end: nothing moves. */
	fill_ints(&v.da, 20, 3);
	ASSERT_INTS(v, 4, 8, 20, 21, 22);
	da_erase_range(v, 3, 5);
	ASSERT_INTS(v, 4, 8, 20);

	/* The whole array: only num changes. */
	da_erase_range(v, 0, v.num);
	assert_int_equal(v.num, 0);
	assert_ptr_equal(v.array, buffer);
	assert_int_equal(v.capacity, 8);

	/* A whole array of one is darray_erase too. */
	fill_ints(&v.da, 5, 1);
	da_erase_range(v, 0, 1);
	assert_int_equal(v.num, 0);

	da_free(v);
}

static void test_erase_range_records(void **state)
{
	UNUSED_PARAMETER(state);

	DARRAY(struct record) v;
	da_init(v);
	for (uint32_t i = 0; i < 6; i++) {
		struct record r = {(int64_t)i * 100, i, ~i};
		da_push_back(v, &r);
	}

	da_erase_range(v, 0, 4);
	assert_int_equal(v.num, 2);
	assert_int_equal(v.array[0].pts, 400);
	assert_int_equal(v.array[0].track, 4);
	assert_int_equal(v.array[0].tag, ~4u);
	assert_int_equal(v.array[1].pts, 500);
	assert_int_equal(v.array[1].track, 5);
	assert_int_equal(v.array[1].tag, ~5u);

	da_free(v);
}

static void test_removal_leaves_tail_bytes(void **state)
{
	UNUSED_PARAMETER(state);

	/* Characterized, not endorsed: removal moves the tail down and lowers
	 * num, but the slots past the new num keep their old values. Nothing
	 * may read them; this pins that removal does not clear them. */
	DARRAY(int) v;
	da_init(v);
	fill_ints(&v.da, 1, 5);

	da_erase(v, 1);
	ASSERT_INTS(v, 1, 3, 4, 5);
	assert_int_equal(v.array[4], 5);

	da_erase_range(v, 0, 2);
	ASSERT_INTS(v, 4, 5);
	assert_int_equal(v.array[2], 4);
	assert_int_equal(v.array[3], 5);

	da_pop_front(v);
	ASSERT_INTS(v, 5);
	assert_int_equal(v.array[1], 5);

	/* Removing everything moves nothing. */
	da_erase_range(v, 0, 1);
	assert_int_equal(v.num, 0);
	assert_int_equal(v.array[0], 5);

	da_free(v);
}

static void test_pop_front(void **state)
{
	UNUSED_PARAMETER(state);

	DARRAY(struct record) v;
	da_init(v);
	for (uint32_t i = 0; i < 3; i++) {
		struct record r = {(int64_t)i, i, i + 1};
		da_push_back(v, &r);
	}
	void *buffer = v.array;

	da_pop_front(v);
	assert_int_equal(v.num, 2);
	assert_int_equal(v.array[0].pts, 1);
	assert_int_equal(v.array[1].pts, 2);
	assert_int_equal(v.array[1].tag, 3);

	da_pop_front(v);
	da_pop_front(v);
	assert_int_equal(v.num, 0);
	assert_ptr_equal(v.array, buffer);
	assert_int_equal(v.capacity, 4);

	da_free(v);
}

static void test_push_back_new(void **state)
{
	UNUSED_PARAMETER(state);

	DARRAY(struct record) v;
	da_init(v);

	/* Each call returns the new last item, zeroed, after any growth. */
	const size_t caps[] = {1, 2, 4, 4, 8};
	for (size_t i = 0; i < sizeof(caps) / sizeof(caps[0]); i++) {
		struct record *r = da_push_back_new(v);
		assert_int_equal(v.num, i + 1);
		assert_int_equal(v.capacity, caps[i]);
		assert_ptr_equal(r, &v.array[i]);
		assert_int_equal(r->pts, 0);
		assert_int_equal(r->track, 0);
		assert_int_equal(r->tag, 0);
		r->pts = (int64_t)i;
		r->tag = 0xFFFFFFFFu;
	}
	for (size_t i = 0; i < v.num; i++)
		assert_int_equal(v.array[i].pts, (int64_t)i);

	/* A reused slot is zeroed too, not left with its old bytes. */
	da_pop_back(v);
	struct record *r = da_push_back_new(v);
	assert_ptr_equal(r, &v.array[4]);
	assert_int_equal(r->pts, 0);
	assert_int_equal(r->tag, 0);

	/* With GCC and Clang, da_push_back_new is a macro of its own; the
	 * darray_push_back_new function behind it elsewhere does the same. */
	r->tag = 0xFFFFFFFFu;
	da_pop_back(v);
	r = darray_push_back_new(sizeof(struct record), &v.da);
	assert_ptr_equal(r, &v.array[4]);
	assert_int_equal(v.num, 5);
	assert_int_equal(r->tag, 0);
	r = darray_push_back_new(sizeof(struct record), &v.da);
	assert_ptr_equal(r, &v.array[5]);
	assert_int_equal(v.num, 6);
	assert_int_equal(v.capacity, 8);
	assert_int_equal(r->pts, 0);

	da_free(v);
}

static void test_alloc_accounting(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	DARRAY(struct record) v;
	struct record r = {1, 2, 3};
	da_init(v);

	/* The first insert allocates; growth replaces the block, one at a time. */
	da_insert(v, 0, &r);
	assert_int_equal(bnum_allocs(), allocs + 1);
	for (int i = 0; i < 20; i++)
		da_insert(v, v.num / 2, &r);
	assert_int_equal(v.capacity, 32);
	assert_int_equal(bnum_allocs(), allocs + 1);
	for (int i = 0; i < 20; i++)
		da_push_back_new(v);
	assert_int_equal(v.capacity, 64);
	assert_int_equal(bnum_allocs(), allocs + 1);

	/* Removal never allocates or frees. */
	da_erase(v, 3);
	da_erase_range(v, 0, 10);
	da_pop_front(v);
	da_erase_range(v, 0, v.num);
	assert_non_null(v.array);
	assert_int_equal(bnum_allocs(), allocs + 1);

	da_free(v);
	assert_null(v.array);
	assert_int_equal(v.num, 0);
	assert_int_equal(v.capacity, 0);
	assert_int_equal(bnum_allocs(), allocs);

	/* Freeing an array that never allocated is fine. */
	da_free(v);
	assert_int_equal(bnum_allocs(), allocs);
}

static void test_resort_pattern(void **state)
{
	UNUSED_PARAMETER(state);

	/* resort_interleaved_packets: take the darray out of its owner, zero
	 * the owner, insert every old item back in sorted position, then free
	 * the old buffer. */
	long allocs = bnum_allocs();
	DARRAY(struct record) owned, old;
	da_init(owned);
	const int64_t pts[] = {30, 10, 50, 20, 40};
	for (size_t i = 0; i < sizeof(pts) / sizeof(pts[0]); i++) {
		struct record r = {pts[i], (uint32_t)i, 0};
		da_push_back(owned, &r);
	}

	old.da = owned.da;
	memset(&owned, 0, sizeof(owned));
	for (size_t i = 0; i < old.num; i++) {
		size_t idx = 0;
		while (idx < owned.num && owned.array[idx].pts < old.array[i].pts)
			idx++;
		da_insert(owned, idx, &old.array[i]);
	}
	assert_int_equal(bnum_allocs(), allocs + 2);
	da_free(old);
	assert_int_equal(bnum_allocs(), allocs + 1);

	assert_int_equal(owned.num, 5);
	assert_int_equal(owned.capacity, 8);
	for (size_t i = 0; i < owned.num; i++)
		assert_int_equal(owned.array[i].pts, (int64_t)(i + 1) * 10);

	da_free(owned);
	assert_int_equal(bnum_allocs(), allocs);
}

static void test_packet_time_pattern(void **state)
{
	UNUSED_PARAMETER(state);

	/* The encoder_packet_times arrays: append zeroed records with
	 * da_push_back_new, drop the oldest with da_pop_front, and erase a
	 * matching one from the middle with da_erase. */
	DARRAY(struct record) v;
	da_init(v);

	for (int round = 0; round < 40; round++) {
		struct record *r = da_push_back_new(v);
		r->pts = round;
		r->track = (uint32_t)(round % 3);

		if (round % 4 == 3)
			da_pop_front(v);
		if (round % 5 == 4) {
			for (size_t i = 0; i < v.num; i++) {
				if (v.array[i].pts == round - 2) {
					da_erase(v, i);
					break;
				}
			}
		}
	}

	/* 40 pushes, 10 pops from the front and 8 matched erases. */
	const int64_t expected[] = {13, 14, 15, 16, 18, 19, 20, 21, 23, 24, 25,
				    26, 28, 29, 30, 31, 33, 34, 35, 36, 38, 39};
	assert_int_equal(v.num, sizeof(expected) / sizeof(expected[0]));
	for (size_t i = 0; i < v.num; i++) {
		assert_int_equal(v.array[i].pts, expected[i]);
		assert_int_equal(v.array[i].track, (uint32_t)(expected[i] % 3));
	}
	assert_int_equal(v.capacity, 32);

	da_free(v);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_layout),
		cmocka_unit_test(test_insert_into_empty),
		cmocka_unit_test(test_insert_front_middle_end),
		cmocka_unit_test(test_insert_growth),
		cmocka_unit_test(test_insert_records),
		cmocka_unit_test(test_erase),
		cmocka_unit_test(test_erase_range),
		cmocka_unit_test(test_erase_range_records),
		cmocka_unit_test(test_removal_leaves_tail_bytes),
		cmocka_unit_test(test_pop_front),
		cmocka_unit_test(test_push_back_new),
		cmocka_unit_test(test_alloc_accounting),
		cmocka_unit_test(test_resort_pattern),
		cmocka_unit_test(test_packet_time_pattern),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
