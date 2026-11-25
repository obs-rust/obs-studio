/* Tier 3: the unchanged extracted obs_source_replace_missing_file() against
 * the Rust export, over the real private layout. The Rust side reads
 * context.data through obs_source_missing_files_getter_fields(), defined once
 * in source_missing_files_getter.c. */
#define _NO_CRT_STDIO_INLINE
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_source_replace_missing_file oracle_obs_source_replace_missing_file
#include "libobs/build/libobs.pch.h"
#include "source-replace-missing-file.inc"
#undef obs_source_replace_missing_file

struct source_replace_missing_file_state {
	unsigned calls;
	void *src;
	const char *path;
	void *data;
};

static struct source_replace_missing_file_state source_replace_missing_file_seen;

static void source_replace_missing_file_callback(void *src, const char *new_path, void *data)
{
	source_replace_missing_file_seen.calls++;
	source_replace_missing_file_seen.src = src;
	source_replace_missing_file_seen.path = new_path;
	source_replace_missing_file_seen.data = data;
}

struct source_replace_missing_file_log {
	unsigned count;
	int level;
	char message[256];
};
#if defined(__GNUC__) || defined(__clang__)
static void source_replace_missing_file_capture(int level, const char *format, va_list args, void *param)
	__attribute__((format(printf, 2, 0)));
#endif
static void source_replace_missing_file_capture(int level, const char *format, va_list args, void *param)
{
	struct source_replace_missing_file_log *log = param;
	log->count++;
	log->level = level;
	vsnprintf(log->message, sizeof(log->message), format, args);
}

typedef void (*source_replace_missing_file_proc)(obs_missing_file_cb, obs_source_t *, const char *, void *);
typedef void (*source_replace_missing_file_get_log)(log_handler_t *, void **);
typedef void (*source_replace_missing_file_set_log)(log_handler_t, void *);

/* flags: 1 source present, 2 private data, 4 path, 8 data argument. Each
 * side runs twice on its own source filled with the noise byte; the trace is
 * the calls, the pointers each saw, the logs and an unchanged source. */
bool oracle_source_replace_missing_file_check(source_replace_missing_file_proc proc,
					      source_replace_missing_file_get_log get_log,
					      source_replace_missing_file_set_log set_log, unsigned flags,
					      uint32_t noise);
bool oracle_source_replace_missing_file_check(source_replace_missing_file_proc proc,
					      source_replace_missing_file_get_log get_log,
					      source_replace_missing_file_set_log set_log, unsigned flags,
					      uint32_t noise)
{
	log_handler_t previous;
	void *param;
	get_log(&previous, &param);
	bool present = (flags & 1) != 0, has_data = (flags & 2) != 0;
	int private_data = 0, marker = 0;
	const char *path = (flags & 4) ? "fixed.png" : NULL;
	void *data = (flags & 8) ? &marker : NULL;
	bool dispatch = present && has_data;
	source_replace_missing_file_proc procs[] = {proc, oracle_obs_source_replace_missing_file};
	struct source_replace_missing_file_log logs[2] = {{0}};
	unsigned calls[2] = {0};
	bool equal = true;
	for (size_t p = 0; p < 2; p++) {
		struct obs_source *source = malloc(sizeof(*source));
		struct obs_source *before = malloc(sizeof(*before));
		if (!source || !before) {
			free(source);
			free(before);
			set_log(previous, param);
			return false;
		}
		memset(source, (int)(noise & 0xff), sizeof(*source));
		source->context.data = has_data ? &private_data : NULL;
		memcpy(before, source, sizeof(*source));
		set_log(source_replace_missing_file_capture, &logs[p]);
		for (int i = 0; i < 2; i++) {
			memset(&source_replace_missing_file_seen, 0, sizeof(source_replace_missing_file_seen));
			source_replace_missing_file_seen.path = "unset";
			procs[p](source_replace_missing_file_callback, present ? source : NULL, path, data);
			calls[p] += source_replace_missing_file_seen.calls;
			if (dispatch) {
				equal = equal && source_replace_missing_file_seen.calls == 1 &&
					source_replace_missing_file_seen.src == &private_data &&
					source_replace_missing_file_seen.path == path &&
					source_replace_missing_file_seen.data == data;
			} else {
				equal = equal && source_replace_missing_file_seen.calls == 0;
			}
		}
		set_log(previous, param);
		equal = equal && memcmp(before, source, sizeof(*source)) == 0;
		free(before);
		free(source);
	}
	equal = equal && calls[0] == calls[1] && logs[0].count == logs[1].count && logs[0].level == logs[1].level &&
		strcmp(logs[0].message, logs[1].message) == 0;
	equal = equal && logs[0].count == (present ? 0u : 2u);
	if (!present)
		equal = equal && logs[0].level == LOG_DEBUG &&
			strcmp(logs[0].message, "obs_source_replace_missing_file: Null 'source' parameter") == 0;
	return equal;
}
