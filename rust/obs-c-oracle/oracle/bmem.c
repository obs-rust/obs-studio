/* Test-only oracle: the original libobs/util/bmem.c, unmodified, with its
 * global symbols renamed to oracle_* so it can link next to the Rust
 * implementation. os_breakpoint/os_oom resolve to oracle/test_stubs.c,
 * bcrash to util/base-variadic.c. */
/* bmem.c pulls in util/threading.h, whose pthread_mutex_init_recursive needs
 * the POSIX/XSI extensions that a strict -std=c11 build hides on glibc. */
#if !defined(_WIN32) && !defined(_GNU_SOURCE)
#define _GNU_SOURCE
#endif

#define bmalloc oracle_bmalloc
#define brealloc oracle_brealloc
#define bfree oracle_bfree
#define bnum_allocs oracle_bnum_allocs
#define base_get_alignment oracle_base_get_alignment
#define bmemdup oracle_bmemdup

#include "util/bmem.c"
