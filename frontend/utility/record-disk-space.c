#include "record-disk-space.h"

#include <stddef.h>

bool record_disk_space_low(enum record_disk_check when, bool output_to_url, const char *path,
			   uint64_t (*free_space)(const char *path))
{
	/* A recording to a URL writes nothing to disk: neither check applies,
	 * before it starts or while it runs. */
	(void)when;
	if (output_to_url)
		return false;

	if (!path)
		return false;

	return free_space(path) < RECORD_MIN_FREE_BYTES;
}
