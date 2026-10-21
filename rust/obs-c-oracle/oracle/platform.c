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
 * defines it on non-Apple Unix. obs_get_video_info and the clock of
 * platform-filename.c are fixtures at the end of this file, and
 * os_breakpoint / os_oom become loud aborts in platform_host.c instead of
 * libobs's SIGTRAP. */
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
/* os_generate_formatted_filename's clock: time() becomes the settable one
 * below, only for this file, so a test can give the C and Rust the same
 * second. <time.h> was included above, under the plain name. */
static time_t filename_now;
static time_t oracle_platform_filename_time(time_t *t)
{
	if (t)
		*t = filename_now;
	return filename_now;
}
#define time oracle_platform_filename_time
#include "util/platform-filename.c.h"
#undef time
#include "util/platform-mkdirs.c.h"
#include "util/platform-time.c.h"
#include "util/platform-conversions.c.h"
#if defined(__APPLE__) && defined(__clang__)
#pragma clang diagnostic pop
#endif

/* Test-only exports called from Rust over FFI (src/platform_time.rs): the
 * os_nstime_to_timespec base hook and the struct timespec layout. The hook
 * makes a NULL-storage call so pthread_once has run, then replaces the base it
 * recorded with the given one. */
void oracle_platform_time_set_base(int64_t sec, long nsec, uint64_t timestamp);
size_t oracle_timespec_size(void);
size_t oracle_timespec_align(void);
size_t oracle_timespec_offset_tv_sec(void);
size_t oracle_timespec_offset_tv_nsec(void);

void oracle_platform_time_set_base(int64_t sec, long nsec, uint64_t timestamp)
{
	os_nstime_to_timespec(0, NULL);
	timespec_offset.ts.tv_sec = (time_t)sec;
	timespec_offset.ts.tv_nsec = nsec;
	timespec_offset.ts_valid = true;
	timespec_offset.timestamp = timestamp;
}

size_t oracle_timespec_size(void)
{
	return sizeof(struct timespec);
}

size_t oracle_timespec_align(void)
{
	return _Alignof(struct timespec);
}

size_t oracle_timespec_offset_tv_sec(void)
{
	return offsetof(struct timespec, tv_sec);
}

size_t oracle_timespec_offset_tv_nsec(void)
{
	return offsetof(struct timespec, tv_nsec);
}

/* Test-only fixtures and exports for os_generate_formatted_filename, called
 * from Rust over FFI (src/platform_filename.rs).
 *
 * Intentionally different from the original: the real obs_get_video_info is
 * in libobs/obs.c, which pulls in the whole video pipeline that the oracle
 * does not build. Both the C (oracle_obs_get_video_info, through the rename
 * above) and the Rust shim (the plain obs_get_video_info, which libobs
 * provides in libobs-with-rust) read the same fixture: it copies the set
 * struct and returns the set result, and counts its calls. The fixture and
 * the clock are plain statics: callers must not set them from two threads at
 * once. */
static struct obs_video_info filename_ovi;
static bool filename_ovi_result;
static int filename_ovi_calls;

bool obs_get_video_info(struct obs_video_info *ovi)
{
	filename_ovi_calls++;
	*ovi = filename_ovi;
	return filename_ovi_result;
}

#undef obs_get_video_info
bool obs_get_video_info(struct obs_video_info *ovi);
bool obs_get_video_info(struct obs_video_info *ovi)
{
	return oracle_obs_get_video_info(ovi);
}

void oracle_platform_filename_set_time(int64_t now);
void oracle_platform_filename_set_video_info(const struct obs_video_info *ovi, bool result);
int oracle_platform_filename_video_info_calls(void);
size_t oracle_obs_video_info_size(void);
size_t oracle_obs_video_info_align(void);
void oracle_obs_video_info_offsets(size_t out[13]);

void oracle_platform_filename_set_time(int64_t now)
{
	filename_now = (time_t)now;
}

void oracle_platform_filename_set_video_info(const struct obs_video_info *ovi, bool result)
{
	filename_ovi = *ovi;
	filename_ovi_result = result;
}

int oracle_platform_filename_video_info_calls(void)
{
	return filename_ovi_calls;
}

size_t oracle_obs_video_info_size(void)
{
	return sizeof(struct obs_video_info);
}

size_t oracle_obs_video_info_align(void)
{
	return _Alignof(struct obs_video_info);
}

/* In declaration order: graphics_module through scale_type. */
void oracle_obs_video_info_offsets(size_t out[13])
{
	out[0] = offsetof(struct obs_video_info, graphics_module);
	out[1] = offsetof(struct obs_video_info, fps_num);
	out[2] = offsetof(struct obs_video_info, fps_den);
	out[3] = offsetof(struct obs_video_info, base_width);
	out[4] = offsetof(struct obs_video_info, base_height);
	out[5] = offsetof(struct obs_video_info, output_width);
	out[6] = offsetof(struct obs_video_info, output_height);
	out[7] = offsetof(struct obs_video_info, output_format);
	out[8] = offsetof(struct obs_video_info, adapter);
	out[9] = offsetof(struct obs_video_info, gpu_conversion);
	out[10] = offsetof(struct obs_video_info, colorspace);
	out[11] = offsetof(struct obs_video_info, range);
	out[12] = offsetof(struct obs_video_info, scale_type);
}
