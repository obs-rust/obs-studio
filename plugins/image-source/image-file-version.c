#include "image-file-version.h"

#include <string.h>
#include <sys/stat.h>

#include <util/platform.h>

#ifndef _WIN32
/* Windows: image-file-version-win32.c (os_stat() has whole seconds only). */
void image_file_version_get(const char *path, struct image_file_version *version)
{
	struct stat stats;

	memset(version, 0, sizeof(*version));
	if (!path || os_stat(path, &stats) != 0)
		return;

	version->exists = true;
	version->mtime_sec = (int64_t)stats.st_mtime;
#if defined(__APPLE__)
	version->mtime_nsec = (int64_t)stats.st_mtimespec.tv_nsec;
#else
	version->mtime_nsec = (int64_t)stats.st_mtim.tv_nsec;
#endif
	version->size = (int64_t)stats.st_size;
}
#endif

bool image_file_version_equal(const struct image_file_version *a, const struct image_file_version *b)
{
	return a->exists == b->exists && a->mtime_sec == b->mtime_sec && a->mtime_nsec == b->mtime_nsec &&
	       a->size == b->size;
}

/* Seconds between 1601-01-01 and 1970-01-01. */
#define FILETIME_UNIX_EPOCH 11644473600LL

void image_file_version_from_filetime(struct image_file_version *version, uint64_t filetime, uint64_t size)
{
	memset(version, 0, sizeof(*version));
	version->exists = true;
	version->mtime_sec = (int64_t)(filetime / 10000000ULL) - FILETIME_UNIX_EPOCH;
	version->mtime_nsec = (int64_t)(filetime % 10000000ULL) * 100;
	version->size = (int64_t)size;
}
