/* Test-only oracle: the original libobs/util/platform.c, unmodified, with every
 * global symbol renamed to oracle_* so it can link next to the Rust
 * implementation, the way oracle/platform_nix_posix.c does for
 * platform-nix.c. The functions the oracle TUs call are renamed in those TUs
 * too (see each file's header comment), so the oracle runs the original C all
 * the way down instead of a hand copy.
 *
 * This replaces the hand-written platform stand-ins that used to live in
 * test_stubs.c, platform_conv_host.c, config_file_host.c, file_serializer_host.c
 * and obs_data_host.c. Their bodies were copied from libobs sources, so an edit
 * to platform.c could otherwise leave the oracle stale (#145).
 *
 * The file's libobs dependencies are routed at this crate's own oracle copies.
 * os_mkdir, os_safe_replace and os_gettime_ns are the renamed ones from
 * platform_nix_posix.c; the clock additionally needs a plain-named provider on
 * macOS and Windows, which platform_host.c supplies because the Rust shim only
 * defines it on non-Apple Unix. obs_get_video_info is a stub in
 * platform_host.c, and os_breakpoint / os_oom become loud aborts there instead
 * of libobs's SIGTRAP. */
#define _FILE_OFFSET_BITS 64
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

/* this file's exports */
#define os_wfopen oracle_os_wfopen
#define os_fopen oracle_os_fopen
#define os_fseeki64 oracle_os_fseeki64
#define os_ftelli64 oracle_os_ftelli64
#define os_fread_mbs oracle_os_fread_mbs
#define os_fread_utf8 oracle_os_fread_utf8
#define os_quick_read_mbs_file oracle_os_quick_read_mbs_file
#define os_quick_read_utf8_file oracle_os_quick_read_utf8_file
#define os_quick_write_mbs_file oracle_os_quick_write_mbs_file
#define os_quick_write_utf8_file oracle_os_quick_write_utf8_file
#define os_quick_write_utf8_file_safe oracle_os_quick_write_utf8_file_safe
#define os_get_file_size oracle_os_get_file_size
/* os_stat is `#define os_stat stat` in platform.h on non-Windows: the
 * definition keeps that, so it is renamed only on Windows, where obs-util
 * defines the plain os_stat. */
#ifdef _WIN32
#define os_stat oracle_os_stat
#endif
#define os_fgetsize oracle_os_fgetsize
#define os_mbs_to_wcs oracle_os_mbs_to_wcs
#define os_utf8_to_wcs oracle_os_utf8_to_wcs
#define os_wcs_to_mbs oracle_os_wcs_to_mbs
#define os_wcs_to_utf8 oracle_os_wcs_to_utf8
#define os_mbs_to_wcs_ptr oracle_os_mbs_to_wcs_ptr
#define os_utf8_to_wcs_ptr oracle_os_utf8_to_wcs_ptr
#define os_wcs_to_mbs_ptr oracle_os_wcs_to_mbs_ptr
#define os_wcs_to_utf8_ptr oracle_os_wcs_to_utf8_ptr
#define os_utf8_to_mbs_ptr oracle_os_utf8_to_mbs_ptr
#define os_mbs_to_utf8_ptr oracle_os_mbs_to_utf8_ptr
#define os_strtod oracle_os_strtod
#define os_dtostr oracle_os_dtostr
#define os_mkdirs oracle_os_mkdirs
#define os_generate_formatted_filename oracle_os_generate_formatted_filename
#define os_nstime_to_timespec oracle_os_nstime_to_timespec

/* libobs dependencies: the oracle copies */
#define astrcmp_n oracle_astrcmp_n
#define bmalloc oracle_bmalloc
#define bfree oracle_bfree
#define brealloc oracle_brealloc
#define blog oracle_blog
#define dstr_cat_dstr oracle_dstr_cat_dstr
#define dstr_copy oracle_dstr_copy
#define dstr_left oracle_dstr_left
#define dstr_mid oracle_dstr_mid
#define dstr_ncat oracle_dstr_ncat
#define dstr_replace oracle_dstr_replace
#define dstr_right oracle_dstr_right
#define utf8_to_wchar oracle_utf8_to_wchar
#define wchar_to_utf8 oracle_wchar_to_utf8
/* platform-nix.c / platform-windows.c, and obs-video.c which the oracle does
 * not build: see oracle/platform_host.c */
#define os_mkdir oracle_os_mkdir
#define os_safe_replace oracle_os_safe_replace
#define os_gettime_ns oracle_os_gettime_ns
#define obs_get_video_info oracle_obs_get_video_info

/* platform.c - the real libobs source, unmodified, compiled here. */

/* libobs' own build explicitly disables -Wsign-conversion
 * (cmake/linux/compilerconfig.cmake:30 and cmake/windows/
 * compilerconfig.cmake:123), so its code does not build clean under the
 * oracle's stricter ratchet (rust/obs-c-oracle/build.rs's STRICT_FLAGS,
 * obs-rust/obs-studio#516). Widen only inside this include: the oracle's job
 * is to keep the wrappers it owns warning-clean, not to rewrite libobs for a
 * warning libobs does not ask for. A no-diagnostic push is harmless where the
 * compiler already supports the pragma. */
#if defined(__GNUC__) && !defined(__clang__)
#pragma GCC diagnostic ignored "-Wsign-conversion"
#elif defined(_MSC_VER)
#pragma warning(disable : 4245)
#else
#pragma clang diagnostic ignored "-Wsign-conversion"
#endif

/* The Rust oracle cross-compiles for macOS 10.12, while OBS's app target is
 * macOS 13.0. Keep the included libobs source unchanged and scope its newer
 * API availability warning away from the oracle's strict cross-build. */
#if defined(__APPLE__) && defined(__clang__)
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunguarded-availability-new"
#endif
#include "util/platform-file.c.h"
#include "util/platform.c.h"
#if defined(__APPLE__) && defined(__clang__)
#pragma clang diagnostic pop
#endif
