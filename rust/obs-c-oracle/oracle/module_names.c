/* Test-only oracle: the original libobs/obs-module-names.c.h, unmodified,
 * with its six entry points renamed to oracle_* so they can link next to
 * the Rust shims. obs_module_paths_data_dir() is the oracle's own
 * (oracle/module_paths.c), so the core module lookup is the original C all
 * the way down. The rest keeps its real names: os_file_exists, the dstr
 * helpers, bmalloc and bfree come from their Rust ports (obs-util) under
 * cargo test, so the oracle looks at the same file system as the shims and
 * its strings are bfree'd like theirs. The callback is the caller's (the
 * test binary's). */

#define obs_module_names_file_name oracle_obs_module_names_file_name
#define obs_module_names_module_name oracle_obs_module_names_module_name
#define obs_module_names_config_path oracle_obs_module_names_config_path
#define obs_module_names_manifest_path oracle_obs_module_names_manifest_path
#define obs_module_names_listed oracle_obs_module_names_listed
#define obs_module_names_find_core oracle_obs_module_names_find_core
#define obs_module_paths_data_dir oracle_obs_module_paths_data_dir

#include "obs-module-names.c.h"
