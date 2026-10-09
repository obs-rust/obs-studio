/* Symbols file-serializer.c needs from platform.c / bmem.c, without pulling
 * obs.h in. On Unix these are the same calls platform.c and platform-nix.c
 * make. On Windows they follow platform.c (wide fopen) and platform-windows.c
 * (DeleteFileW / MoveFileExW). */
#define _FILE_OFFSET_BITS 64
#ifndef _WIN32
/* fseeko/ftello are POSIX; -std=c11 hides them without this. */
#define _POSIX_C_SOURCE 200809L
#endif

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

/* bmemdup comes from the Rust bmem port (obs-util). */

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

static wchar_t *utf8_to_wide(const char *path)
{
	int n;
	wchar_t *wide;

	if (!path)
		return NULL;
	n = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, NULL, 0);
	if (n <= 0)
		return NULL;
	wide = (wchar_t *)malloc((size_t)n * sizeof(wchar_t));
	if (!wide)
		return NULL;
	if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, -1, wide, n) != n) {
		free(wide);
		return NULL;
	}
	return wide;
}

FILE *os_fopen(const char *path, const char *mode)
{
	wchar_t *wpath;
	wchar_t *wmode;
	FILE *file;

	if (!path)
		return NULL;
	wpath = utf8_to_wide(path);
	wmode = utf8_to_wide(mode);
	file = (wpath && wmode) ? _wfopen(wpath, wmode) : NULL;
	free(wpath);
	free(wmode);
	return file;
}

int os_fseeki64(FILE *file, int64_t offset, int origin)
{
	return _fseeki64(file, offset, origin);
}

int64_t os_ftelli64(FILE *file)
{
	return _ftelli64(file);
}

int os_unlink(const char *path)
{
	wchar_t *wide = utf8_to_wide(path);
	int code;

	if (!wide)
		return -1;
	code = DeleteFileW(wide) ? 0 : -1;
	free(wide);
	return code;
}

int os_rename(const char *old_path, const char *new_path)
{
	wchar_t *old_wide = utf8_to_wide(old_path);
	wchar_t *new_wide = utf8_to_wide(new_path);
	int code = -1;

	if (old_wide && new_wide)
		code = MoveFileExW(old_wide, new_wide, MOVEFILE_REPLACE_EXISTING) ? 0 : -1;
	free(old_wide);
	free(new_wide);
	return code;
}

int os_safe_replace(const char *target, const char *from, const char *backup)
{
	wchar_t *wtarget = target ? utf8_to_wide(target) : NULL;
	wchar_t *wfrom = from ? utf8_to_wide(from) : NULL;
	wchar_t *wbackup = backup ? utf8_to_wide(backup) : NULL;
	int code = -1;

	if (wtarget && wfrom && (!backup || wbackup)) {
		if (ReplaceFileW(wtarget, wfrom, wbackup, 0, NULL, NULL))
			code = 0;
		else if (GetLastError() == ERROR_FILE_NOT_FOUND)
			code = MoveFileExW(wfrom, wtarget, MOVEFILE_REPLACE_EXISTING) ? 0 : -1;
	}
	free(wtarget);
	free(wfrom);
	free(wbackup);
	return code;
}
#else
#include <unistd.h>

FILE *os_fopen(const char *path, const char *mode)
{
	return path ? fopen(path, mode) : NULL;
}

int os_fseeki64(FILE *file, int64_t offset, int origin)
{
	return fseeko(file, offset, origin);
}

int64_t os_ftelli64(FILE *file)
{
	return ftello(file);
}

int os_unlink(const char *path)
{
	return unlink(path);
}

int os_rename(const char *old_path, const char *new_path)
{
	return rename(old_path, new_path);
}

int os_safe_replace(const char *target, const char *from, const char *backup)
{
	if (backup && access(target, F_OK) == 0 && rename(target, backup) != 0)
		return -1;
	return rename(from, target);
}
#endif
