#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <time.h>
#include <cmocka.h>

#include <util/platform.h>

/*
 * Characterization tests for os_nstime_to_timespec in
 * libobs/util/platform-time.c (#262 slice 6), which no other test calls. They
 * pin the current C behavior for the Rust port; cases marked "characterized,
 * not endorsed" are odd but real.
 *
 * The first call takes a base pair: the wall clock (timespec_get) and
 * os_gettime_ns(). Every later result is that wall time plus the distance of
 * the timestamp from the base timestamp. Neither half of the base can be read
 * back, but their difference K = wall - base_ns can: group_setup makes the
 * first call with a timestamp taken just before it, which is at or below the
 * base timestamp, so the result is exact and gives K. Every test then
 * predicts results from K alone, so nothing depends on how much time passes,
 * except the bounds in test_first_call_sets_the_base.
 */

#define NS_PER_SEC 1000000000LL

struct split {
	int64_t sec;
	int64_t nsec;
};

/* around the first call */
static uint64_t mono_before;
static uint64_t mono_after;
static struct timespec wall_before;
static struct timespec wall_after;
static struct timespec first;
static struct timespec *first_ret;

/* K = wall - base_ns, with 0 <= nsec < NS_PER_SEC */
static struct split k;

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	if (timespec_get(&wall_before, TIME_UTC) != TIME_UTC)
		return -1;
	mono_before = os_gettime_ns();
	first_ret = os_nstime_to_timespec(mono_before, &first);
	mono_after = os_gettime_ns();
	if (timespec_get(&wall_after, TIME_UTC) != TIME_UTC)
		return -1;
	if (first_ret != &first)
		return -1;

	k.sec = (int64_t)first.tv_sec - (int64_t)(mono_before / NS_PER_SEC);
	k.nsec = (int64_t)first.tv_nsec - (int64_t)(mono_before % NS_PER_SEC);
	if (k.nsec < 0) {
		k.nsec += NS_PER_SEC;
		k.sec -= 1;
	}
	return 0;
}

/* K + timestamp, normalized */
static struct split expected(uint64_t timestamp)
{
	struct split out = {
		k.sec + (int64_t)(timestamp / NS_PER_SEC),
		k.nsec + (int64_t)(timestamp % NS_PER_SEC),
	};
	if (out.nsec >= NS_PER_SEC) {
		out.nsec -= NS_PER_SEC;
		out.sec += 1;
	}
	return out;
}

static void assert_timespec(const struct timespec *ts, int64_t sec, int64_t nsec)
{
	assert_int_equal((int64_t)ts->tv_sec, sec);
	assert_int_equal((int64_t)ts->tv_nsec, nsec);
}

/*
 * Converts `timestamp` and checks it against K. Above the base (`above`),
 * a result that lands on a whole second comes back as the previous second
 * plus 1000000000 ns: see test_whole_second_above_the_base. Below the base it
 * is normalized.
 */
static void check(uint64_t timestamp, bool above)
{
	struct timespec ts = {-1, -1};
	struct split want = expected(timestamp);

	assert_ptr_equal(os_nstime_to_timespec(timestamp, &ts), &ts);
	if (above && want.nsec == 0)
		assert_timespec(&ts, want.sec - 1, NS_PER_SEC);
	else
		assert_timespec(&ts, want.sec, want.nsec);
}

/* the smallest timestamp >= `from` whose result has `nsec` nanoseconds */
static uint64_t with_nsec(uint64_t from, int64_t nsec)
{
	int64_t now = expected(from).nsec;
	return from + (uint64_t)((nsec - now + NS_PER_SEC) % NS_PER_SEC);
}

static void test_first_call_sets_the_base(void **state)
{
	UNUSED_PARAMETER(state);

	assert_ptr_equal(first_ret, &first);
	assert_true(first.tv_nsec >= 0 && first.tv_nsec < NS_PER_SEC);

	/* the first call's result is the wall time when it ran, minus the time
	 * between mono_before and the base timestamp, so it lies in
	 * [wall_before - (mono_after - mono_before), wall_after] */
	int64_t first_ns = (int64_t)first.tv_sec * NS_PER_SEC + first.tv_nsec;
	int64_t lo =
		(int64_t)wall_before.tv_sec * NS_PER_SEC + wall_before.tv_nsec - (int64_t)(mono_after - mono_before);
	int64_t hi = (int64_t)wall_after.tv_sec * NS_PER_SEC + wall_after.tv_nsec;
	assert_true(first_ns >= lo);
	assert_true(first_ns <= hi);

	/* later calls keep the base: the same timestamp gives the same result */
	struct timespec again;
	assert_ptr_equal(os_nstime_to_timespec(mono_before, &again), &again);
	assert_timespec(&again, first.tv_sec, first.tv_nsec);
}

static void test_null_storage(void **state)
{
	UNUSED_PARAMETER(state);

	assert_null(os_nstime_to_timespec(0, NULL));
	assert_null(os_nstime_to_timespec(mono_after, NULL));
	assert_null(os_nstime_to_timespec(UINT64_MAX, NULL));

	/* and the base is unchanged */
	check(mono_before, false);
}

static void test_storage_contents_are_ignored(void **state)
{
	UNUSED_PARAMETER(state);

	struct timespec a = {0, 0};
	struct timespec b = {-12345, 999999999};
	uint64_t t = with_nsec(mono_after, 123456789);

	assert_ptr_equal(os_nstime_to_timespec(t, &a), &a);
	assert_ptr_equal(os_nstime_to_timespec(t, &b), &b);
	assert_timespec(&b, a.tv_sec, a.tv_nsec);
	assert_timespec(&a, expected(t).sec, 123456789);
}

static void test_around_the_base(void **state)
{
	UNUSED_PARAMETER(state);

	/* the base timestamp is in [mono_before, mono_after]: every timestamp
	 * there, one of them equal to the base, follows K. Either form of a
	 * whole second is accepted in this window, since which side of the base
	 * a timestamp is on is not known here. */
	uint64_t end = mono_after;
	if (end - mono_before > 100000)
		end = mono_before + 100000;
	for (uint64_t t = mono_before; t <= end; t++) {
		struct timespec ts;
		struct split want = expected(t);

		assert_ptr_equal(os_nstime_to_timespec(t, &ts), &ts);
		if (want.nsec == 0 && ts.tv_nsec == NS_PER_SEC)
			assert_timespec(&ts, want.sec - 1, NS_PER_SEC);
		else
			assert_timespec(&ts, want.sec, want.nsec);
	}
}

static void test_above_the_base(void **state)
{
	UNUSED_PARAMETER(state);

	uint64_t t = with_nsec(mono_after, 500000000);

	/* sub-second steps */
	check(t, true);
	check(t + 1, true);
	check(t + 499999999, true);
	check(t + 600000000, true);
	check(t + 999999999, true);

	/* exact seconds keep the nanoseconds */
	check(t + 1000000000, true);
	check(t + 5 * 1000000000ULL, true);
	check(t + 86400 * 1000000000ULL, true);

	struct timespec a, b;
	os_nstime_to_timespec(t, &a);
	os_nstime_to_timespec(t + 3 * 1000000000ULL, &b);
	assert_timespec(&b, (int64_t)a.tv_sec + 3, a.tv_nsec);

	/* a spread of offsets, which crosses the nanosecond carry */
	for (uint64_t i = 0; i < 64; i++)
		check(mono_after + i * 123456789ULL, true);
}

static void test_nanosecond_carry_above_the_base(void **state)
{
	UNUSED_PARAMETER(state);

	uint64_t t = with_nsec(mono_after, 1);
	struct timespec ts;

	/* one past a whole second and one before it are normalized */
	assert_ptr_equal(os_nstime_to_timespec(t, &ts), &ts);
	assert_timespec(&ts, expected(t).sec, 1);

	t = with_nsec(mono_after, 999999999);
	assert_ptr_equal(os_nstime_to_timespec(t, &ts), &ts);
	assert_timespec(&ts, expected(t).sec, 999999999);
}

static void test_whole_second_above_the_base(void **state)
{
	UNUSED_PARAMETER(state);

	/* characterized, not endorsed: above the base, the nanoseconds are the
	 * base's plus the offset's, and the carry only fires when that sum is
	 * greater than 1000000000. A sum of exactly 1000000000 is returned as
	 * tv_nsec == 1000000000 with tv_sec one short, which is not a normalized
	 * timespec. Every result whose time is a whole second does this, unless
	 * the base wall time itself was a whole second (tv_nsec 0), which this
	 * test cannot rule out but which is a 1-in-10^7 chance even at Windows'
	 * 100 ns resolution. */
	uint64_t t = with_nsec(mono_after, 0);
	struct timespec ts;

	assert_ptr_equal(os_nstime_to_timespec(t, &ts), &ts);
	assert_timespec(&ts, expected(t).sec - 1, NS_PER_SEC);

	assert_ptr_equal(os_nstime_to_timespec(t + 7 * 1000000000ULL, &ts), &ts);
	assert_timespec(&ts, expected(t).sec + 6, NS_PER_SEC);

	/* one nanosecond either side is normalized */
	check(t - 1, true);
	check(t + 1, true);
}

static void test_below_the_base(void **state)
{
	UNUSED_PARAMETER(state);

	/* below the base, a borrow takes a second when the offset's nanoseconds
	 * are more than the base's; the result is always normalized, including
	 * on whole seconds */
	check(0, false);
	check(1, false);
	check(999999999, false);
	check(1000000000, false);
	check(mono_before - 1, false);
	if (mono_before > 3 * 1000000000ULL) {
		check(mono_before - 1000000000, false);
		check(with_nsec(mono_before - 3 * 1000000000ULL, 0), false);
	}

	/* a spread of offsets, so both the borrow and the no-borrow branch run */
	for (uint64_t i = 1; i <= 64; i++) {
		uint64_t offset = i * 98765431ULL;
		if (offset <= mono_before)
			check(mono_before - offset, false);
	}
}

static void test_large_timestamps(void **state)
{
	UNUSED_PARAMETER(state);

	/* about 584 years past the base: tv_sec still fits a 64-bit time_t */
	check(UINT64_MAX, true);
	check(UINT64_MAX - 1, true);
	check(UINT64_MAX - 1000000000, true);
	check(1ULL << 63, true);
	check((1ULL << 63) - 1, true);

	struct timespec ts;
	assert_ptr_equal(os_nstime_to_timespec(UINT64_MAX, &ts), &ts);
	assert_true(ts.tv_sec > first.tv_sec + (time_t)18000000000LL);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_first_call_sets_the_base),
		cmocka_unit_test(test_null_storage),
		cmocka_unit_test(test_storage_contents_are_ignored),
		cmocka_unit_test(test_around_the_base),
		cmocka_unit_test(test_above_the_base),
		cmocka_unit_test(test_nanosecond_carry_above_the_base),
		cmocka_unit_test(test_whole_second_above_the_base),
		cmocka_unit_test(test_below_the_base),
		cmocka_unit_test(test_large_timestamps),
	};

	return cmocka_run_group_tests(tests, group_setup, NULL);
}
