/* Test-only oracle: the original libobs/util/platform-nix.c, unmodified,
 * with its global symbols renamed to oracle_* so it can link next to the
 * Rust implementation. Named *_posix.c so Windows does not compile it.
 *
 * -std=c11 hides CLOCK_MONOTONIC and the pthread types this file pulls in
 * through platform.h. Define the feature macro before any header.
 */
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include <stddef.h>

#define os_dlopen oracle_os_dlopen
#define os_dlsym oracle_os_dlsym
#define os_dlclose oracle_os_dlclose
#define get_plugin_info oracle_get_plugin_info
#define os_is_obs_plugin oracle_os_is_obs_plugin
#define os_sleepto_ns oracle_os_sleepto_ns
#define os_sleepto_ns_fast oracle_os_sleepto_ns_fast
#define os_sleep_ms oracle_os_sleep_ms
#define os_file_exists oracle_os_file_exists
#define os_get_abs_path oracle_os_get_abs_path
#define os_get_abs_path_ptr oracle_os_get_abs_path_ptr
#define os_opendir oracle_os_opendir
#define os_readdir oracle_os_readdir
#define os_closedir oracle_os_closedir
#define os_glob oracle_os_glob
#define os_globfree oracle_os_globfree
#define os_unlink oracle_os_unlink
#define os_rmdir oracle_os_rmdir
#define os_mkdir oracle_os_mkdir
#define os_rename oracle_os_rename
#define os_safe_replace oracle_os_safe_replace
#define os_copyfile oracle_os_copyfile
#define os_getcwd oracle_os_getcwd
#define os_chdir oracle_os_chdir
#define os_breakpoint oracle_os_breakpoint
#define os_oom oracle_os_oom
#define os_generate_uuid oracle_os_generate_uuid
#define os_cpu_usage_info_start oracle_os_cpu_usage_info_start
#define os_cpu_usage_info_query oracle_os_cpu_usage_info_query
#define os_cpu_usage_info_destroy oracle_os_cpu_usage_info_destroy
#define os_gettime_ns oracle_os_gettime_ns
#define os_get_config_path oracle_os_get_config_path
#define os_get_config_path_ptr oracle_os_get_config_path_ptr
#define os_get_program_data_path oracle_os_get_program_data_path
#define os_get_program_data_path_ptr oracle_os_get_program_data_path_ptr
#define os_get_executable_path_ptr oracle_os_get_executable_path_ptr
#define os_get_emulation_status oracle_os_get_emulation_status
#define os_get_free_space oracle_os_get_free_space
#define os_request_high_performance oracle_os_request_high_performance
#define os_end_high_performance oracle_os_end_high_performance
#define os_get_physical_cores oracle_os_get_physical_cores
#define os_get_logical_cores oracle_os_get_logical_cores
#define os_get_sys_free_size oracle_os_get_sys_free_size
#define os_get_proc_memory_usage oracle_os_get_proc_memory_usage
#define os_get_proc_resident_size oracle_os_get_proc_resident_size
#define os_get_proc_virtual_size oracle_os_get_proc_virtual_size
#define os_get_sys_total_size oracle_os_get_sys_total_size
#define os_get_free_disk_space oracle_os_get_free_disk_space
#define os_openbsd_get_executable_path oracle_os_openbsd_get_executable_path
/* dstr_catf stays in the dstr oracle under the renamed symbol. The other
 * dstr calls resolve to the Rust shims. */
#define dstr_catf oracle_dstr_catf

/* sleepto calls os_gettime_ns. On macOS that function lives in
 * platform-cocoa.m, so this file has to provide the renamed symbol.
 * The name is written out in full so the macro above does not rewrite it.
 */
#ifdef __APPLE__
#include <stdint.h>
#include <time.h>
/* The header redeclares this with visibility("default") after the
 * rename macro. Put the attribute on the definition so that redeclaration
 * does not warn.
 */
__attribute__((visibility("default"))) uint64_t oracle_os_gettime_ns(void)
{
	return clock_gettime_nsec_np(CLOCK_UPTIME_RAW);
}
#endif

#include "util/platform-nix.c"

size_t oracle_os_dirent_size(void)
{
	return sizeof(struct os_dirent);
}
size_t oracle_os_dirent_align(void)
{
	return _Alignof(struct os_dirent);
}
size_t oracle_os_dirent_offset_d_name(void)
{
	return offsetof(struct os_dirent, d_name);
}
size_t oracle_os_dirent_offset_directory(void)
{
	return offsetof(struct os_dirent, directory);
}
size_t oracle_os_globent_size(void)
{
	return sizeof(struct os_globent);
}
size_t oracle_os_globent_align(void)
{
	return _Alignof(struct os_globent);
}
size_t oracle_os_globent_offset_path(void)
{
	return offsetof(struct os_globent, path);
}
size_t oracle_os_globent_offset_directory(void)
{
	return offsetof(struct os_globent, directory);
}
size_t oracle_os_glob_info_size(void)
{
	return sizeof(struct os_glob_info);
}
size_t oracle_os_glob_info_align(void)
{
	return _Alignof(struct os_glob_info);
}
size_t oracle_os_glob_info_offset_gl_pathc(void)
{
	return offsetof(struct os_glob_info, gl_pathc);
}
size_t oracle_os_glob_info_offset_gl_pathv(void)
{
	return offsetof(struct os_glob_info, gl_pathv);
}
size_t oracle_os_proc_memory_usage_size(void)
{
	return sizeof(struct os_proc_memory_usage);
}
size_t oracle_os_proc_memory_usage_align(void)
{
	return _Alignof(struct os_proc_memory_usage);
}
size_t oracle_os_proc_memory_usage_offset_resident_size(void)
{
	return offsetof(struct os_proc_memory_usage, resident_size);
}
size_t oracle_os_proc_memory_usage_offset_virtual_size(void)
{
	return offsetof(struct os_proc_memory_usage, virtual_size);
}
