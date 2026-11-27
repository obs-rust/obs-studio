/* Original obs_source_inc_showing(), obs_source_inc_active(),
 * obs_source_dec_showing() and obs_source_dec_active(), renamed oracle_*.
 * The obs_source_activate() / obs_source_deactivate() both sides call are
 * obs-source's Rust exports (#5811), whose counter adapters report to
 * oracle/view.c for these test sources, so a C call and a Rust call give
 * comparable traces (call, source name, view type). */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_inc_showing oracle_obs_source_inc_showing
#define obs_source_inc_active oracle_obs_source_inc_active
#define obs_source_dec_showing oracle_obs_source_dec_showing
#define obs_source_dec_active oracle_obs_source_dec_active
#include "libobs/build/libobs.pch.h"
#include "source-show-active-counters.inc"
#undef obs_source_dec_active
#undef obs_source_dec_showing
#undef obs_source_inc_active
#undef obs_source_inc_showing

typedef void (*oracle_view_trace_fn)(const char *event, const char *name, long long value);
void oracle_view_set_trace(oracle_view_trace_fn hook);
void oracle_view_flush(void);

struct source_show_active_counters_record {
	unsigned log_count;
	int log_level;
	char log_message[256];
	unsigned trace_count;
	char trace[256];
};

static struct source_show_active_counters_record *recording;

static void source_show_active_counters_trace(const char *event, const char *name, long long value)
{
	if (!recording)
		return;
	recording->trace_count++;
	snprintf(recording->trace, sizeof(recording->trace), "%s %s %lld", event, name ? name : "-", value);
}

#if defined(__GNUC__) || defined(__clang__)
static void source_show_active_counters_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_show_active_counters_capture(int level, const char *format, va_list args, void *param)
{
	struct source_show_active_counters_record *record = param;
	record->log_count++;
	record->log_level = level;
	vsnprintf(record->log_message, sizeof(record->log_message), format, args);
}

typedef void (*source_show_active_counters_call)(obs_source_t *);
typedef void (*source_show_active_counters_get_log)(log_handler_t *, void **);
typedef void (*source_show_active_counters_set_log)(log_handler_t, void *);

/* Runs `call` (the Rust export, or a mutant of it) and the original C
 * function `which` (0 inc_showing, 1 inc_active, 2 dec_showing,
 * 3 dec_active) on the same source, an oracle/view.c test source or NULL.
 * Both must log alike (once at the function's own name for NULL, never
 * otherwise) and make the same activate/deactivate call (one for a source,
 * none for NULL). What both sides shared goes to observed_out: the trace
 * ("activate <name> <view type>") for a source, the log line for NULL. */
bool oracle_source_show_active_counters_check(source_show_active_counters_call call, int which,
					      source_show_active_counters_get_log get_log,
					      source_show_active_counters_set_log set_log, obs_source_t *source,
					      char *observed_out, size_t observed_size);
bool oracle_source_show_active_counters_check(source_show_active_counters_call call, int which,
					      source_show_active_counters_get_log get_log,
					      source_show_active_counters_set_log set_log, obs_source_t *source,
					      char *observed_out, size_t observed_size)
{
	static const source_show_active_counters_call originals[4] = {
		oracle_obs_source_inc_showing,
		oracle_obs_source_inc_active,
		oracle_obs_source_dec_showing,
		oracle_obs_source_dec_active,
	};
	if (which < 0 || which > 3)
		return false;
	source_show_active_counters_call calls[2] = {call, originals[which]};
	struct source_show_active_counters_record records[2];
	log_handler_t previous;
	void *param;
	get_log(&previous, &param);
	oracle_view_set_trace(source_show_active_counters_trace);
	for (size_t side = 0; side < 2; side++) {
		memset(&records[side], 0, sizeof(records[side]));
		records[side].log_level = -1;
		recording = &records[side];
		set_log(source_show_active_counters_capture, &records[side]);
		calls[side](source);
		oracle_view_flush();
		set_log(previous, param);
		recording = NULL;
	}
	oracle_view_set_trace(NULL);

	unsigned want_logs = source ? 0u : 1u;
	unsigned want_traces = source ? 1u : 0u;
	bool equal = records[0].log_count == records[1].log_count && records[1].log_count == want_logs &&
		     records[0].log_level == records[1].log_level &&
		     strcmp(records[0].log_message, records[1].log_message) == 0 &&
		     records[0].trace_count == records[1].trace_count && records[1].trace_count == want_traces &&
		     strcmp(records[0].trace, records[1].trace) == 0;
	snprintf(observed_out, observed_size, "%s", source ? records[1].trace : records[1].log_message);
	return equal;
}
