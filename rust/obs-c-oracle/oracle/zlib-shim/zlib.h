/* Test-only stand-in for <zlib.h>, for the util/profiler-io.c oracle
 * (oracle/profiler_io.c). The four calls it makes write through stdio
 * without compressing, so the oracle's "csv.gz" holds the plain CSV and the
 * Rust port's gzip output is compared with it after decompressing. The
 * Windows Rust test runners have no zlib. */

#pragma once

#include <stdio.h>
#include <wchar.h>

typedef FILE *gzFile;

static inline gzFile gzopen(const char *path, const char *mode)
{
	return fopen(path, mode);
}

static inline int gzwrite(gzFile file, const void *buf, unsigned len)
{
	return (int)fwrite(buf, 1, len, file);
}

static inline int gzclose(gzFile file)
{
	return fclose(file);
}

#if defined(_WIN32)
static inline gzFile gzopen_w(const wchar_t *path, const char *mode)
{
	(void)mode;
	return _wfopen(path, L"wb");
}

static inline int gzclose_w(gzFile file)
{
	return fclose(file);
}
#endif
