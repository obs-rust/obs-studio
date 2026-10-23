/* Test-only oracle: the libobs symbols the oracle needs that it cannot compile
 * from the real source. Each one is tagged with its libobs source file in the
 * tag form the drift check on #145 looks for, so a copy cannot silently fall
 * behind the original.
 *
 * The names are oracle_* so an oracle call does not resolve to the Rust port's
 * plain os_breakpoint / os_oom symbols in a test binary, which is the #145
 * failure mode.
 *
 * Deliberately includes no libobs header: platform.h marks these EXPORT. */
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>

/* The two entry points libobs/util/bmem.c calls on the 0-size and
 * failed-allocation paths (bmem.c:104, :111, :125, :132).
 *
 * Intentionally different from the original: libobs raises SIGTRAP there
 * (libobs/util/platform-nix.c:793 for os_breakpoint and :798 for os_oom,
 * libobs/util/platform-windows.c:1192 and :1197 for the Windows halves). A
 * SIGTRAP inside cargo test aborts the harness and confuses debuggers, and an
 * empty body - what the deleted oracle/test_stubs.c had - lets a parity test pass
 * against an oracle libobs would have trapped in. So they abort loudly instead:
 * any test that reaches either path dies visibly rather than silently agreeing
 * with a difference. rust/obs-util/tests/platform_oracle.rs checks that with a
 * fork.
 *
 * Note that a check matching definition names against libobs cannot see these:
 * the oracle_ prefix keeps them out of the port's symbol namespace.
 */
/* platform-nix.c, platform-windows.c */
void oracle_os_breakpoint(void);
void oracle_os_oom(void);

/* stderr is block-buffered when it is a file, and abort() does not flush stdio,
 * so the marker is flushed explicitly before the abort. */
void oracle_os_breakpoint(void)
{
	fprintf(stderr, "oracle: os_breakpoint: libobs would raise SIGTRAP "
			"(libobs/util/platform-nix.c:793); aborting\n");
	fflush(stderr);
	abort();
}

/* platform-nix.c, platform-windows.c */
void oracle_os_oom(void)
{
	fprintf(stderr, "oracle: os_oom: libobs would raise SIGTRAP "
			"(libobs/util/platform-nix.c:798); aborting\n");
	fflush(stderr);
	abort();
}

/* The MinGW link-check target excludes platform-windows.c because its libobs
 * implementation uses MSVC-only constructs. Supply the five oracle_ symbols
 * that file would otherwise provide. This branch is link-only; these stubs are
 * not executed by the Windows test suite.
 */
#if defined(_WIN32) && !defined(_MSC_VER)
/* platform-windows.c */
bool oracle_os_file_exists(const char *path);

/* platform-windows.c */
int oracle_os_mkdir(const char *path);

/* platform-windows.c */
int oracle_os_rename(const char *old_path, const char *new_path);

/* platform-windows.c */
int oracle_os_safe_replace(const char *target, const char *from, const char *backup);

/* platform-windows.c */
uint64_t oracle_os_gettime_ns(void);

bool oracle_os_file_exists(const char *path)
{
	(void)path;
	return false;
}

int oracle_os_mkdir(const char *path)
{
	(void)path;
	return -1;
}

int oracle_os_rename(const char *old_path, const char *new_path)
{
	(void)old_path;
	(void)new_path;
	return -1;
}

int oracle_os_safe_replace(const char *target, const char *from, const char *backup)
{
	(void)target;
	(void)from;
	(void)backup;
	return -1;
}

uint64_t oracle_os_gettime_ns(void)
{
	return 0;
}
#endif

/* The clock fallback for targets that need a plain name at link time.
 *
 * This intentionally returns 0; tests use oracle_profiler_gettime_ns instead.
 * Keep it weak where supported so a Rust platform shim wins when linked.
 */
/* platform-cocoa.m, platform-windows.c */
#if defined(__APPLE__) || defined(_WIN32)
uint64_t os_gettime_ns(void);
#if defined(_MSC_VER)
uint64_t os_gettime_ns_stub(void);
uint64_t os_gettime_ns_stub(void)
{
	return 0;
}
#pragma comment(linker, "/alternatename:os_gettime_ns=os_gettime_ns_stub")
#else
__attribute__((weak)) uint64_t os_gettime_ns(void)
{
	return 0;
}
#endif
#endif
