/* The Rust output callback setters given a NULL output (#4754). The C
 * originals dereference it, so this probes only the Rust shims: each one
 * logs obs_output_valid's debug line once and returns. */
#include <stdarg.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include "util/base.h"

struct output_callback_null_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_callback_null_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void output_callback_null_capture(int level, const char *format, va_list args, void *param)
{
	struct output_callback_null_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

static bool output_callback_null_reconnect(void *data, void *output, int code)
{
	(void)data;
	(void)output;
	(void)code;
	return true;
}
static void output_callback_null_packet(void *output, void *pkt, void *pkt_time, void *param)
{
	(void)output;
	(void)pkt;
	(void)pkt_time;
	(void)param;
}

typedef bool (*output_callback_null_reconnect_fn)(void *, void *, int);
typedef void (*output_callback_null_packet_fn)(void *, void *, void *, void *);
typedef void (*output_callback_null_set_reconnect)(void *, output_callback_null_reconnect_fn, void *);
typedef void (*output_callback_null_set_packet)(void *, output_callback_null_packet_fn, void *);
typedef void (*output_callback_null_get_log)(log_handler_t *, void **);
typedef void (*output_callback_null_set_log)(log_handler_t, void *);

static bool output_callback_null_one(struct output_callback_null_log *log, const char *expected)
{
	char line[256];
	snprintf(line, sizeof(line), "%s: Null 'output' parameter", expected);
	return log->count == 1 && log->level == LOG_DEBUG && strcmp(log->message, line) == 0;
}

/* Calls each setter with a NULL output, with and without a callback and
 * data, and checks each call wrote exactly the one debug line. */
bool oracle_output_callback_null_logs(output_callback_null_set_reconnect set_reconnect,
				      output_callback_null_set_packet add, output_callback_null_set_packet remove,
				      output_callback_null_get_log get_log, output_callback_null_set_log set_log);
bool oracle_output_callback_null_logs(output_callback_null_set_reconnect set_reconnect,
				      output_callback_null_set_packet add, output_callback_null_set_packet remove,
				      output_callback_null_get_log get_log, output_callback_null_set_log set_log)
{
	log_handler_t old_handler;
	void *old_param;
	int token = 0;
	bool ok = true;
	get_log(&old_handler, &old_param);
	for (int i = 0; i < 2; i++) {
		void *data = i ? &token : NULL;
		struct output_callback_null_log log;

		memset(&log, 0, sizeof(log));
		set_log(output_callback_null_capture, &log);
		set_reconnect(NULL, output_callback_null_reconnect, data);
		ok = ok && output_callback_null_one(&log, "obs_output_set_reconnect_callback");

		memset(&log, 0, sizeof(log));
		set_log(output_callback_null_capture, &log);
		set_reconnect(NULL, NULL, data);
		ok = ok && output_callback_null_one(&log, "obs_output_set_reconnect_callback");

		memset(&log, 0, sizeof(log));
		set_log(output_callback_null_capture, &log);
		add(NULL, i ? output_callback_null_packet : NULL, data);
		ok = ok && output_callback_null_one(&log, "obs_output_add_packet_callback");

		memset(&log, 0, sizeof(log));
		set_log(output_callback_null_capture, &log);
		remove(NULL, i ? output_callback_null_packet : NULL, data);
		ok = ok && output_callback_null_one(&log, "obs_output_remove_packet_callback");
	}
	set_log(old_handler, old_param);
	return ok;
}
