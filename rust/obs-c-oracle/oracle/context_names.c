/* Test-only oracle: the original libobs/obs-context-names.c.h, unmodified,
 * with its two functions renamed to oracle_* so they can link next to the
 * Rust shims. dstr_printf is C-only and lives renamed in oracle/dstr_libc.c.
 * The rest keeps its real names: bmalloc, brealloc, bfree and bmemdup (for
 * bstrdup) come from their Rust port (obs-util) under cargo test, so the
 * names the oracle returns are bfree'd like the shims'. The exists()
 * callback is the caller's (the test binary's). */
#include <stddef.h>
#include <stdint.h>

#define dstr_printf oracle_dstr_printf
#define obs_context_name_dup oracle_obs_context_name_dup
#define obs_context_name_deduplicate oracle_obs_context_name_deduplicate

#include "obs-context-names.c.h"
