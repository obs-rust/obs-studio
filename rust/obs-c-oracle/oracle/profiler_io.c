/* Test-only oracle: the original libobs/util/profiler-io.c, unmodified, with
 * its global symbols renamed to oracle_* so it links next to the Rust port.
 * <zlib.h> is the pass-through stand-in in oracle/zlib-shim, so
 * oracle_profiler_snapshot_dump_csv_gz writes the CSV uncompressed.
 *
 * A NULL snapshot is taken from (and freed by) the oracle profiler in
 * oracle/profiler.c. The dstr printf functions are the oracle's
 * (oracle/dstr_libc.c); the rest of dstr, bmem, os_fopen and blog are the
 * ones the C calls in libobs. */

#include <stddef.h>

#define profile_print_func oracle_profile_print_func
#define profiler_print oracle_profiler_print
#define profiler_print_time_between_calls oracle_profiler_print_time_between_calls
#define profiler_snapshot_dump_csv oracle_profiler_snapshot_dump_csv
#define profiler_snapshot_dump_csv_gz oracle_profiler_snapshot_dump_csv_gz
#define profile_snapshot_create oracle_profile_snapshot_create
#define profile_snapshot_free oracle_profile_snapshot_free
#define dstr_printf oracle_dstr_printf
#define dstr_catf oracle_dstr_catf

/* libobs builds this file without the oracle's strict warnings: the C
 * passes struct pointers to %p, sign-extends an int mask into active, and
 * declares profile_print_func in no header. That is the C file, left as
 * written. Clang reports the %p calls as -Wformat-pedantic, which GCC does
 * not accept as a pragma. */
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wsign-conversion"
#pragma GCC diagnostic ignored "-Wformat"
#pragma GCC diagnostic ignored "-Wmissing-prototypes"
#if defined(__clang__)
#pragma GCC diagnostic ignored "-Wformat-pedantic"
#endif
#endif

#include "util/profiler-io.c.h"

/* Test-only: a base_set_log_handler handler that appends each line, then
 * '\n', to a caller-owned buffer, so Tier 3 can compare every line the Rust
 * shim and this file log. Only lines logged on the thread that called
 * oracle_profiler_io_log_own(1) are kept; tests running in parallel log
 * through the same global handler. A line that does not fit sets
 * `overflow`. */
#include <stdarg.h>

#include "util/threading.h"

static THREAD_LOCAL int profiler_io_log_owner;

void oracle_profiler_io_log_own(int own);

void oracle_profiler_io_log_own(int own)
{
	profiler_io_log_owner = own;
}

struct oracle_profiler_io_log {
	char *data;
	size_t cap;
	size_t len;
	int levels_not_info;
	int overflow;
};

void oracle_profiler_io_log_capture(int level, const char *format, va_list args, void *param);

void oracle_profiler_io_log_capture(int level, const char *format, va_list args, void *param)
{
	struct oracle_profiler_io_log *log = param;
	if (!profiler_io_log_owner)
		return;
	if (level != LOG_INFO)
		log->levels_not_info++;

	size_t room = log->cap - log->len;
	int n = vsnprintf(log->data + log->len, room, format, args);
	if (n < 0 || (size_t)n + 1 >= room) {
		log->overflow = 1;
		return;
	}
	log->len += (size_t)n;
	log->data[log->len++] = '\n';
}
