#pragma once

#include <stdbool.h>
#include <stdint.h>
#include <time.h>
#include <wchar.h>

/* The text of a UTF-8 or UTF-16 (with BOM) file, without carriage returns,
 * as a bmalloc'd wide string; NULL if the file cannot be opened. */
wchar_t *ft2_read_text_file(const char *filename);

/* As ft2_read_text_file, but only the last log_lines lines. */
wchar_t *ft2_read_text_file_end(const char *filename, uint32_t log_lines);

/* The source's "Read from file" path, polled for changes once a second. */
struct ft2_text_file_watch {
	char *path; /* NULL when there is no file to poll */
	time_t timestamp;
	bool changed; /* the file changed at the last check: reread it at the next */
	uint64_t last_checked;
};

enum ft2_text_file_action {
	FT2_TEXT_FILE_MISSING,   /* no such file: show blank text */
	FT2_TEXT_FILE_UNCHANGED, /* the same file: nothing to do */
	FT2_TEXT_FILE_LOAD,      /* read the file now */
};

/* Point the watch at path (from the source's settings) at time now (ns).
 * force rereads the file even when the path is the same. */
enum ft2_text_file_action ft2_text_file_watch_set(struct ft2_text_file_watch *watch, const char *path, bool force,
						  uint64_t now);

/* Poll the file at time now (ns); true when the source should reread it. */
bool ft2_text_file_watch_tick(struct ft2_text_file_watch *watch, uint64_t now);

void ft2_text_file_watch_free(struct ft2_text_file_watch *watch);
