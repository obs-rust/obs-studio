/* Test-only oracle: the original libobs/obs-windows-symbol-paths.c.h,
 * unmodified, with its three functions renamed to oracle_* so they can link
 * next to the Rust shims. Its dstr calls are renamed to the C dstr oracle
 * (oracle/dstr.c, the same renames), so the rules run on the original
 * util/dstr.c, not on its Rust port; dstr_cat(), an inline of util/dstr.h,
 * reaches oracle_dstr_ncat() through them. It takes no Windows type, so it
 * builds on every host. */
#ifndef _WIN32
#define _GNU_SOURCE
#endif
#include <stddef.h>
#include <stdint.h>

#define obs_windows_symbol_paths_dir oracle_obs_windows_symbol_paths_dir
#define obs_windows_symbol_paths_listed oracle_obs_windows_symbol_paths_listed
#define obs_windows_symbol_paths_join oracle_obs_windows_symbol_paths_join

#define astrcmpi oracle_astrcmpi
#define astrcmp_n oracle_astrcmp_n
#define astrcmpi_n oracle_astrcmpi_n
#define astrstri oracle_astrstri
#define strdepad oracle_strdepad
#define strlist_split oracle_strlist_split
#define strlist_free oracle_strlist_free
#define dstr_init_copy_strref oracle_dstr_init_copy_strref
#define dstr_copy oracle_dstr_copy
#define dstr_copy_strref oracle_dstr_copy_strref
#define dstr_ncopy oracle_dstr_ncopy
#define dstr_ncopy_dstr oracle_dstr_ncopy_dstr
#define dstr_cat_dstr oracle_dstr_cat_dstr
#define dstr_cat_strref oracle_dstr_cat_strref
#define dstr_ncat oracle_dstr_ncat
#define dstr_ncat_dstr oracle_dstr_ncat_dstr
#define dstr_insert oracle_dstr_insert
#define dstr_insert_dstr oracle_dstr_insert_dstr
#define dstr_insert_ch oracle_dstr_insert_ch
#define dstr_remove oracle_dstr_remove
#define dstr_safe_printf oracle_dstr_safe_printf
#define dstr_replace oracle_dstr_replace
#define dstr_depad oracle_dstr_depad
#define dstr_left oracle_dstr_left
#define dstr_mid oracle_dstr_mid
#define dstr_right oracle_dstr_right

#include "obs-windows-symbol-paths.c.h"
