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
	/* Sub-second part of the modification time, where the platform
	 * reports it (stat() on Linux and macOS, the file's last write time on
	 * Windows); 0 otherwise. */
	int64_t mtime_nsec;
	int64_t size;
};

void image_file_version_get(const char *path, struct image_file_version *version);
bool image_file_version_equal(const struct image_file_version *a, const struct image_file_version *b);

/* Fills @version from a Windows file time (the number of 100-nanosecond
 * intervals since 1601-01-01 UTC, as in WIN32_FILE_ATTRIBUTE_DATA's
 * ftLastWriteTime) and size. Plain arithmetic, so it is built and tested on
 * every platform; image_file_version_get() uses it on Windows. */
void image_file_version_from_filetime(struct image_file_version *version, uint64_t filetime, uint64_t size);

#ifdef __cplusplus
}
#endif
