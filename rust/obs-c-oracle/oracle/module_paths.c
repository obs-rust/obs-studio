/* Test-only oracle: the original libobs/obs-module-paths.c.h, unmodified,
 * with its three entry points renamed to oracle_* so they can link next to
 * the Rust shims. The rest keeps its real names: os_file_exists, os_glob and
 * os_globfree, the dstr helpers, astrcmpi_n, bmalloc and bfree come from
 * their Rust ports (obs-util) under cargo test, so the oracle searches the
 * same file system as the shims and its paths are bfree'd like theirs. The
 * extension hook and the callback are the caller's (the test binary's).
 *
 * oracle_obs_module_info2_layout() measures struct obs_module_info2 (obs.h)
 * for the layout test of the shims' #[repr(C)] copy. */
#include <stddef.h>
#include <stdint.h>

#define obs_module_paths_data_dir oracle_obs_module_paths_data_dir
#define obs_module_paths_find oracle_obs_module_paths_find
#define obs_module_paths_find_file oracle_obs_module_paths_find_file

#include "obs-module-paths.c.h"

size_t oracle_obs_module_info2_layout(int which);

/* 0: size, 1: alignment, 2..5: the offsets of bin_path, data_path, name and
 * type; 6: the size of enum obs_runtime_module_type. */
size_t oracle_obs_module_info2_layout(int which)
{
	switch (which) {
	case 0:
		return sizeof(struct obs_module_info2);
	case 1:
		return _Alignof(struct obs_module_info2);
	case 2:
		return offsetof(struct obs_module_info2, bin_path);
	case 3:
		return offsetof(struct obs_module_info2, data_path);
	case 4:
		return offsetof(struct obs_module_info2, name);
	case 5:
		return offsetof(struct obs_module_info2, type);
	case 6:
		return sizeof(enum obs_runtime_module_type);
	default:
		return SIZE_MAX;
	}
}
