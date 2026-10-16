#pragma once

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Chooses the file a new recording (or screenshot, or stream dump) writes
 * to, and reserves it.
 *
 * path is the full path the filename format produced. With overwrite set,
 * path is returned unchanged and nothing is created. Otherwise the first of
 * path, "name (2).ext", "name (3).ext", ... ("name_2.ext", ... with no_space)
 * that does not exist yet is created as an empty file, atomically
 * (O_CREAT | O_EXCL), so two outputs that start in the same second, in one
 * OBS or in two, never get the same file. The output opens the file later
 * and truncates it.
 *
 * If the file cannot be created for any reason other than the name being
 * taken (a missing directory, no permission), the name is returned without
 * a reservation and the output reports the error when it opens the file, as
 * before. A path without an extension that already exists is returned as is.
 *
 * *reserved (may be NULL) tells whether this call created the file. Returns
 * a string the caller frees with bfree(), or NULL if path is NULL. */
char *output_filename_reserve(const char *path, bool no_space, bool overwrite, bool *reserved);

/* Removes a file reserved by output_filename_reserve() if it is still
 * empty, for an output that failed before writing anything. A file the
 * output has written to is kept. */
void output_filename_release(const char *path);

/* The owner of a file reserved for an output that starts later than its
 * name is chosen: the Enhanced Broadcasting stream dump reserves its file
 * when the go-live starts and only opens it once the stream output has
 * started, and the go-live can fail in between. Zero-initialise it. */
struct output_filename_reservation {
	char *path;
};

/* Takes over path, a file output_filename_reserve() created (NULL or ""
 * when it created none). A reservation still held from an earlier attempt
 * is cancelled first. */
void output_filename_reservation_hold(struct output_filename_reservation *reservation, const char *path);

/* The output has started and owns the file now: it is kept. */
void output_filename_reservation_hand_over(struct output_filename_reservation *reservation);

/* The output never started: the file is removed if it is still empty
 * (output_filename_release()). */
void output_filename_reservation_cancel(struct output_filename_reservation *reservation);

#ifdef __cplusplus
}
#endif
