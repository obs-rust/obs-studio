/* Test-only oracle: the original libobs/util/platform-windows.c, unmodified,
 * with its global symbols renamed to oracle_* so it can link next to the
 * Rust implementation. Windows-only.
 *
 * UNICODE makes FindNextFile the wide entry, as the libobs Windows build
 * does (cmake/windows/compilerconfig.cmake). The string conversions it calls
 * are the oracle_ ones that oracle/platform.c compiles from
 * platform-conversions.c, since the plain names are the Rust port's. dstr and
 * bmalloc resolve to the Rust shims. blog and bcrash stay in base-variadic.c.
 *
 * The oracle build defines PTW32_STATIC_LIB, so this copy's DllMain also
 * runs the pthread static-lib hooks. libobs does not define that macro.
 * The Rust DllMain matches libobs (timer period only) and is not compared
 * to this one.
 */
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif

#include <stddef.h>
#include <stdint.h>

#ifdef _MSC_VER
/* get_win_ver casts GetProcAddress to a function pointer. That is C4152,
 * and the oracle builds with warnings as errors. The C file is unchanged. */
#pragma warning(disable : 4152)
#endif

#define os_dlopen oracle_os_dlopen
#define os_dlsym oracle_os_dlsym
#define os_dlclose oracle_os_dlclose
#define get_plugin_info oracle_get_plugin_info
#define os_is_obs_plugin oracle_os_is_obs_plugin
#define os_cpu_usage_info_start oracle_os_cpu_usage_info_start
#define os_cpu_usage_info_query oracle_os_cpu_usage_info_query
#define os_cpu_usage_info_destroy oracle_os_cpu_usage_info_destroy
#define os_sleepto_ns oracle_os_sleepto_ns
#define os_sleepto_ns_fast oracle_os_sleepto_ns_fast
#define os_sleep_ms oracle_os_sleep_ms
#define os_gettime_ns oracle_os_gettime_ns
#define os_get_config_path oracle_os_get_config_path
#define os_get_config_path_ptr oracle_os_get_config_path_ptr
#define os_get_program_data_path oracle_os_get_program_data_path
#define os_get_program_data_path_ptr oracle_os_get_program_data_path_ptr
#define os_get_executable_path_ptr oracle_os_get_executable_path_ptr
#define os_file_exists oracle_os_file_exists
#define os_get_abs_path oracle_os_get_abs_path
#define os_get_abs_path_ptr oracle_os_get_abs_path_ptr
#define os_opendir oracle_os_opendir
#define os_readdir oracle_os_readdir
#define os_closedir oracle_os_closedir
#define os_get_free_space oracle_os_get_free_space
#define os_glob oracle_os_glob
#define os_globfree oracle_os_globfree
#define os_unlink oracle_os_unlink
#define os_rmdir oracle_os_rmdir
#define os_mkdir oracle_os_mkdir
#define os_rename oracle_os_rename
#define os_safe_replace oracle_os_safe_replace
#define DllMain oracle_DllMain
#define os_request_high_performance oracle_os_request_high_performance
#define os_end_high_performance oracle_os_end_high_performance
#define os_copyfile oracle_os_copyfile
#define os_getcwd oracle_os_getcwd
#define os_chdir oracle_os_chdir
#define get_dll_ver oracle_get_dll_ver
#define is_64_bit_windows oracle_is_64_bit_windows
#define is_arm64_windows oracle_is_arm64_windows
#define os_get_emulation_status oracle_os_get_emulation_status
#define get_reg_dword oracle_get_reg_dword
#define get_win_ver oracle_get_win_ver
#define get_win_release_id oracle_get_win_release_id
#define get_win_ver_int oracle_get_win_ver_int
#define os_inhibit_sleep_create oracle_os_inhibit_sleep_create
#define os_inhibit_sleep_set_active oracle_os_inhibit_sleep_set_active
#define os_inhibit_sleep_destroy oracle_os_inhibit_sleep_destroy
#define num_logical_cores oracle_num_logical_cores
/* The shared platform_host.c supplies the oracle's loud-abort versions. Keep
 * these real Windows definitions in the translation unit under unused names. */
#define os_breakpoint unreferenced_windows_os_breakpoint
#define os_oom unreferenced_windows_os_oom
#define os_get_physical_cores oracle_os_get_physical_cores
#define os_get_logical_cores oracle_os_get_logical_cores
#define os_get_sys_free_size oracle_os_get_sys_free_size
#define os_get_sys_total_size oracle_os_get_sys_total_size
#define os_get_proc_memory_usage oracle_os_get_proc_memory_usage
#define os_get_proc_resident_size oracle_os_get_proc_resident_size
#define os_get_proc_virtual_size oracle_os_get_proc_virtual_size
#define os_get_free_disk_space oracle_os_get_free_disk_space
#define os_generate_uuid oracle_os_generate_uuid
/* platform-conversions.c, compiled by oracle/platform.c */
#define os_utf8_to_wcs oracle_os_utf8_to_wcs
#define os_utf8_to_wcs_ptr oracle_os_utf8_to_wcs_ptr
#define os_wcs_to_utf8 oracle_os_wcs_to_utf8
#define os_wcs_to_utf8_ptr oracle_os_wcs_to_utf8_ptr
/* Variadic and wchar helpers live in the dstr oracle under these names.
 * The inline dstr helpers in dstr.h stay as written and call the Rust shims. */
#define dstr_printf oracle_dstr_printf
#define dstr_from_wcs oracle_dstr_from_wcs

/* clang-cl (the cross-lint and soldr MSVC target) also reports two idioms
 * in the unmodified original that cl.exe does not: an unused `size` and
 * `= {sizeof(x)}` struct initializers. */
#if defined(__clang__)
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunused-but-set-variable"
#pragma clang diagnostic ignored "-Wmissing-field-initializers"
#endif
#include "util/platform-windows.c.h"
#if defined(__clang__)
#pragma clang diagnostic pop
#endif

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
size_t oracle_win_version_info_size(void)
{
	return sizeof(struct win_version_info);
}
size_t oracle_win_version_info_align(void)
{
	return _Alignof(struct win_version_info);
}
size_t oracle_win_version_info_offset_major(void)
{
	return offsetof(struct win_version_info, major);
}
size_t oracle_win_version_info_offset_minor(void)
{
	return offsetof(struct win_version_info, minor);
}
size_t oracle_win_version_info_offset_build(void)
{
	return offsetof(struct win_version_info, build);
}
size_t oracle_win_version_info_offset_revis(void)
{
	return offsetof(struct win_version_info, revis);
}
size_t oracle_reg_dword_size(void)
{
	return sizeof(struct reg_dword);
}
size_t oracle_reg_dword_align(void)
{
	return _Alignof(struct reg_dword);
}
size_t oracle_reg_dword_offset_status(void)
{
	return offsetof(struct reg_dword, status);
}
size_t oracle_reg_dword_offset_size(void)
{
	return offsetof(struct reg_dword, size);
}
size_t oracle_reg_dword_offset_return_value(void)
{
	return offsetof(struct reg_dword, return_value);
}
size_t oracle_memory_status_ex_size(void)
{
	return sizeof(MEMORYSTATUSEX);
}
size_t oracle_process_memory_counters_size(void)
{
	return sizeof(PROCESS_MEMORY_COUNTERS);
}
size_t oracle_logical_processor_size(void)
{
	return sizeof(SYSTEM_LOGICAL_PROCESSOR_INFORMATION);
}
