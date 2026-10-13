#include "image-file-version.h"

#include <string.h>
#include <sys/stat.h>

#include <util/platform.h>

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
#elif !defined(_WIN32)
	version->mtime_nsec = (int64_t)stats.st_mtim.tv_nsec;
#endif
	version->size = (int64_t)stats.st_size;
}

bool image_file_version_equal(const struct image_file_version *a, const struct image_file_version *b)
{
	return a->exists == b->exists && a->mtime_sec == b->mtime_sec && a->mtime_nsec == b->mtime_nsec &&
	       a->size == b->size;
}
