#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <util/base.h>
#include <util/bmem.h>
#include <util/darray.h>
#include <util/dstr.h>
#include <util/platform.h>
#include <util/profiler.h>
#include <util/profiler-snapshot.h>

/*
 * Characterizes util/profiler-io.c: profiler_print and
 * profiler_print_time_between_calls (the log lines they write) and
 * profiler_snapshot_dump_csv / _csv_gz (the bytes they write). The snapshot
 * is built by hand through the private profiler-snapshot.h layout, so every
 * time and count is fixed, and freed with profile_snapshot_free.
 *
 * Two quirks are pinned, not endorsed: a median that falls on the boundary
 * of two time entries is reported as 0 ms (child1), and a calls-per-parent
 * ratio that rounds to 1.0 is not printed (leaf, 0.95).
 */

#define TMP_DIR "test_profiler_io.tmp"
#define CSV_FILE TMP_DIR "/out.csv"
#define GZ_FILE TMP_DIR "/out.csv.gz"

#define MS "\xc2\xa0ms"
#define PM "\xc2\xb1"

static const char root_name[] = "root";
static const char child1_name[] = "child1";
static const char leaf_name[] = "leaf";
static const char child2_name[] = "child2";
static const char other_name[] = "other";

struct captured {
	char lines[16][256];
	size_t num;
};

static void capture_log(int level, const char *format, va_list args, void *param)
{
	struct captured *c = param;
	assert_int_equal(level, LOG_INFO);
	assert_true(c->num < 16);
	vsnprintf(c->lines[c->num], sizeof(c->lines[c->num]), format, args);
	c->num++;
}

static void add_time(profiler_time_entries_t *entries, uint64_t delta, uint64_t count)
{
	profiler_time_entry_t *t = da_push_back_new(*entries);
	t->time_delta = delta;
	t->count = count;
}

static profiler_snapshot_entry_t *new_entry(profiler_snapshot_entry_t *e, const char *name, uint64_t min_,
					    uint64_t max_, uint64_t calls)
{
	e->name = name;
	e->min_time = min_;
	e->max_time = max_;
	e->overall_count = calls;
	return e;
}

static profiler_snapshot_t *build_snapshot(void)
{
	profiler_snapshot_t *snap = bzalloc(sizeof(*snap));

	/* root: spread times, an expected time between calls, two children */
	profiler_snapshot_entry_t *root = new_entry(da_push_back_new(snap->roots), root_name, 1000, 5000, 10);
	add_time(&root->times, 5000, 1);
	add_time(&root->times, 3000, 4);
	add_time(&root->times, 1000, 5);
	root->expected_time_between_calls = 2000;
	root->min_time_between_calls = 1500;
	root->max_time_between_calls = 2100;
	root->overall_between_calls_count = 9;
	add_time(&root->times_between_calls, 2100, 2);
	add_time(&root->times_between_calls, 2000, 5);
	add_time(&root->times_between_calls, 1900, 1);
	add_time(&root->times_between_calls, 1500, 1);

	profiler_snapshot_entry_t *child1 = new_entry(da_push_back_new(root->children), child1_name, 500, 1500, 20);
	add_time(&child1->times, 1500, 10);
	add_time(&child1->times, 500, 10);

	profiler_snapshot_entry_t *leaf = new_entry(da_push_back_new(child1->children), leaf_name, 250, 250, 19);
	add_time(&leaf->times, 250, 19);

	profiler_snapshot_entry_t *child2 = new_entry(da_push_back_new(root->children), child2_name, 100, 100, 15);
	add_time(&child2->times, 100, 15);
	child2->expected_time_between_calls = 1000;
	child2->min_time_between_calls = 1000;
	child2->max_time_between_calls = 1000;
	child2->overall_between_calls_count = 14;
	add_time(&child2->times_between_calls, 1000, 14);

	/* other: one call, every time between calls above the expected time */
	profiler_snapshot_entry_t *other = new_entry(da_push_back_new(snap->roots), other_name, 7, 7, 1);
	add_time(&other->times, 7, 1);
	other->expected_time_between_calls = 5;
	other->min_time_between_calls = 9;
	other->max_time_between_calls = 9;
	other->overall_between_calls_count = 3;
	add_time(&other->times_between_calls, 9, 3);

	return snap;
}

static void assert_lines(const struct captured *c, const char *const *expected, size_t num)
{
	assert_int_equal(c->num, num);
	for (size_t i = 0; i < num; i++)
		assert_string_equal(c->lines[i], expected[i]);
}

static void test_profiler_print(void **state)
{
	UNUSED_PARAMETER(state);

	log_handler_t prev = NULL;
	void *prev_param = NULL;
	struct captured c = {0};
	profiler_snapshot_t *snap = build_snapshot();

	base_get_log_handler(&prev, &prev_param);
	base_set_log_handler(capture_log, &c);
	profiler_print(snap);
	base_set_log_handler(prev, prev_param);

	static const char *const expected[] = {
		"== Profiler Results =============================",
		"root: min=1" MS ", median=3" MS ", max=5" MS ", 99th percentile=5" MS ", 50% below 2" MS,
		" \xe2\x94\xa3"
		"child1: min=0.5" MS ", median=0" MS ", max=1.5" MS ", 99th percentile=1.5" MS
		", 2 calls per parent call",
		" \xe2\x94\x83 \xe2\x94\x97"
		"leaf: 0.25" MS,
		" \xe2\x94\x97"
		"child2: 0.1" MS ", 1.5 calls per parent call",
		"other: 0.007" MS,
		"=================================================",
	};
	assert_lines(&c, expected, sizeof(expected) / sizeof(expected[0]));

	profile_snapshot_free(snap);
}

static void test_profiler_print_time_between_calls(void **state)
{
	UNUSED_PARAMETER(state);

	log_handler_t prev = NULL;
	void *prev_param = NULL;
	struct captured c = {0};
	profiler_snapshot_t *snap = build_snapshot();

	base_get_log_handler(&prev, &prev_param);
	base_set_log_handler(capture_log, &c);
	profiler_print_time_between_calls(snap);
	base_set_log_handler(prev, prev_param);

	/* entries without an expected time (child1, and so leaf) print nothing */
	static const char *const expected[] = {
		"== Profiler Time Between Calls ==================",
		"root: min=1.5" MS ", median=2" MS ", max=2.1" MS ", 55.5556% within " PM "2% of 2" MS
		" (22.2222% lower, 22.2222% higher)",
		" \xe2\x94\x97"
		"child2: min=1" MS ", median=1" MS ", max=1" MS ", 100% within " PM "2% of 1" MS
		" (0% lower, 0% higher)",
		"other: min=0.009" MS ", median=0.009" MS ", max=0.009" MS ", 0% within " PM "2% of 0.005" MS
		" (0% lower, 100% higher)",
		"=================================================",
	};
	assert_lines(&c, expected, sizeof(expected) / sizeof(expected[0]));

	profile_snapshot_free(snap);
}

static void test_profiler_print_live_snapshot(void **state)
{
	UNUSED_PARAMETER(state);

	log_handler_t prev = NULL;
	void *prev_param = NULL;
	struct captured c = {0};

	/* NULL takes (and frees) a snapshot of the profiler, empty when it never ran */
	base_get_log_handler(&prev, &prev_param);
	base_set_log_handler(capture_log, &c);
	profiler_print(NULL);
	profiler_print_time_between_calls(NULL);
	base_set_log_handler(prev, prev_param);

	static const char *const expected[] = {
		"== Profiler Results =============================",
		"=================================================",
		"== Profiler Time Between Calls ==================",
		"=================================================",
	};
	assert_lines(&c, expected, sizeof(expected) / sizeof(expected[0]));
}

static void cat_row(struct dstr *out, const profiler_snapshot_entry_t *e, const profiler_snapshot_entry_t *parent,
		    uint64_t between, uint64_t delta, uint64_t count)
{
	char line[512];
	const char *parent_name = parent ? parent->name : NULL;
	snprintf(line, sizeof(line), "%p,%p,%p,%p,%s,%llu,%llu,%llu\n", (void *)e, (void *)parent, (void *)e->name,
		 (void *)parent_name, e->name, (unsigned long long)between, (unsigned long long)delta,
		 (unsigned long long)count);
	dstr_cat(out, line);
}

static void cat_entry(struct dstr *out, const profiler_snapshot_entry_t *e, const profiler_snapshot_entry_t *parent)
{
	for (size_t i = 0; i < e->times.num; i++)
		cat_row(out, e, parent, 0, e->times.array[i].time_delta, e->times.array[i].count);
	for (size_t i = 0; i < e->times_between_calls.num; i++)
		cat_row(out, e, parent, e->expected_time_between_calls, e->times_between_calls.array[i].time_delta,
			e->times_between_calls.array[i].count);
	for (size_t i = 0; i < e->children.num; i++)
		cat_entry(out, &e->children.array[i], e);
}

static void test_profiler_dump_csv(void **state)
{
	UNUSED_PARAMETER(state);

	profiler_snapshot_t *snap = build_snapshot();

	struct dstr expected = {0};
	dstr_copy(&expected,
		  "id,parent_id,name_id,parent_name_id,name,time_between_calls,time_delta_\xc2\xb5s,count\n");
	for (size_t i = 0; i < snap->roots.num; i++)
		cat_entry(&expected, &snap->roots.array[i], NULL);

	assert_true(os_mkdirs(TMP_DIR) != MKDIR_ERROR);

	/* "wb+": an existing file is truncated */
	FILE *f = os_fopen(CSV_FILE, "wb");
	assert_non_null(f);
	for (int i = 0; i < 64; i++)
		fputs("stale stale stale stale stale stale stale stale stale stale\n", f);
	fclose(f);

	assert_true(profiler_snapshot_dump_csv(snap, CSV_FILE));
	char *csv = os_quick_read_utf8_file(CSV_FILE);
	assert_non_null(csv);
	assert_string_equal(csv, expected.array);
	bfree(csv);

	/* an empty snapshot writes the header alone */
	profiler_snapshot_t *empty = bzalloc(sizeof(*empty));
	assert_true(profiler_snapshot_dump_csv(empty, CSV_FILE));
	csv = os_quick_read_utf8_file(CSV_FILE);
	assert_non_null(csv);
	assert_string_equal(csv,
			    "id,parent_id,name_id,parent_name_id,name,time_between_calls,time_delta_\xc2\xb5s,count\n");
	bfree(csv);
	profile_snapshot_free(empty);

	/* gz: a gzip stream (magic, deflate) is written; a missing directory fails */
	assert_true(profiler_snapshot_dump_csv_gz(snap, GZ_FILE));
	f = os_fopen(GZ_FILE, "rb");
	assert_non_null(f);
	unsigned char magic[3] = {0};
	assert_int_equal(fread(magic, 1, 3, f), 3);
	fclose(f);
	assert_int_equal(magic[0], 0x1f);
	assert_int_equal(magic[1], 0x8b);
	assert_int_equal(magic[2], 8);

	assert_false(profiler_snapshot_dump_csv(snap, TMP_DIR "/no_such_dir/out.csv"));
	assert_false(profiler_snapshot_dump_csv_gz(snap, TMP_DIR "/no_such_dir/out.csv.gz"));

	dstr_free(&expected);
	profile_snapshot_free(snap);
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	os_unlink(CSV_FILE);
	os_unlink(GZ_FILE);
	os_rmdir(TMP_DIR);
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_profiler_print),
		cmocka_unit_test(test_profiler_print_time_between_calls),
		cmocka_unit_test(test_profiler_print_live_snapshot),
		cmocka_unit_test_teardown(test_profiler_dump_csv, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
