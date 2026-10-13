#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* What the Image source compares, once a second, to see whether its file
 * changed on disk and has to be loaded again. */
struct image_file_version {
	bool exists;
	int64_t mtime_sec;
	/* Sub-second part of the modification time, where the platform's
	 * stat() reports it; 0 otherwise. */
	int64_t mtime_nsec;
	int64_t size;
};

void image_file_version_get(const char *path, struct image_file_version *version);
bool image_file_version_equal(const struct image_file_version *a, const struct image_file_version *b);

#ifdef __cplusplus
}
#endif
