/* Test-only oracle: the original libobs/obs-module-source-rules.c.h,
 * unmodified, with its five entry points renamed to oracle_* so they can
 * link next to the Rust shims. dstr_printf is C-only and lives renamed in
 * oracle/dstr_libc.c, so the versioned id is printed by the original
 * util/dstr-libc.c (the C library's vsnprintf). bmalloc, brealloc, bfree and
 * bmemdup (for bstrdup) keep their real names: under cargo test they come
 * from their Rust port (obs-util), so the oracle's ids are bfree'd like the
 * shims'. */

#define dstr_printf oracle_dstr_printf
#define obs_module_source_rules_list_of oracle_obs_module_source_rules_list_of
#define obs_module_source_rules_size_supported oracle_obs_module_source_rules_size_supported
#define obs_module_source_rules_output_flags oracle_obs_module_source_rules_output_flags
#define obs_module_source_rules_check oracle_obs_module_source_rules_check
#define obs_module_source_rules_versioned_id oracle_obs_module_source_rules_versioned_id

#include "obs-module-source-rules.c.h"
