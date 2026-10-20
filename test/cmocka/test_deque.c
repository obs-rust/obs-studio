#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

/*
 * Characterization tests for the header-inline double-ended queue in
 * libobs/util/deque.h (#1546), before its Rust equivalent in obs-util. They
 * pin the current C behavior: the struct layout, the growth rule (double the
 * capacity, or the new size when that is larger), where the data goes when it
 * wraps past the end of the buffer, how growth moves wrapped data, and the
 * bmalloc accounting. Cases marked "characterized, not endorsed" are odd but
 * real.
 *
 * The functions are static inline, so this tests the C header only in both
 * libobs builds; the Rust equivalent is checked against oracle wrappers of
 * these functions (Tier 3). The obs-output delay queue (a deque of struct
 * delay_data) is the first Rust user: see test_record_fifo.
 */

#include <util/bmem.h>
#include <util/deque.h>

static void assert_state(const struct deque *dq, size_t size, size_t start_pos, size_t end_pos, size_t capacity)
{
	assert_int_equal(dq->size, size);
	assert_int_equal(dq->start_pos, start_pos);
	assert_int_equal(dq->end_pos, end_pos);
	assert_int_equal(dq->capacity, capacity);
}

static void assert_front(struct deque *dq, const char *expected, size_t size)
{
	char buf[64] = {0};
	assert_true(size <= sizeof(buf));
	deque_peek_front(dq, buf, size);
	assert_memory_equal(buf, expected, size);
}

static void assert_raw(const struct deque *dq, size_t offset, const char *expected, size_t size)
{
	assert_memory_equal((const uint8_t *)dq->data + offset, expected, size);
}

/* A deque with capacity 8 holding "abcdef" from position 0. */
static void make_abcdef_in_8(struct deque *dq)
{
	deque_init(dq);
	deque_reserve(dq, 8);
	deque_push_back(dq, "abcdef", 6);
	assert_state(dq, 6, 0, 6, 8);
}

/* Full and wrapped: raw "XYZdefgh", start_pos 3, end_pos 3, capacity 8. */
static void make_full_wrapped(struct deque *dq)
{
	deque_init(dq);
	deque_push_back(dq, "abcdefgh", 8);
	deque_pop_front(dq, NULL, 3);
	deque_push_back(dq, "XYZ", 3);
	assert_state(dq, 8, 3, 3, 8);
	assert_raw(dq, 0, "XYZdefgh", 8);
}

static void test_layout(void **state)
{
	UNUSED_PARAMETER(state);

	struct align_probe {
		char c;
		struct deque dq;
	};

	assert_int_equal(sizeof(void *), sizeof(size_t));
	assert_int_equal(sizeof(struct deque), 5 * sizeof(size_t));
	assert_int_equal(offsetof(struct align_probe, dq), sizeof(size_t));
	assert_int_equal(offsetof(struct deque, data), 0);
	assert_int_equal(offsetof(struct deque, size), 1 * sizeof(size_t));
	assert_int_equal(offsetof(struct deque, start_pos), 2 * sizeof(size_t));
	assert_int_equal(offsetof(struct deque, end_pos), 3 * sizeof(size_t));
	assert_int_equal(offsetof(struct deque, capacity), 4 * sizeof(size_t));
}

static void test_init_zeroes(void **state)
{
	UNUSED_PARAMETER(state);

	struct deque dq;
	memset(&dq, 0xa5, sizeof(dq));
	deque_init(&dq);
	assert_null(dq.data);
	assert_state(&dq, 0, 0, 0, 0);
	assert_null(deque_data(&dq, 0));
}

static void test_growth_doubles_or_takes_size(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct deque dq;
	deque_init(&dq);

	/* The first push allocates exactly its size. */
	deque_push_back(&dq, "abcd", 4);
	assert_state(&dq, 4, 0, 4, 4);
	assert_int_equal(bnum_allocs(), allocs + 1);

	/* Then the capacity doubles... */
	deque_push_back(&dq, "e", 1);
	assert_state(&dq, 5, 0, 5, 8);

	deque_push_back(&dq, "fghijklmno", 10);
	assert_state(&dq, 15, 0, 15, 16);

	/* ...unless the new size is larger than double, then it is the size. */
	char twenty[20];
	memset(twenty, 'z', sizeof(twenty));
	deque_push_back(&dq, twenty, sizeof(twenty));
	assert_state(&dq, 35, 0, 35, 35);

	/* A full, unwrapped deque has end_pos == capacity, not 0. */
	assert_int_equal(dq.end_pos, dq.capacity);

	/* brealloc of an existing block is not a new allocation. */
	assert_int_equal(bnum_allocs(), allocs + 1);
	assert_front(&dq, "abcdefghijklmno", 15);

	deque_free(&dq);
	assert_int_equal(bnum_allocs(), allocs);
}

static void test_growth_takes_size_from_small(void **state)
{
	UNUSED_PARAMETER(state);

	struct deque dq;
	deque_init(&dq);
	deque_push_back(&dq, "abc", 3);
	assert_state(&dq, 3, 0, 3, 3);
	deque_push_back(&dq, "defghij", 7);
	assert_state(&dq, 10, 0, 10, 10);
	assert_front(&dq, "abcdefghij", 10);
	deque_free(&dq);
}

static void test_pop_front(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct deque dq;
	char buf[8] = {0};

	deque_init(&dq);
	deque_push_back(&dq, "abcdefgh", 8);

	deque_pop_front(&dq, buf, 3);
	assert_memory_equal(buf, "abc", 3);
	assert_state(&dq, 5, 3, 8, 8);
	assert_front(&dq, "defgh", 5);

	/* Popping to empty resets both positions and keeps the buffer. */
	deque_pop_front(&dq, NULL, 5);
	assert_state(&dq, 0, 0, 0, 8);
	assert_non_null(dq.data);
	assert_int_equal(bnum_allocs(), allocs + 1);

	/* The next push starts at position 0 again, without reallocating. */
	deque_push_back(&dq, "ij", 2);
	assert_state(&dq, 2, 0, 2, 8);
	assert_raw(&dq, 0, "ij", 2);
	assert_int_equal(bnum_allocs(), allocs + 1);

	deque_free(&dq);
	assert_int_equal(bnum_allocs(), allocs);
}

static void test_push_back_wraps_at_end(void **state)
{
	UNUSED_PARAMETER(state);

	struct deque dq;

	/* end_pos == capacity: the whole push goes to the start. */
	make_full_wrapped(&dq);
	assert_front(&dq, "defghXYZ", 8);
	deque_free(&dq);

	/* end_pos < capacity: the push is split across the end. */
	make_abcdef_in_8(&dq);
	deque_pop_front(&dq, NULL, 4);
	assert_state(&dq, 2, 4, 6, 8);
	deque_push_back(&dq, "UVWX", 4);
	assert_state(&dq, 6, 4, 2, 8);
	assert_raw(&dq, 0, "WXcdefUV", 8);
	assert_front(&dq, "efUVWX", 6);
	deque_free(&dq);
}

static void test_growth_moves_wrapped_tail(void **state)
{
	UNUSED_PARAMETER(state);

	struct deque dq;
	make_full_wrapped(&dq);

	/* The bytes from start_pos to the old end move to the new end; the
	 * wrapped head stays at 0 and the push lands after it. */
	deque_push_back(&dq, "1", 1);
	assert_state(&dq, 9, 11, 4, 16);
	assert_raw(&dq, 0, "XYZ1", 4);
	assert_raw(&dq, 11, "defgh", 5);
	assert_front(&dq, "defghXYZ1", 9);
	deque_free(&dq);
}

static void test_growth_unwrapped_with_offset_does_not_move(void **state)
{
	UNUSED_PARAMETER(state);

	struct deque dq;
	deque_init(&dq);
	deque_push_back(&dq, "abcdefgh", 8);
	deque_pop_front(&dq, NULL, 2);
	assert_state(&dq, 6, 2, 8, 8);

	/* Without growth this push would wrap; after growth it does not. */
	deque_push_back(&dq, "ijk", 3);
	assert_state(&dq, 9, 2, 11, 16);
	assert_raw(&dq, 2, "cdefghijk", 9);
	assert_front(&dq, "cdefghijk", 9);
	deque_free(&dq);
}

static void test_reserve(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct deque dq;
	deque_init(&dq);

	deque_reserve(&dq, 0);
	assert_null(dq.data);
	assert_state(&dq, 0, 0, 0, 0);

	deque_reserve(&dq, 16);
	assert_non_null(dq.data);
	assert_state(&dq, 0, 0, 0, 16);
	assert_int_equal(bnum_allocs(), allocs + 1);

	/* Never shrinks. */
	deque_reserve(&dq, 8);
	assert_state(&dq, 0, 0, 0, 16);
	deque_free(&dq);
	assert_int_equal(bnum_allocs(), allocs);

	/* Reserving on wrapped data moves the tail by the difference. */
	make_full_wrapped(&dq);
	deque_reserve(&dq, 12);
	assert_state(&dq, 8, 7, 3, 12);
	assert_raw(&dq, 0, "XYZ", 3);
	assert_raw(&dq, 7, "defgh", 5);
	assert_front(&dq, "defghXYZ", 8);

	/* The gap fills up to exactly the capacity, with no growth. */
	deque_push_back(&dq, "PQRS", 4);
	assert_state(&dq, 12, 7, 7, 12);
	assert_front(&dq, "defghXYZPQRS", 12);
	deque_free(&dq);
}

static void test_push_front(void **state)
{
	UNUSED_PARAMETER(state);

	struct deque dq;
	deque_init(&dq);

	/* Into an empty deque: at position 0. */
	deque_push_front(&dq, "abc", 3);
	assert_state(&dq, 3, 0, 3, 3);
	assert_raw(&dq, 0, "abc", 3);

	/* start_pos 0: wraps to the end of the (grown) buffer. */
	deque_push_front(&dq, "Z", 1);
	assert_state(&dq, 4, 5, 3, 6);
	assert_raw(&dq, 5, "Z", 1);
	assert_front(&dq, "Zabc", 4);

	/* start_pos >= size: just before start_pos. */
	deque_push_front(&dq, "Y", 1);
	assert_state(&dq, 5, 4, 3, 6);
	assert_front(&dq, "YZabc", 5);
	deque_free(&dq);

	/* 0 < start_pos < size: split, the tail of the push at 0. */
	make_abcdef_in_8(&dq);
	deque_pop_front(&dq, NULL, 2);
	deque_push_front(&dq, "WXYZ", 4);
	assert_state(&dq, 8, 6, 6, 8);
	assert_raw(&dq, 0, "YZcdefWX", 8);
	assert_front(&dq, "WXYZcdef", 8);

	/* Into a full wrapped deque: growth moves the tail first. */
	deque_push_front(&dq, "V", 1);
	assert_state(&dq, 9, 13, 6, 16);
	assert_front(&dq, "VWXYZcdef", 9);
	deque_free(&dq);
}

static void test_push_zero(void **state)
{
	UNUSED_PARAMETER(state);

	struct deque dq;

	make_abcdef_in_8(&dq);
	deque_pop_front(&dq, NULL, 4);
	assert_state(&dq, 2, 4, 6, 8);

	/* Split across the end; the bytes between end and start are kept. */
	deque_push_back_zero(&dq, 4);
	assert_state(&dq, 6, 4, 2, 8);
	assert_raw(&dq, 0, "\0\0cdef\0\0", 8);
	assert_front(&dq, "ef\0\0\0\0", 6);

	deque_push_front_zero(&dq, 2);
	assert_state(&dq, 8, 2, 2, 8);
	assert_front(&dq, "\0\0ef\0\0\0\0", 8);
	deque_free(&dq);

	deque_init(&dq);
	deque_push_front_zero(&dq, 3);
	assert_state(&dq, 3, 0, 3, 3);
	assert_raw(&dq, 0, "\0\0\0", 3);
	deque_free(&dq);

	make_abcdef_in_8(&dq);
	deque_pop_front(&dq, NULL, 2);
	deque_push_front_zero(&dq, 4);
	assert_state(&dq, 8, 6, 6, 8);
	assert_raw(&dq, 0, "\0\0cdef\0\0", 8);
	assert_front(&dq, "\0\0\0\0cdef", 8);
	deque_free(&dq);

	/* Into an empty deque from the back, with growth. */
	deque_init(&dq);
	deque_push_back(&dq, "a", 1);
	deque_push_back_zero(&dq, 2);
	assert_state(&dq, 3, 0, 3, 3);
	assert_front(&dq, "a\0\0", 3);
	deque_free(&dq);
}

static void test_peek_and_pop_back(void **state)
{
	UNUSED_PARAMETER(state);

	struct deque dq;
	char buf[8] = {0};

	deque_init(&dq);
	deque_push_back(&dq, "abcdefgh", 8);
	deque_peek_back(&dq, buf, 3);
	assert_memory_equal(buf, "fgh", 3);
	deque_pop_back(&dq, NULL, 3);
	assert_state(&dq, 5, 0, 5, 8);
	deque_pop_back(&dq, buf, 5);
	assert_memory_equal(buf, "abcde", 5);
	assert_state(&dq, 0, 0, 0, 8);
	deque_free(&dq);

	/* Wrapped: raw "WXcdefUV", content "efUVWX". */
	make_abcdef_in_8(&dq);
	deque_pop_front(&dq, NULL, 4);
	deque_push_back(&dq, "UVWX", 4);
	assert_state(&dq, 6, 4, 2, 8);

	memset(buf, 0, sizeof(buf));
	deque_peek_back(&dq, buf, 4);
	assert_memory_equal(buf, "UVWX", 4);

	/* characterized, not endorsed: popping exactly end_pos bytes leaves
	 * end_pos at capacity, not 0. */
	deque_pop_back(&dq, buf, 2);
	assert_memory_equal(buf, "WX", 2);
	assert_state(&dq, 4, 4, 8, 8);

	deque_peek_back(&dq, buf, 2);
	assert_memory_equal(buf, "UV", 2);
	deque_pop_back(&dq, NULL, 3);
	assert_state(&dq, 1, 4, 5, 8);
	assert_front(&dq, "e", 1);

	/* A null destination only moves the positions. */
	deque_peek_back(&dq, NULL, 1);
	deque_peek_front(&dq, NULL, 1);
	assert_state(&dq, 1, 4, 5, 8);
	deque_free(&dq);
}

static void test_data_index(void **state)
{
	UNUSED_PARAMETER(state);

	struct deque dq;
	make_abcdef_in_8(&dq);
	deque_pop_front(&dq, NULL, 4);
	deque_push_back(&dq, "UVWX", 4);

	/* Indexes are relative to start_pos and wrap at the capacity. */
	assert_ptr_equal(deque_data(&dq, 0), (uint8_t *)dq.data + 4);
	assert_int_equal(*(uint8_t *)deque_data(&dq, 0), 'e');
	assert_int_equal(*(uint8_t *)deque_data(&dq, 3), 'V');
	assert_ptr_equal(deque_data(&dq, 4), dq.data);
	assert_int_equal(*(uint8_t *)deque_data(&dq, 5), 'X');
	assert_null(deque_data(&dq, 6));
	assert_null(deque_data(&dq, SIZE_MAX));
	deque_free(&dq);
}

static void test_place_and_upsize(void **state)
{
	UNUSED_PARAMETER(state);

	struct deque dq;
	make_abcdef_in_8(&dq);
	deque_pop_front(&dq, NULL, 4);
	assert_state(&dq, 2, 4, 6, 8);

	/* Past the end: upsize zero-fills, then the copy wraps. */
	deque_place(&dq, 1, "PQRST", 5);
	assert_state(&dq, 6, 4, 2, 8);
	assert_raw(&dq, 0, "STcdePQR", 8);
	assert_front(&dq, "ePQRST", 6);

	/* Inside: an overwrite, no size change. */
	deque_place(&dq, 0, "Z", 1);
	assert_state(&dq, 6, 4, 2, 8);
	assert_front(&dq, "ZPQRST", 6);

	/* Shrinking or equal upsize is a no-op. */
	deque_upsize(&dq, 3);
	deque_upsize(&dq, 6);
	assert_state(&dq, 6, 4, 2, 8);

	/* A gap: upsize grows (moving the wrapped tail) and zero-fills it. */
	deque_place(&dq, 8, "!", 1);
	assert_state(&dq, 9, 12, 5, 16);
	assert_front(&dq, "ZPQRST\0\0!", 9);
	deque_free(&dq);

	/* upsize on an empty deque allocates and zero-fills. */
	deque_init(&dq);
	deque_upsize(&dq, 4);
	assert_state(&dq, 4, 0, 4, 4);
	assert_front(&dq, "\0\0\0\0", 4);
	deque_free(&dq);
}

static void test_free_resets(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct deque dq;

	/* Freeing a deque that never allocated is fine and frees nothing. */
	deque_init(&dq);
	deque_free(&dq);
	assert_null(dq.data);
	assert_state(&dq, 0, 0, 0, 0);
	assert_int_equal(bnum_allocs(), allocs);

	make_full_wrapped(&dq);
	assert_int_equal(bnum_allocs(), allocs + 1);
	deque_free(&dq);
	assert_null(dq.data);
	assert_state(&dq, 0, 0, 0, 0);
	assert_int_equal(bnum_allocs(), allocs);

	/* A freed deque is reusable. */
	deque_push_back(&dq, "ab", 2);
	assert_state(&dq, 2, 0, 2, 2);
	deque_free(&dq);
	assert_int_equal(bnum_allocs(), allocs);
}

/* The obs-output delay pattern: fixed-size records pushed at the back, peeked
 * and popped at the front, through growth and wraparound. */
struct record {
	uint64_t ts;
	uint32_t seq;
	int32_t kind;
};

static void test_record_fifo(void **state)
{
	UNUSED_PARAMETER(state);

	long allocs = bnum_allocs();
	struct deque dq;
	uint32_t next_push = 0;
	uint32_t next_pop = 0;
	size_t wrapped = 0;
	size_t max_capacity = 0;

	deque_init(&dq);
	for (uint32_t round = 0; round < 64; round++) {
		uint32_t pushes = round % 3 + 1;
		uint32_t pops = round % 2 + 1;

		for (uint32_t i = 0; i < pushes; i++) {
			struct record rec = {(uint64_t)next_push * 1000, next_push, (int32_t)(next_push % 4)};
			deque_push_back(&dq, &rec, sizeof(rec));
			next_push++;
		}

		/* Every record is where deque_data says, in push order. */
		for (uint32_t i = next_pop; i < next_push; i++) {
			struct record rec;
			memcpy(&rec, deque_data(&dq, (i - next_pop) * sizeof(rec)), sizeof(rec));
			assert_int_equal(rec.seq, i);
			assert_int_equal(rec.ts, (uint64_t)i * 1000);
		}

		for (uint32_t i = 0; i < pops && dq.size; i++) {
			struct record rec;
			deque_peek_front(&dq, &rec, sizeof(rec));
			assert_int_equal(rec.seq, next_pop);
			if (rec.kind == 1) {
				deque_pop_front(&dq, NULL, sizeof(rec));
			} else {
				struct record popped;
				deque_pop_front(&dq, &popped, sizeof(popped));
				assert_memory_equal(&popped, &rec, sizeof(rec));
			}
			next_pop++;
		}

		assert_int_equal(dq.size, (size_t)(next_push - next_pop) * sizeof(struct record));
		if (dq.size && dq.end_pos <= dq.start_pos)
			wrapped++;
		if (dq.capacity > max_capacity)
			max_capacity = dq.capacity;
	}

	/* 127 pushes, 96 pops; the capacity doubled from one record (16 bytes)
	 * to 64 records (1024 bytes), and the queue ended 36 rounds wrapped. */
	assert_int_equal(next_push, 127);
	assert_int_equal(next_pop, 96);
	assert_int_equal(dq.size, 31 * sizeof(struct record));
	assert_int_equal(sizeof(struct record), 16);
	assert_int_equal(max_capacity, 64 * sizeof(struct record));
	assert_int_equal(dq.capacity, max_capacity);
	assert_int_equal(wrapped, 36);
	assert_int_equal(bnum_allocs(), allocs + 1);

	while (dq.size) {
		struct record rec;
		deque_pop_front(&dq, &rec, sizeof(rec));
		assert_int_equal(rec.seq, next_pop);
		next_pop++;
	}
	assert_int_equal(next_pop, 127);
	assert_int_equal(dq.start_pos, 0);
	assert_int_equal(dq.end_pos, 0);

	deque_free(&dq);
	assert_int_equal(bnum_allocs(), allocs);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_layout),
		cmocka_unit_test(test_init_zeroes),
		cmocka_unit_test(test_growth_doubles_or_takes_size),
		cmocka_unit_test(test_growth_takes_size_from_small),
		cmocka_unit_test(test_pop_front),
		cmocka_unit_test(test_push_back_wraps_at_end),
		cmocka_unit_test(test_growth_moves_wrapped_tail),
		cmocka_unit_test(test_growth_unwrapped_with_offset_does_not_move),
		cmocka_unit_test(test_reserve),
		cmocka_unit_test(test_push_front),
		cmocka_unit_test(test_push_zero),
		cmocka_unit_test(test_peek_and_pop_back),
		cmocka_unit_test(test_data_index),
		cmocka_unit_test(test_place_and_upsize),
		cmocka_unit_test(test_free_resets),
		cmocka_unit_test(test_record_fifo),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
