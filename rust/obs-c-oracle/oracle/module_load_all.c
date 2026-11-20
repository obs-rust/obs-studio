/* Original deprecated module-loading no-ops, renamed beside the Rust exports.
 * Tier 2 additionally runs both through the complete shared libraries. */
#include "libobs/build/libobs.pch.h"

#if defined(_MSC_VER)
#pragma warning(disable : 4996)
#else
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif

void oracle_obs_load_all_modules(void);
void oracle_obs_load_all_modules2(struct obs_module_failure_info *mfi);
#define obs_load_all_modules oracle_obs_load_all_modules
#define obs_load_all_modules2 oracle_obs_load_all_modules2
#include "libobs/obs-module-load-all.c.h"
#undef obs_load_all_modules2
#undef obs_load_all_modules
