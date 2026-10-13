#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* The low disk space check for recordings, kept free of Qt so it can be unit
 * tested. A recording to a file is not started, and a running one is stopped
 * (with the replay buffer), when less than RECORD_MIN_FREE_BYTES are left
 * where it is written. */

#define RECORD_MIN_FREE_BYTES (50ULL * 1024ULL * 1024ULL)

enum record_disk_check {
	/* before a recording or the replay buffer starts */
	RECORD_DISK_CHECK_START,
	/* every second while recording */
	RECORD_DISK_CHECK_RUNNING,
};

/* output_to_url: the Custom Output (FFmpeg) writes to a URL instead of a
 * file. path: the recording folder setting, or NULL. free_space: returns the
 * bytes free on the volume of a path (os_get_free_disk_space(), which returns
 * 0 for a path that does not exist). */
bool record_disk_space_low(enum record_disk_check when, bool output_to_url, const char *path,
			   uint64_t (*free_space)(const char *path));

#ifdef __cplusplus
}
#endif
