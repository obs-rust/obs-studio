#include "output-filename-reserve.h"

#include <errno.h>
#include <fcntl.h>
#include <string.h>
#include <sys/stat.h>

#ifdef _WIN32
#include <io.h>
#else
#include <unistd.h>
#endif

#include <util/bmem.h>
#include <util/dstr.h>
#include <util/platform.h>

enum create_result {
	CREATE_OK,
	CREATE_EXISTS,
	CREATE_FAILED,
};

/* Creates path as an empty file, failing if anything already has that name.
 * O_EXCL makes the check and the creation one step, also between processes. */
static enum create_result create_exclusive(const char *path)
{
#ifdef _WIN32
	wchar_t *wpath = NULL;
	if (!os_utf8_to_wcs_ptr(path, 0, &wpath) || !wpath)
		return CREATE_FAILED;
	int fd = _wopen(wpath, _O_WRONLY | _O_CREAT | _O_EXCL | _O_BINARY, _S_IREAD | _S_IWRITE);
	bfree(wpath);
	if (fd == -1)
		return errno == EEXIST ? CREATE_EXISTS : CREATE_FAILED;
	_close(fd);
#else
	int fd = open(path, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0666);
	if (fd == -1)
		return errno == EEXIST ? CREATE_EXISTS : CREATE_FAILED;
	close(fd);
#endif
	return CREATE_OK;
}

char *output_filename_reserve(const char *path, bool no_space, bool overwrite, bool *reserved)
{
	if (reserved)
		*reserved = false;
	if (!path)
		return NULL;
	if (overwrite)
		return bstrdup(path);

	enum create_result result = create_exclusive(path);
	if (result != CREATE_EXISTS) {
		if (reserved)
			*reserved = result == CREATE_OK;
		return bstrdup(path);
	}

	/* the same numbering FindBestFilename() used: "name (2).ext" or
	 * "name_2.ext" */
	const char *ext = strrchr(path, '.');
	if (!ext)
		return bstrdup(path);

	size_t ext_start = (size_t)(ext - path);
	struct dstr test = {0};
	for (int num = 2;; num++) {
		dstr_ncopy(&test, path, ext_start);
		dstr_catf(&test, no_space ? "_%d" : " (%d)", num);
		dstr_cat(&test, ext);

		result = create_exclusive(test.array);
		if (result != CREATE_EXISTS) {
			if (reserved)
				*reserved = result == CREATE_OK;
			return test.array;
		}
	}
}

void output_filename_release(const char *path)
{
	if (path && *path && os_get_file_size(path) == 0)
		os_unlink(path);
}

void output_filename_reservation_hold(struct output_filename_reservation *reservation, const char *path)
{
	output_filename_reservation_cancel(reservation);
	if (path && *path)
		reservation->path = bstrdup(path);
}

void output_filename_reservation_hand_over(struct output_filename_reservation *reservation)
{
	bfree(reservation->path);
	reservation->path = NULL;
}

void output_filename_reservation_cancel(struct output_filename_reservation *reservation)
{
	output_filename_release(reservation->path);
	bfree(reservation->path);
	reservation->path = NULL;
}
