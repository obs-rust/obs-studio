/* obs-qsv11: on Linux, Initialize() never unloaded the VPL loader it got
 * from MFXLoad(), on success or failure, and the session it created was
 * never closed. When MFXCreateSession(), vaInitialize() or
 * MFXVideoCORE_SetHandle() failed it also left the session, the VA display
 * or the device open (obs-rust/obs-studio#2393).
 *
 * The real common_utils_linux.cpp is compiled in. The libvpl and libva
 * calls Initialize() makes are wrapped (--wrap): the wrappers hand out
 * fake objects, count what is acquired and released, and fail the step a
 * test asks to fail. The "device" is a temporary file and its descriptors
 * are counted in /proc/self/fd, so no GPU is needed.
 *
 * The frame allocator, simple_alloc(), is tested the same way: its VA
 * surfaces, textures and allocations are counted across each failure exit
 * after vaCreateSurfaces() (obs-rust/obs-studio#2396). */

/* The C++ headers go first: cmocka.h defines macros (fail, ...) that clash
 * with libstdc++. */
#include "plugins/obs-qsv11/common_utils_linux.cpp.hpp"

#include <limits.h>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
// The Windows cmocka.h has no extern "C" guard of its own.
extern "C" {
#include <cmocka.h>
}

enum fail_step {
	FAIL_NONE,
	FAIL_OPEN,
	FAIL_DISPLAY,
	FAIL_SESSION,
	FAIL_VA_INIT,
	FAIL_SET_HANDLE,
};

static char device_path[PATH_MAX];
static enum fail_step failing;

/* Live objects: acquired minus released. */
static int loaders;
static int sessions;
static int displays;
/* Order of releases, to check sessions close before their loader. */
static bool loader_unloaded_with_session_open;

static int fake_loader_obj;
static int fake_config_obj;
static int fake_session_obj;
static int fake_display_obj;

/* simple_alloc(): the surface and the call that fail, and what is live. */
enum alloc_fail_step {
	ALLOC_FAIL_NONE,
	ALLOC_FAIL_EXPORT,
	ALLOC_FAIL_TEX_Y,
	ALLOC_FAIL_TEX_UV,
};
#define ALLOC_SURFACES 3
#define ALLOC_FAILING_SURFACE 1
static enum alloc_fail_step alloc_failing;
static int va_surfaces;
static int textures;
static int exports;
static int textures_created;
static int fake_texture_obj;

extern "C" {
mfxLoader __wrap_MFXLoad(void);
void __wrap_MFXUnload(mfxLoader loader);
mfxConfig __wrap_MFXCreateConfig(mfxLoader loader);
mfxStatus __wrap_MFXSetConfigFilterProperty(mfxConfig config, const mfxU8 *name, mfxVariant value);
mfxStatus __wrap_MFXCreateSession(mfxLoader loader, mfxU32 i, mfxSession *session);
mfxStatus __wrap_MFXClose(mfxSession session);
mfxStatus __wrap_MFXVideoCORE_SetHandle(mfxSession session, mfxHandleType type, mfxHDL hdl);
VADisplay __wrap_vaGetDisplayDRM(int fd);
VAStatus __wrap_vaInitialize(VADisplay dpy, int *major, int *minor);
VAStatus __wrap_vaTerminate(VADisplay dpy);
mfxStatus __wrap_MFXVideoCORE_GetHandle(mfxSession session, mfxHandleType type, mfxHDL *hdl);
VAStatus __wrap_vaCreateSurfaces(VADisplay dpy, unsigned int format, unsigned int width, unsigned int height,
				 VASurfaceID *surfaces, unsigned int num_surfaces, VASurfaceAttrib *attrib_list,
				 unsigned int num_attribs);
VAStatus __wrap_vaDestroySurfaces(VADisplay dpy, VASurfaceID *surfaces, int num_surfaces);
VAStatus __wrap_vaExportSurfaceHandle(VADisplay dpy, VASurfaceID surface_id, uint32_t mem_type, uint32_t flags,
				      void *descriptor);
gs_texture_t *__wrap_gs_texture_create_from_dmabuf(unsigned int width, unsigned int height, uint32_t drm_format,
						   enum gs_color_format color_format, uint32_t n_planes, const int *fds,
						   const uint32_t *strides, const uint32_t *offsets,
						   const uint64_t *modifiers);
void __wrap_gs_texture_destroy(gs_texture_t *tex);
void __wrap_obs_enter_graphics(void);
void __wrap_obs_leave_graphics(void);

mfxLoader __wrap_MFXLoad(void)
{
	loaders++;
	return (mfxLoader)&fake_loader_obj;
}

void __wrap_MFXUnload(mfxLoader loader)
{
	assert_ptr_equal(loader, &fake_loader_obj);
	if (sessions > 0) {
		loader_unloaded_with_session_open = true;
	}
	loaders--;
}

mfxConfig __wrap_MFXCreateConfig(mfxLoader loader)
{
	assert_ptr_equal(loader, &fake_loader_obj);
	return (mfxConfig)&fake_config_obj;
}

mfxStatus __wrap_MFXSetConfigFilterProperty(mfxConfig config, const mfxU8 *name, mfxVariant value)
{
	(void)name;
	(void)value;
	assert_ptr_equal(config, &fake_config_obj);
	return MFX_ERR_NONE;
}

mfxStatus __wrap_MFXCreateSession(mfxLoader loader, mfxU32 i, mfxSession *session)
{
	(void)i;
	assert_ptr_equal(loader, &fake_loader_obj);
	if (failing == FAIL_SESSION) {
		return MFX_ERR_NOT_FOUND;
	}
	sessions++;
	*session = (mfxSession)&fake_session_obj;
	return MFX_ERR_NONE;
}

mfxStatus __wrap_MFXClose(mfxSession session)
{
	assert_ptr_equal(session, &fake_session_obj);
	sessions--;
	return MFX_ERR_NONE;
}

mfxStatus __wrap_MFXVideoCORE_SetHandle(mfxSession session, mfxHandleType type, mfxHDL hdl)
{
	assert_ptr_equal(session, &fake_session_obj);
	assert_int_equal(type, MFX_HANDLE_VA_DISPLAY);
	assert_ptr_equal(hdl, &fake_display_obj);
	return failing == FAIL_SET_HANDLE ? MFX_ERR_UNDEFINED_BEHAVIOR : MFX_ERR_NONE;
}

VADisplay __wrap_vaGetDisplayDRM(int fd)
{
	(void)fd;
	if (failing == FAIL_DISPLAY) {
		return NULL;
	}
	displays++;
	return (VADisplay)&fake_display_obj;
}

VAStatus __wrap_vaInitialize(VADisplay dpy, int *major, int *minor)
{
	assert_ptr_equal(dpy, &fake_display_obj);
	if (failing == FAIL_VA_INIT) {
		return VA_STATUS_ERROR_UNKNOWN;
	}
	*major = 1;
	*minor = 22;
	return VA_STATUS_SUCCESS;
}

VAStatus __wrap_vaTerminate(VADisplay dpy)
{
	assert_ptr_equal(dpy, &fake_display_obj);
	displays--;
	return VA_STATUS_SUCCESS;
}

mfxStatus __wrap_MFXVideoCORE_GetHandle(mfxSession session, mfxHandleType type, mfxHDL *hdl)
{
	assert_ptr_equal(session, &fake_session_obj);
	assert_int_equal(type, MFX_HANDLE_VA_DISPLAY);
	*hdl = &fake_display_obj;
	return MFX_ERR_NONE;
}

VAStatus __wrap_vaCreateSurfaces(VADisplay dpy, unsigned int format, unsigned int width, unsigned int height,
				 VASurfaceID *surfaces, unsigned int num_surfaces, VASurfaceAttrib *attrib_list,
				 unsigned int num_attribs)
{
	(void)format;
	(void)width;
	(void)height;
	(void)attrib_list;
	(void)num_attribs;
	assert_ptr_equal(dpy, &fake_display_obj);
	for (unsigned int i = 0; i < num_surfaces; i++) {
		surfaces[i] = 100 + i;
	}
	va_surfaces += (int)num_surfaces;
	return VA_STATUS_SUCCESS;
}

VAStatus __wrap_vaDestroySurfaces(VADisplay dpy, VASurfaceID *surfaces, int num_surfaces)
{
	assert_ptr_equal(dpy, &fake_display_obj);
	for (int i = 0; i < num_surfaces; i++) {
		assert_int_equal(surfaces[i], 100 + i);
	}
	va_surfaces -= num_surfaces;
	return VA_STATUS_SUCCESS;
}

/* Exports the surface as one dmabuf: a descriptor of the device file. */
VAStatus __wrap_vaExportSurfaceHandle(VADisplay dpy, VASurfaceID surface_id, uint32_t mem_type, uint32_t flags,
				      void *descriptor)
{
	(void)mem_type;
	(void)flags;
	assert_ptr_equal(dpy, &fake_display_obj);
	if (alloc_failing == ALLOC_FAIL_EXPORT && surface_id == 100 + ALLOC_FAILING_SURFACE) {
		return VA_STATUS_ERROR_UNKNOWN;
	}
	VADRMPRIMESurfaceDescriptor *desc = (VADRMPRIMESurfaceDescriptor *)descriptor;
	desc->width = 64;
	desc->height = 64;
	desc->num_objects = 1;
	desc->objects[0].fd = open(device_path, O_RDONLY | O_CLOEXEC);
	assert_true(desc->objects[0].fd >= 0);
	desc->num_layers = 2;
	exports++;
	return VA_STATUS_SUCCESS;
}

gs_texture_t *__wrap_gs_texture_create_from_dmabuf(unsigned int width, unsigned int height, uint32_t drm_format,
						   enum gs_color_format color_format, uint32_t n_planes, const int *fds,
						   const uint32_t *strides, const uint32_t *offsets,
						   const uint64_t *modifiers)
{
	(void)width;
	(void)height;
	(void)drm_format;
	(void)n_planes;
	(void)fds;
	(void)strides;
	(void)offsets;
	(void)modifiers;
	/* Two per surface, Y then UV. */
	int surface = textures_created / 2;
	textures_created++;
	if (surface == ALLOC_FAILING_SURFACE && ((alloc_failing == ALLOC_FAIL_TEX_Y && color_format == GS_R8) ||
						 (alloc_failing == ALLOC_FAIL_TEX_UV && color_format == GS_R8G8))) {
		return NULL;
	}
	textures++;
	return (gs_texture_t *)&fake_texture_obj;
}

void __wrap_gs_texture_destroy(gs_texture_t *tex)
{
	if (tex) {
		assert_ptr_equal(tex, &fake_texture_obj);
		textures--;
	}
}

void __wrap_obs_enter_graphics(void) {}

void __wrap_obs_leave_graphics(void) {}
}

static int setup(void **state)
{
	(void)state;
	const char *dir = getenv("TMPDIR");
	snprintf(device_path, sizeof(device_path), "%s/qsv-init-release-XXXXXX", dir && *dir ? dir : "/tmp");
	int fd = mkstemp(device_path);
	if (fd < 0) {
		return -1;
	}
	close(fd);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	unlink(device_path);
	return 0;
}

/* Closes, and counts, the descriptors of the device this process holds. */
static int close_left_open(void)
{
	int left = 0;
	for (int fd = 3; fd < 1024; fd++) {
		char link[64];
		char target[PATH_MAX];
		snprintf(link, sizeof(link), "/proc/self/fd/%d", fd);
		ssize_t n = readlink(link, target, sizeof(target) - 1);
		if (n < 0) {
			continue;
		}
		target[n] = 0;
		if (strcmp(target, device_path) == 0) {
			close(fd);
			left++;
		}
	}
	return left;
}

static mfxStatus open_encoder(enum fail_step step, void **data)
{
	failing = step;
	loaders = 0;
	sessions = 0;
	displays = 0;
	loader_unloaded_with_session_open = false;
	/* A missing device makes open() fail. */
	default_h264_device = step == FAIL_OPEN ? "/nonexistent/qsv-init-release" : device_path;

	mfxVersion ver = {};
	mfxSession session = nullptr;
	*data = nullptr;
	mfxStatus sts = Initialize(ver, &session, nullptr, nullptr, false, QSV_CODEC_AVC, data);

	default_h264_device = nullptr;
	return sts;
}

/* Nothing Initialize() acquired is still held. */
static void check_all_released(const char *what)
{
	int fds = close_left_open();
	if (loaders || sessions || displays || fds || loader_unloaded_with_session_open) {
		fprintf(stderr, "%s left %d loaders, %d sessions, %d VA displays and %d device descriptors%s\n", what,
			loaders, sessions, displays, fds,
			loader_unloaded_with_session_open ? "; the loader was unloaded with its session open" : "");
		fail();
	}
}

static void check_failed_open(enum fail_step step, const char *what)
{
	void *data;
	mfxStatus sts = open_encoder(step, &data);

	if (sts >= MFX_ERR_NONE) {
		fprintf(stderr, "Initialize() succeeded although %s failed\n", what);
		fail();
	}
	assert_null(data);
	check_all_released(what);
}

/* An encoder opened and closed releases everything */
static void open_and_close_release_all(void **state)
{
	(void)state;
	void *data;
	mfxStatus sts = open_encoder(FAIL_NONE, &data);

	assert_int_equal(sts, MFX_ERR_NONE);
	assert_non_null(data);
	assert_int_equal(loaders, 1);
	assert_int_equal(sessions, 1);
	assert_int_equal(displays, 1);

	ReleaseSessionData(data);
	check_all_released("open and close");
}

static void failed_device_open_releases_all(void **state)
{
	(void)state;
	check_failed_open(FAIL_OPEN, "open()");
}

static void failed_display_releases_all(void **state)
{
	(void)state;
	check_failed_open(FAIL_DISPLAY, "vaGetDisplayDRM()");
}

static void failed_session_releases_all(void **state)
{
	(void)state;
	check_failed_open(FAIL_SESSION, "MFXCreateSession()");
}

static void failed_va_init_releases_all(void **state)
{
	(void)state;
	check_failed_open(FAIL_VA_INIT, "vaInitialize()");
}

static void failed_set_handle_releases_all(void **state)
{
	(void)state;
	check_failed_open(FAIL_SET_HANDLE, "MFXVideoCORE_SetHandle()");
}

static mfxStatus alloc_frames(enum alloc_fail_step step, mfxFrameAllocResponse *response)
{
	alloc_failing = step;
	va_surfaces = 0;
	textures = 0;
	exports = 0;
	textures_created = 0;

	mfxSession session = (mfxSession)&fake_session_obj;
	mfxFrameAllocRequest request = {};
	request.Type = MFX_MEMTYPE_VIDEO_MEMORY_ENCODER_TARGET | MFX_MEMTYPE_FROM_ENCODE;
	request.Info.FourCC = MFX_FOURCC_NV12;
	request.Info.Width = 64;
	request.Info.Height = 64;
	request.NumFrameMin = ALLOC_SURFACES;
	request.NumFrameSuggested = ALLOC_SURFACES;
	*response = {};
	return simple_alloc(&session, &request, response);
}

/* Nothing simple_alloc() acquired is still held. */
static void check_frames_released(const char *what, long allocs_before)
{
	long allocs = bnum_allocs() - allocs_before;
	int fds = close_left_open();
	if (va_surfaces || textures || allocs || fds) {
		fprintf(stderr, "%s left %d VA surfaces, %d textures, %ld allocations and %d dmabuf descriptors\n",
			what, va_surfaces, textures, allocs, fds);
		fail();
	}
}

static void check_failed_alloc(enum alloc_fail_step step, const char *what)
{
	long allocs_before = bnum_allocs();
	mfxFrameAllocResponse response;
	mfxStatus sts = alloc_frames(step, &response);

	assert_int_equal(sts, MFX_ERR_MEMORY_ALLOC);
	assert_null(response.mids);
	assert_int_equal(response.NumFrameActual, 0);
	assert_int_equal(exports, step == ALLOC_FAIL_EXPORT ? ALLOC_FAILING_SURFACE : ALLOC_FAILING_SURFACE + 1);
	check_frames_released(what, allocs_before);
}

/* Frames allocated and freed release everything */
static void alloc_and_free_release_all(void **state)
{
	(void)state;
	long allocs_before = bnum_allocs();
	mfxFrameAllocResponse response;
	mfxStatus sts = alloc_frames(ALLOC_FAIL_NONE, &response);

	assert_int_equal(sts, MFX_ERR_NONE);
	assert_non_null(response.mids);
	assert_int_equal(response.NumFrameActual, ALLOC_SURFACES);
	assert_int_equal(va_surfaces, ALLOC_SURFACES);
	assert_int_equal(textures, 2 * ALLOC_SURFACES);

	mfxSession session = (mfxSession)&fake_session_obj;
	assert_int_equal(simple_free(&session, &response), MFX_ERR_NONE);
	check_frames_released("alloc and free", allocs_before);
}

static void failed_export_releases_frames(void **state)
{
	(void)state;
	check_failed_alloc(ALLOC_FAIL_EXPORT, "vaExportSurfaceHandle()");
}

static void failed_y_texture_releases_frames(void **state)
{
	(void)state;
	check_failed_alloc(ALLOC_FAIL_TEX_Y, "the Y texture");
}

static void failed_uv_texture_releases_frames(void **state)
{
	(void)state;
	check_failed_alloc(ALLOC_FAIL_TEX_UV, "the UV texture");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(open_and_close_release_all),       cmocka_unit_test(failed_device_open_releases_all),
		cmocka_unit_test(failed_display_releases_all),      cmocka_unit_test(failed_session_releases_all),
		cmocka_unit_test(failed_va_init_releases_all),      cmocka_unit_test(failed_set_handle_releases_all),
		cmocka_unit_test(alloc_and_free_release_all),       cmocka_unit_test(failed_export_releases_frames),
		cmocka_unit_test(failed_y_texture_releases_frames), cmocka_unit_test(failed_uv_texture_releases_frames),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
