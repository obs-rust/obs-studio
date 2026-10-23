/* Test-only oracle: the original libobs/obs-module-locale.c.h, unmodified,
 * with its three entry points renamed to oracle_* so they can link next to
 * the Rust shims. astrcmpi is renamed to the C dstr oracle (oracle/dstr.c,
 * the same rename), so the "same locale" decision runs on the original
 * util/dstr.c, not on its Rust port. dstr_copy, dstr_cat, bmalloc and bfree
 * keep their real names: under cargo test they come from their Rust ports
 * (obs-util), so the oracle's names are bfree'd like the shims'. */

#define obs_module_locale_default_file oracle_obs_module_locale_default_file
#define obs_module_locale_is_default oracle_obs_module_locale_is_default
#define obs_module_locale_file oracle_obs_module_locale_file

#define astrcmpi oracle_astrcmpi

#include "obs-module-locale.c.h"
