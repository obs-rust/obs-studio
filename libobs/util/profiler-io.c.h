/*
 * Print and CSV half of util/profiler.c. The C reference compiles it; the
 * hybrid libobs uses the Rust port (obs-util::profiler_io).
 *
 * Moved verbatim so the recording engine can be swapped for the Rust port
 * without taking zlib or the dstr printf printers with it. profile_print_func
 * stays a non-static C symbol, as it was in profiler.c.
 */

#include "build/libobs-ported-profiler-io.pch.h"

typedef void (*profile_entry_print_func)(profiler_snapshot_entry_t *entry, struct dstr *indent_buffer,
					 struct dstr *output_buffer, unsigned indent, uint64_t active,
					 uint64_t parent_calls);

/* UTF-8 characters */
#define VPIPE_RIGHT " \xe2\x94\xa3"
#define VPIPE " \xe2\x94\x83"
#define DOWN_RIGHT " \xe2\x94\x97"

static void make_indent_string(struct dstr *indent_buffer, unsigned indent, uint64_t active)
{
	indent_buffer->len = 0;

	if (!indent) {
		dstr_cat_ch(indent_buffer, 0);
		return;
	}

	for (size_t i = 0; i < indent; i++) {
		const char *fragment = "";
		bool last = i + 1 == indent;
		if (active & ((uint64_t)1 << i))
			fragment = last ? VPIPE_RIGHT : VPIPE;
		else
			fragment = last ? DOWN_RIGHT : "  ";

		dstr_cat(indent_buffer, fragment);
	}
}

static void gather_stats(uint64_t expected_time_between_calls, profiler_time_entries_t *entries, uint64_t calls,
			 uint64_t *percentile99, uint64_t *median, double *percent_within_bounds)
{
	if (!entries->num) {
		*percentile99 = 0;
		*median = 0;
		*percent_within_bounds = 0.;
		return;
	}

	/*if (entry_buffer->num > 2)
		blog(LOG_INFO, "buffer-size %lu, overall count %llu\n"
				"map-size %lu, occupied %lu, probes %lu",
				entry_buffer->num, calls,
				map->size, map->occupied,
				map->max_probe_count);*/
	uint64_t accu = 0;
	for (size_t i = 0; i < entries->num; i++) {
		uint64_t old_accu = accu;
		accu += entries->array[i].count;

		if (old_accu < calls * 0.01 && accu >= calls * 0.01)
			*percentile99 = entries->array[i].time_delta;
		else if (old_accu < calls * 0.5 && accu >= calls * 0.5) {
			*median = entries->array[i].time_delta;
			break;
		}
	}

	*percent_within_bounds = 0.;
	if (!expected_time_between_calls)
		return;

	accu = 0;
	for (size_t i = 0; i < entries->num; i++) {
		profiler_time_entry_t *entry = &entries->array[i];
		if (entry->time_delta < expected_time_between_calls)
			break;

		accu += entry->count;
	}
	*percent_within_bounds = (1. - (double)accu / calls) * 100;
}

#define G_MS "g\xC2\xA0ms"

static void profile_print_entry(profiler_snapshot_entry_t *entry, struct dstr *indent_buffer,
				struct dstr *output_buffer, unsigned indent, uint64_t active, uint64_t parent_calls)
{
	uint64_t calls = entry->overall_count;
	uint64_t min_ = entry->min_time;
	uint64_t max_ = entry->max_time;
	uint64_t percentile99 = 0;
	uint64_t median = 0;
	double percent_within_bounds = 0.;
	gather_stats(entry->expected_time_between_calls, &entry->times, calls, &percentile99, &median,
		     &percent_within_bounds);

	make_indent_string(indent_buffer, indent, active);

	if (min_ == max_) {
		dstr_printf(output_buffer, "%s%s: %" G_MS, indent_buffer->array, entry->name, min_ / 1000.);
	} else {
		dstr_printf(output_buffer,
			    "%s%s: min=%" G_MS ", median=%" G_MS ", "
			    "max=%" G_MS ", 99th percentile=%" G_MS,
			    indent_buffer->array, entry->name, min_ / 1000., median / 1000., max_ / 1000.,
			    percentile99 / 1000.);

		if (entry->expected_time_between_calls) {
			double expected_ms = entry->expected_time_between_calls / 1000.;
			dstr_catf(output_buffer, ", %g%% below %" G_MS, percent_within_bounds, expected_ms);
		}
	}

	if (parent_calls && calls != parent_calls) {
		double calls_per_parent = (double)calls / parent_calls;
		if (lround(calls_per_parent * 10) != 10)
			dstr_catf(output_buffer, ", %g calls per parent call", calls_per_parent);
	}

	blog(LOG_INFO, "%s", output_buffer->array);

	active |= (uint64_t)1 << indent;
	for (size_t i = 0; i < entry->children.num; i++) {
		if ((i + 1) == entry->children.num)
			active &= (1 << indent) - 1;
		profile_print_entry(&entry->children.array[i], indent_buffer, output_buffer, indent + 1, active, calls);
	}
}

static void gather_stats_between(profiler_time_entries_t *entries, uint64_t calls, uint64_t lower_bound,
				 uint64_t upper_bound, uint64_t min_, uint64_t max_, uint64_t *median, double *percent,
				 double *lower, double *higher)
{
	*median = 0;
	*percent = 0.;
	*lower = 0.;
	*higher = 0.;

	if (!entries->num)
		return;

	uint64_t accu = 0;
	for (size_t i = 0; i < entries->num; i++) {
		accu += entries->array[i].count;
		if (accu < calls * 0.5)
			continue;

		*median = entries->array[i].time_delta;
		break;
	}

	bool found_upper_bound = max_ <= upper_bound;
	bool found_lower_bound = false;

	if (min_ >= upper_bound) {
		*higher = 100.;
		return;
	}

	if (found_upper_bound && min_ >= lower_bound) {
		*percent = 100.;
		return;
	}

	accu = 0;
	for (size_t i = 0; i < entries->num; i++) {
		uint64_t delta = entries->array[i].time_delta;

		if (!found_upper_bound && delta <= upper_bound) {
			*higher = (double)accu / calls * 100;
			accu = 0;
			found_upper_bound = true;
		}

		if (!found_lower_bound && delta < lower_bound) {
			*percent = (double)accu / calls * 100;
			accu = 0;
			found_lower_bound = true;
		}

		accu += entries->array[i].count;
	}

	if (!found_upper_bound) {
		*higher = 100.;

	} else if (!found_lower_bound) {
		*percent = (double)accu / calls * 100;

	} else {
		*lower = (double)accu / calls * 100;
	}
}

static void profile_print_entry_expected(profiler_snapshot_entry_t *entry, struct dstr *indent_buffer,
					 struct dstr *output_buffer, unsigned indent, uint64_t active,
					 uint64_t parent_calls)
{
	UNUSED_PARAMETER(parent_calls);

	if (!entry->expected_time_between_calls)
		return;

	uint64_t expected_time = entry->expected_time_between_calls;

	uint64_t min_ = entry->min_time_between_calls;
	uint64_t max_ = entry->max_time_between_calls;
	uint64_t median = 0;
	double percent = 0.;
	double lower = 0.;
	double higher = 0.;
	gather_stats_between(&entry->times_between_calls, entry->overall_between_calls_count,
			     (uint64_t)(expected_time * 0.98), (uint64_t)(expected_time * 1.02 + 0.5), min_, max_,
			     &median, &percent, &lower, &higher);

	make_indent_string(indent_buffer, indent, active);

	blog(LOG_INFO,
	     "%s%s: min=%" G_MS ", median=%" G_MS ", max=%" G_MS ", %g%% "
	     "within ±2%% of %" G_MS " (%g%% lower, %g%% higher)",
	     indent_buffer->array, entry->name, min_ / 1000., median / 1000., max_ / 1000., percent,
	     expected_time / 1000., lower, higher);

	active |= (uint64_t)1 << indent;
	for (size_t i = 0; i < entry->children.num; i++) {
		if ((i + 1) == entry->children.num)
			active &= (1 << indent) - 1;
		profile_print_entry_expected(&entry->children.array[i], indent_buffer, output_buffer, indent + 1,
					     active, 0);
	}
}

void profile_print_func(const char *intro, profile_entry_print_func print, profiler_snapshot_t *snap)
{
	struct dstr indent_buffer = {0};
	struct dstr output_buffer = {0};

	bool free_snapshot = !snap;
	if (!snap)
		snap = profile_snapshot_create();

	blog(LOG_INFO, "%s", intro);
	for (size_t i = 0; i < snap->roots.num; i++) {
		print(&snap->roots.array[i], &indent_buffer, &output_buffer, 0, 0, 0);
	}
	blog(LOG_INFO, "=================================================");

	if (free_snapshot)
		profile_snapshot_free(snap);

	dstr_free(&output_buffer);
	dstr_free(&indent_buffer);
}

void profiler_print(profiler_snapshot_t *snap)
{
	profile_print_func("== Profiler Results =============================", profile_print_entry, snap);
}

void profiler_print_time_between_calls(profiler_snapshot_t *snap)
{
	profile_print_func("== Profiler Time Between Calls ==================", profile_print_entry_expected, snap);
}

typedef void (*dump_csv_func)(void *data, struct dstr *buffer);
static void entry_dump_csv(struct dstr *buffer, const profiler_snapshot_entry_t *parent,
			   const profiler_snapshot_entry_t *entry, dump_csv_func func, void *data)
{
	const char *parent_name = parent ? parent->name : NULL;

	for (size_t i = 0; i < entry->times.num; i++) {
		dstr_printf(buffer,
			    "%p,%p,%p,%p,%s,0,"
			    "%" PRIu64 ",%" PRIu64 "\n",
			    entry, parent, entry->name, parent_name, entry->name, entry->times.array[i].time_delta,
			    entry->times.array[i].count);
		func(data, buffer);
	}

	for (size_t i = 0; i < entry->times_between_calls.num; i++) {
		dstr_printf(buffer,
			    "%p,%p,%p,%p,%s,"
			    "%" PRIu64 ",%" PRIu64 ",%" PRIu64 "\n",
			    entry, parent, entry->name, parent_name, entry->name, entry->expected_time_between_calls,
			    entry->times_between_calls.array[i].time_delta, entry->times_between_calls.array[i].count);
		func(data, buffer);
	}

	for (size_t i = 0; i < entry->children.num; i++)
		entry_dump_csv(buffer, entry, &entry->children.array[i], func, data);
}

static void profiler_snapshot_dump(const profiler_snapshot_t *snap, dump_csv_func func, void *data)
{
	struct dstr buffer = {0};

	dstr_init_copy(&buffer, "id,parent_id,name_id,parent_name_id,name,"
				"time_between_calls,time_delta_µs,count\n");
	func(data, &buffer);

	for (size_t i = 0; i < snap->roots.num; i++)
		entry_dump_csv(&buffer, NULL, &snap->roots.array[i], func, data);

	dstr_free(&buffer);
}

static void dump_csv_fwrite(void *data, struct dstr *buffer)
{
	fwrite(buffer->array, 1, buffer->len, data);
}

bool profiler_snapshot_dump_csv(const profiler_snapshot_t *snap, const char *filename)
{
	FILE *f = os_fopen(filename, "wb+");
	if (!f)
		return false;

	profiler_snapshot_dump(snap, dump_csv_fwrite, f);

	fclose(f);
	return true;
}

static void dump_csv_gzwrite(void *data, struct dstr *buffer)
{
	gzwrite(data, buffer->array, (unsigned)buffer->len);
}

bool profiler_snapshot_dump_csv_gz(const profiler_snapshot_t *snap, const char *filename)
{
	gzFile gz;
#ifdef _WIN32
	wchar_t *filename_w = NULL;

	os_utf8_to_wcs_ptr(filename, 0, &filename_w);
	if (!filename_w)
		return false;

	gz = gzopen_w(filename_w, "wb");
	bfree(filename_w);
#else
	gz = gzopen(filename, "wb");
#endif
	if (!gz)
		return false;

	profiler_snapshot_dump(snap, dump_csv_gzwrite, gz);

#ifdef _WIN32
	gzclose_w(gz);
#else
	gzclose(gz);
#endif
	return true;
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef VPIPE_RIGHT
#undef VPIPE
#undef DOWN_RIGHT
#undef G_MS
