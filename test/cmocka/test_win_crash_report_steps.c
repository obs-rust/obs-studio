#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-win-crash-report.h"

/*
 * Characterization of the crash report rules of
 * libobs/obs-win-crash-handler.c, moved to libobs/obs-win-crash-report.c
 * before their Rust port:
 * - obs_win_crash_report_contains(): enum_all_modules()'s and
 *   enum_module()'s test of whether a module holds an address;
 * - obs_win_crash_report_is_crashed(), _in_pass() and _other_process():
 *   write_thread_trace()'s choice of the threads each of
 *   write_thread_traces()'s two passes traces;
 * - obs_win_crash_report_names_symbol(), _mask_address() and _mask_param():
 *   walk_stack()'s frame line and a 32-bit build's masks;
 * - obs_win_crash_report_name_prefix(): get_thread_name()'s leading space;
 * - obs_win_crash_report_pass_on(): when exception_handler() passes the
 *   exception on.
 * A copy of write_thread_traces()'s two passes runs over saved thread lists,
 * so the order the report traces them in is pinned too. Cases marked
 * "characterized, not endorsed" are odd but real.
 *
 * The functions take no Windows type and are hidden in libobs, so this test
 * compiles the C source into itself, calls only them, and runs on every OS.
 * No test runs the crash handler itself: it needs a real crash.
 */

static void module_edges(void **state)
{
	(void)state;
	const uint64_t base = 0x7FF612340000ull;
	const uint32_t size = 0x2000;

	assert_false(obs_win_crash_report_contains(base - 1, base, size));
	assert_true(obs_win_crash_report_contains(base, base, size));
	assert_true(obs_win_crash_report_contains(base + size - 1, base, size));
	assert_false(obs_win_crash_report_contains(base + size, base, size));
	assert_false(obs_win_crash_report_contains(0, base, size));
	assert_false(obs_win_crash_report_contains(UINT64_MAX, base, size));

	/* a module of size 0 holds nothing, not even its base */
	assert_false(obs_win_crash_report_contains(base, base, 0));

	/* a module at 0, and the largest size */
	assert_true(obs_win_crash_report_contains(0, 0, 1));
	assert_false(obs_win_crash_report_contains(1, 0, 1));
	assert_true(obs_win_crash_report_contains(0xFFFFFFFEull, 0, UINT32_MAX));
	assert_false(obs_win_crash_report_contains(0xFFFFFFFFull, 0, UINT32_MAX));

	/* a 32-bit build's addresses */
	assert_true(obs_win_crash_report_contains(0x00401000, 0x00400000, 0x10000));
	assert_false(obs_win_crash_report_contains(0x00410000, 0x00400000, 0x10000));
}

static void module_end_wraps(void **state)
{
	(void)state;

	/* characterized, not endorsed: base + size is computed in uint64_t, so
	 * a module that ends past 2^64 wraps and holds nothing */
	const uint64_t base = UINT64_MAX - 15;

	assert_false(obs_win_crash_report_contains(base, base, 32));
	assert_false(obs_win_crash_report_contains(UINT64_MAX, base, 32));
	assert_false(obs_win_crash_report_contains(5, base, 32));
	/* one that ends at exactly 2^64 too: its end wraps to 0 */
	assert_false(obs_win_crash_report_contains(base, base, 16));
	assert_false(obs_win_crash_report_contains(UINT64_MAX, base, 16));
	/* one byte short of it does not wrap */
	assert_true(obs_win_crash_report_contains(base, base, 15));
	assert_true(obs_win_crash_report_contains(UINT64_MAX - 1, base, 15));
	assert_false(obs_win_crash_report_contains(UINT64_MAX, base, 15));
}

static void thread_rules(void **state)
{
	(void)state;

	assert_true(obs_win_crash_report_is_crashed(0x1A2C, 0x1A2C));
	assert_false(obs_win_crash_report_is_crashed(0x1A2C, 0x1A2D));
	assert_false(obs_win_crash_report_is_crashed(0, UINT32_MAX));
	assert_true(obs_win_crash_report_is_crashed(UINT32_MAX, UINT32_MAX));

	/* the crashed thread in the first pass, every other in the second */
	assert_true(obs_win_crash_report_in_pass(true, true));
	assert_false(obs_win_crash_report_in_pass(true, false));
	assert_false(obs_win_crash_report_in_pass(false, true));
	assert_true(obs_win_crash_report_in_pass(false, false));

	assert_false(obs_win_crash_report_other_process(4242, 4242));
	assert_true(obs_win_crash_report_other_process(4, 4242));
	assert_true(obs_win_crash_report_other_process(UINT32_MAX, 0));
}

struct thread {
	uint32_t id;
	uint32_t owner;
};

#define MAX_TRACED 16

struct traced {
	uint32_t ids[MAX_TRACED];
	bool crashed[MAX_TRACED];
	size_t count;
};

/* write_thread_traces()'s two passes over a snapshot, with
 * write_thread_trace()'s checks in their order: the ids the report traces,
 * and which of them it labels "(Crashed)". */
static void trace_threads(const struct thread *threads, size_t count, uint32_t current_thread, uint32_t current_process,
			  struct traced *out)
{
	out->count = 0;
	for (int pass = 0; pass < 2; pass++) {
		bool first_thread = pass == 0;

		for (size_t i = 0; i < count; i++) {
			bool crash_thread = obs_win_crash_report_is_crashed(threads[i].id, current_thread);

			if (!obs_win_crash_report_in_pass(first_thread, crash_thread))
				continue;
			if (obs_win_crash_report_other_process(threads[i].owner, current_process))
				continue;

			assert_true(out->count < MAX_TRACED);
			out->ids[out->count] = threads[i].id;
			out->crashed[out->count] = crash_thread;
			out->count++;
		}
	}
}

static void thread_passes(void **state)
{
	(void)state;
	struct traced t;

	/* the crashed thread first, then the others in snapshot order, and
	 * another process's threads left out */
	const struct thread snapshot[] = {{0x10, 700}, {0x24, 700}, {0x31, 912}, {0x18, 700}, {0x2C, 700}};
	trace_threads(snapshot, 5, 0x18, 700, &t);
	assert_int_equal(t.count, 4);
	assert_int_equal(t.ids[0], 0x18);
	assert_true(t.crashed[0]);
	assert_int_equal(t.ids[1], 0x10);
	assert_false(t.crashed[1]);
	assert_int_equal(t.ids[2], 0x24);
	assert_false(t.crashed[2]);
	assert_int_equal(t.ids[3], 0x2C);
	assert_false(t.crashed[3]);

	/* no crashed thread in the snapshot: the first pass traces nothing */
	trace_threads(snapshot, 5, 0x99, 700, &t);
	assert_int_equal(t.count, 4);
	assert_int_equal(t.ids[0], 0x10);
	assert_int_equal(t.ids[1], 0x24);
	assert_int_equal(t.ids[2], 0x18);
	assert_int_equal(t.ids[3], 0x2C);
	for (size_t i = 0; i < t.count; i++)
		assert_false(t.crashed[i]);

	/* the crashed thread's id under another process is that process's
	 * thread: left out of both passes */
	const struct thread other[] = {{0x18, 912}, {0x10, 700}};
	trace_threads(other, 2, 0x18, 700, &t);
	assert_int_equal(t.count, 1);
	assert_int_equal(t.ids[0], 0x10);
	assert_false(t.crashed[0]);

	/* every thread in another process, and an empty snapshot */
	trace_threads(snapshot, 5, 0x18, 1, &t);
	assert_int_equal(t.count, 0);
	trace_threads(snapshot, 0, 0x18, 700, &t);
	assert_int_equal(t.count, 0);

	/* the same id twice, the crashed one: traced twice in the first pass */
	const struct thread twice[] = {{0x18, 700}, {0x10, 700}, {0x18, 700}};
	trace_threads(twice, 3, 0x18, 700, &t);
	assert_int_equal(t.count, 3);
	assert_int_equal(t.ids[0], 0x18);
	assert_int_equal(t.ids[1], 0x18);
	assert_int_equal(t.ids[2], 0x10);
	assert_true(t.crashed[0]);
	assert_true(t.crashed[1]);
	assert_false(t.crashed[2]);
}

/* walk_stack()'s choice: the line with the symbol when the lookup succeeded
 * and the symbol is not an export, else the line with the address. The flags
 * are read only after a successful lookup. */
static bool symbol_line(bool success, const uint32_t *flags)
{
	return success && obs_win_crash_report_names_symbol(*flags);
}

static void frame_line(void **state)
{
	(void)state;

	assert_int_equal(OBS_WIN_CRASH_REPORT_SYMFLAG_EXPORT, 0x200);

	assert_true(obs_win_crash_report_names_symbol(0));
	assert_false(obs_win_crash_report_names_symbol(0x200));
	assert_false(obs_win_crash_report_names_symbol(0x201));
	assert_false(obs_win_crash_report_names_symbol(0x1200));
	assert_false(obs_win_crash_report_names_symbol(UINT32_MAX));
	/* every other flag (SYMFLAG_FUNCTION 0x800, SYMFLAG_FORWARDER 0x400,
	 * ...) still names the symbol */
	assert_true(obs_win_crash_report_names_symbol(0x800));
	assert_true(obs_win_crash_report_names_symbol(0x400));
	assert_true(obs_win_crash_report_names_symbol(0x100));
	assert_true(obs_win_crash_report_names_symbol(~(uint32_t)0x200));

	const uint32_t plain = 0x800, exported = 0x200;
	assert_true(symbol_line(true, &plain));
	assert_false(symbol_line(true, &exported));
	/* a failed lookup (or no symbol buffer) never reads the flags */
	assert_false(symbol_line(false, NULL));
}

static void masks_32bit(void **state)
{
	(void)state;

	/* 32-bit addresses keep every bit */
	assert_int_equal(obs_win_crash_report_mask_address(0x0019FF3C), 0x0019FF3C);
	assert_int_equal(obs_win_crash_report_mask_address(0xFFFFFFFF), 0xFFFFFFFF);
	assert_int_equal(obs_win_crash_report_mask_param(0xFFFFFFFF), 0xFFFFFFFF);
	assert_int_equal(obs_win_crash_report_mask_param(0), 0);

	/* above 2^32: parameters keep 32 bits */
	assert_int_equal(obs_win_crash_report_mask_param(0x123456789ABCDEF0ull), 0x9ABCDEF0);
	assert_int_equal(obs_win_crash_report_mask_param(0x100000000ull), 0);
	assert_int_equal(obs_win_crash_report_mask_param(UINT64_MAX), 0xFFFFFFFF);

	/* characterized, not endorsed: the stack and PC keep 36 bits, nine Fs,
	 * where the parameters keep 32 (no effect on 32-bit addresses) */
	assert_int_equal(obs_win_crash_report_mask_address(0x123456789ABCDEF0ull), 0x89ABCDEF0ull);
	assert_int_equal(obs_win_crash_report_mask_address(0x100000000ull), 0x100000000ull);
	assert_int_equal(obs_win_crash_report_mask_address(UINT64_MAX), 0xFFFFFFFFFull);
	assert_int_equal(obs_win_crash_report_mask_address(0x1000000000ull), 0);
}

static void name_prefix(void **state)
{
	(void)state;

	assert_false(obs_win_crash_report_name_prefix(0));
	assert_true(obs_win_crash_report_name_prefix(1));
	assert_true(obs_win_crash_report_name_prefix(255));
	assert_true(obs_win_crash_report_name_prefix(SIZE_MAX));
}

static void handler_passes_on(void **state)
{
	(void)state;
	const bool inside = true, outside = false;

	/* a debugger: passed on, and the flag is never read */
	assert_true(obs_win_crash_report_pass_on(true, NULL));
	assert_true(obs_win_crash_report_pass_on(true, &inside));
	assert_true(obs_win_crash_report_pass_on(true, &outside));

	/* re-entered: passed on */
	assert_true(obs_win_crash_report_pass_on(false, &inside));

	/* neither: the report runs */
	assert_false(obs_win_crash_report_pass_on(false, &outside));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(module_edges),  cmocka_unit_test(module_end_wraps),  cmocka_unit_test(thread_rules),
		cmocka_unit_test(thread_passes), cmocka_unit_test(frame_line),        cmocka_unit_test(masks_32bit),
		cmocka_unit_test(name_prefix),   cmocka_unit_test(handler_passes_on),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
