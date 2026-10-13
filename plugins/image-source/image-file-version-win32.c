#include "image-file-version.h"

#include <string.h>
#include <sys/stat.h>

#include <windows.h>

#include <util/bmem.h>
#include <util/platform.h>

/* os_stat() gives whole seconds on Windows; the last write time has 100 ns
 * ticks, so an image replaced within the same second is seen too. */
void image_file_version_get(const char *path, struct image_file_version *version)
{
	WIN32_FILE_ATTRIBUTE_DATA data;
	wchar_t *wpath = NULL;
	BOOL ok;

	memset(version, 0, sizeof(*version));
	if (!path || !os_utf8_to_wcs_ptr(path, 0, &wpath))
		return;

	ok = GetFileAttributesExW(wpath, GetFileExInfoStandard, &data);
	bfree(wpath);
	if (!ok)
		return;

	uint64_t write_time = ((uint64_t)data.ftLastWriteTime.dwHighDateTime << 32) |
			      data.ftLastWriteTime.dwLowDateTime;
	uint64_t size = ((uint64_t)data.nFileSizeHigh << 32) | data.nFileSizeLow;
	image_file_version_from_filetime(version, write_time, size);
}
