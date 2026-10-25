// SPDX-FileCopyrightText: 2022 tytan652 <tytan652@tytanium.xyz>
//
// SPDX-License-Identifier: GPL-2.0-or-later

#include "plugins/obs-ffmpeg/build/obs-ffmpeg-vaapi.pch.h"

static bool version_logged = false;

/* Each render node is opened once and its answer for every codec kept
 * (vaapi-probe-policy.h): opening one can take most of a second. */
static struct vaapi_caps_cache device_caps = {0};
static pthread_mutex_t device_caps_mutex = PTHREAD_MUTEX_INITIALIZER;
static bool render_nodes_probed = false;

static uint32_t vaapi_probe_device(const char *device_path);

inline static VADisplay vaapi_open_display_drm(int *fd, const char *device_path)
{
	VADisplay va_dpy;

	if (!device_path)
		return NULL;

	*fd = open(device_path, O_RDWR | O_CLOEXEC);
	if (*fd < 0) {
		blog(LOG_ERROR, "VAAPI: Failed to open device '%s'", device_path);
		return NULL;
	}

	va_dpy = vaGetDisplayDRM(*fd);

	if (!va_dpy) {
		blog(LOG_ERROR, "VAAPI: Failed to initialize DRM display");
		return NULL;
	}

	return va_dpy;
}

inline static void vaapi_close_display_drm(int *fd)
{
	if (*fd < 0)
		return;

	close(*fd);
	*fd = -1;
}

static void vaapi_log_info_cb(void *user_context, const char *message)
{
	UNUSED_PARAMETER(user_context);

	// Libva message always ends with a newline
	struct dstr m;
	dstr_init_copy(&m, message);
	dstr_depad(&m);

	blog(LOG_DEBUG, "Libva: %s", m.array);

	dstr_free(&m);
}

static void vaapi_log_error_cb(void *user_context, const char *message)
{
	UNUSED_PARAMETER(user_context);

	// Libva message always ends with a newline
	struct dstr m;
	dstr_init_copy(&m, message);
	dstr_depad(&m);

	blog(LOG_DEBUG, "Libva error: %s", m.array);

	dstr_free(&m);
}

VADisplay vaapi_open_device(int *fd, const char *device_path, const char *func_name)
{
	VADisplay va_dpy;
	VAStatus va_status;
	int major, minor;
	const char *driver;

	va_dpy = vaapi_open_display_drm(fd, device_path);
	if (!va_dpy)
		return NULL;

	blog(LOG_DEBUG, "VAAPI: Initializing display in %s", func_name);

	vaSetInfoCallback(va_dpy, vaapi_log_info_cb, NULL);
	vaSetErrorCallback(va_dpy, vaapi_log_error_cb, NULL);

	va_status = vaInitialize(va_dpy, &major, &minor);

	if (va_status != VA_STATUS_SUCCESS) {
		blog(LOG_ERROR, "VAAPI: Failed to initialize display in %s", func_name);
		vaapi_close_device(fd, va_dpy);
		return NULL;
	}

	blog(LOG_DEBUG, "VAAPI: Display initialized");

	if (!version_logged) {
		blog(LOG_INFO, "VAAPI: API version %d.%d", major, minor);
		version_logged = true;
	}

	driver = vaQueryVendorString(va_dpy);

	blog(LOG_DEBUG, "VAAPI: '%s' in use for device '%s'", driver, device_path);

	return va_dpy;
}

void vaapi_close_device(int *fd, VADisplay dpy)
{
	vaTerminate(dpy);
	vaapi_close_display_drm(fd);
}

static uint32_t vaapi_display_ep_combo_rate_controls(VAProfile profile, VAEntrypoint entrypoint, VADisplay dpy,
						     const char *device_path)
{
	VAStatus va_status;
	VAConfigAttrib attrib[1];
	attrib->type = VAConfigAttribRateControl;

	va_status = vaGetConfigAttributes(dpy, profile, entrypoint, attrib, 1);

	switch (va_status) {
	case VA_STATUS_SUCCESS:
		return attrib->value;
	case VA_STATUS_ERROR_UNSUPPORTED_PROFILE:
		blog(LOG_DEBUG, "VAAPI: %s is not supported by the device '%s'", vaProfileStr(profile), device_path);
		return 0;
	case VA_STATUS_ERROR_UNSUPPORTED_ENTRYPOINT:
		blog(LOG_DEBUG, "VAAPI: %s %s is not supported by the device '%s'", vaProfileStr(profile),
		     vaEntrypointStr(entrypoint), device_path);
		return 0;
	default:
		blog(LOG_ERROR, "VAAPI: Fail to get RC attribute from the %s %s of the device '%s'",
		     vaProfileStr(profile), vaEntrypointStr(entrypoint), device_path);
		return 0;
	}
}

static bool vaapi_display_ep_combo_supported(VAProfile profile, VAEntrypoint entrypoint, VADisplay dpy,
					     const char *device_path)
{
	uint32_t ret = vaapi_display_ep_combo_rate_controls(profile, entrypoint, dpy, device_path);
	if (ret & VA_RC_CBR || ret & VA_RC_CQP || ret & VA_RC_VBR)
		return true;

	return false;
}

bool vaapi_device_rc_supported(VAProfile profile, VADisplay dpy, uint32_t rc, const char *device_path)
{
	uint32_t ret = vaapi_display_ep_combo_rate_controls(profile, VAEntrypointEncSlice, dpy, device_path);
	if (ret & rc)
		return true;
	ret = vaapi_display_ep_combo_rate_controls(profile, VAEntrypointEncSliceLP, dpy, device_path);
	if (ret & rc)
		return true;

	return false;
}

static bool vaapi_display_ep_bframe_supported(VAProfile profile, VAEntrypoint entrypoint, VADisplay dpy)
{
	VAStatus va_status;
	VAConfigAttrib attrib[1];
	attrib->type = VAConfigAttribEncMaxRefFrames;

	va_status = vaGetConfigAttributes(dpy, profile, entrypoint, attrib, 1);

	if (va_status == VA_STATUS_SUCCESS && attrib->value != VA_ATTRIB_NOT_SUPPORTED)
		return attrib->value >> 16;

	return false;
}

bool vaapi_device_bframe_supported(VAProfile profile, VADisplay dpy)
{
	bool ret = vaapi_display_ep_bframe_supported(profile, VAEntrypointEncSlice, dpy);
	if (ret)
		return true;
	ret = vaapi_display_ep_bframe_supported(profile, VAEntrypointEncSliceLP, dpy);
	if (ret)
		return true;

	return false;
}

#define CHECK_PROFILE(ret, profile, va_dpy, device_path)                                              \
	if (vaapi_display_ep_combo_supported(profile, VAEntrypointEncSlice, va_dpy, device_path)) {   \
		blog(LOG_DEBUG, "'%s' support encoding with %s", device_path, vaProfileStr(profile)); \
		ret |= true;                                                                          \
	}

#define CHECK_PROFILE_LP(ret, profile, va_dpy, device_path)                                                     \
	if (vaapi_display_ep_combo_supported(profile, VAEntrypointEncSliceLP, va_dpy, device_path)) {           \
		blog(LOG_DEBUG, "'%s' support low power encoding with %s", device_path, vaProfileStr(profile)); \
		ret |= true;                                                                                    \
	}

bool vaapi_display_h264_supported(VADisplay dpy, const char *device_path)
{
	bool ret = false;

	CHECK_PROFILE(ret, VAProfileH264ConstrainedBaseline, dpy, device_path);
	CHECK_PROFILE(ret, VAProfileH264Main, dpy, device_path);
	CHECK_PROFILE(ret, VAProfileH264High, dpy, device_path);

	if (!ret) {
		CHECK_PROFILE_LP(ret, VAProfileH264ConstrainedBaseline, dpy, device_path);
		CHECK_PROFILE_LP(ret, VAProfileH264Main, dpy, device_path);
		CHECK_PROFILE_LP(ret, VAProfileH264High, dpy, device_path);
	}

	return ret;
}

bool vaapi_device_h264_supported(const char *device_path)
{
	return (vaapi_probe_device(device_path) & VAAPI_CODEC_H264) != 0;
}

const char *vaapi_get_h264_default_device()
{
	static const char *default_h264_device = NULL;

	if (!default_h264_device) {
		bool ret = false;
		char path[32] = "/dev/dri/renderD1";
		for (int i = 28;; i++) {
			sprintf(path, "/dev/dri/renderD1%d", i);
			if (access(path, F_OK) != 0)
				break;

			ret = vaapi_device_h264_supported(path);
			if (ret) {
				default_h264_device = strdup(path);
				break;
			}
		}
	}

	return default_h264_device;
}

bool vaapi_display_av1_supported(VADisplay dpy, const char *device_path)
{
	bool ret = false;

	CHECK_PROFILE(ret, VAProfileAV1Profile0, dpy, device_path);

	if (!ret) {
		CHECK_PROFILE_LP(ret, VAProfileAV1Profile0, dpy, device_path);
	}

	return ret;
}

bool vaapi_device_av1_supported(const char *device_path)
{
	return (vaapi_probe_device(device_path) & VAAPI_CODEC_AV1) != 0;
}

const char *vaapi_get_av1_default_device()
{
	static const char *default_av1_device = NULL;

	if (!default_av1_device) {
		bool ret = false;
		char path[32] = "/dev/dri/renderD1";
		for (int i = 28;; i++) {
			sprintf(path, "/dev/dri/renderD1%d", i);
			if (access(path, F_OK) != 0)
				break;

			ret = vaapi_device_av1_supported(path);
			if (ret) {
				default_av1_device = strdup(path);
				break;
			}
		}
	}

	return default_av1_device;
}

#ifdef ENABLE_HEVC

bool vaapi_display_hevc_supported(VADisplay dpy, const char *device_path)
{
	bool ret = false;

	CHECK_PROFILE(ret, VAProfileHEVCMain, dpy, device_path);
	CHECK_PROFILE(ret, VAProfileHEVCMain10, dpy, device_path);

	if (!ret) {
		CHECK_PROFILE_LP(ret, VAProfileHEVCMain, dpy, device_path);
		CHECK_PROFILE_LP(ret, VAProfileHEVCMain10, dpy, device_path);
	}

	return ret;
}

bool vaapi_device_hevc_supported(const char *device_path)
{
	return (vaapi_probe_device(device_path) & VAAPI_CODEC_HEVC) != 0;
}

const char *vaapi_get_hevc_default_device()
{
	static const char *default_hevc_device = NULL;

	if (!default_hevc_device) {
		bool ret = false;
		char path[32] = "/dev/dri/renderD1";
		for (int i = 28;; i++) {
			sprintf(path, "/dev/dri/renderD1%d", i);
			if (access(path, F_OK) != 0)
				break;

			ret = vaapi_device_hevc_supported(path);
			if (ret) {
				default_hevc_device = strdup(path);
				break;
			}
		}
	}

	return default_hevc_device;
}

#endif // #ifdef ENABLE_HEVC

/* Whether the device's driver cannot encode (vaapi_driver_is_decode_only()),
 * read from the kernel driver link of its DRM node. */
static bool vaapi_device_decode_only(const char *device_path)
{
	char *node = realpath(device_path, NULL);
	if (!node)
		return false;

	char link[PATH_MAX];
	char target[PATH_MAX];
	const char *name = strrchr(node, '/');
	snprintf(link, sizeof(link), "/sys/class/drm/%s/device/driver", name ? name + 1 : node);
	free(node);

	ssize_t len = readlink(link, target, sizeof(target) - 1);
	if (len <= 0)
		return false;
	target[len] = '\0';

	return vaapi_driver_is_decode_only(target, getenv("LIBVA_DRIVER_NAME"));
}

static uint32_t vaapi_probe_device_uncached(const char *device_path)
{
	uint32_t flags = 0;
	int drm_fd = -1;

	if (vaapi_device_decode_only(device_path)) {
		blog(LOG_INFO, "VAAPI: Skipping '%s': its driver (nvidia-vaapi-driver) does not encode", device_path);
		return flags;
	}

	VADisplay va_dpy = vaapi_open_device(&drm_fd, device_path, "vaapi_probe_device");
	if (!va_dpy)
		return flags;

	if (vaapi_display_h264_supported(va_dpy, device_path))
		flags |= VAAPI_CODEC_H264;
	if (vaapi_display_av1_supported(va_dpy, device_path))
		flags |= VAAPI_CODEC_AV1;
#ifdef ENABLE_HEVC
	if (vaapi_display_hevc_supported(va_dpy, device_path))
		flags |= VAAPI_CODEC_HEVC;
#endif

	vaapi_close_device(&drm_fd, va_dpy);
	return flags;
}

#define VAAPI_MAX_RENDER_NODES 64

struct vaapi_probe_job {
	char path[32];
	uint32_t flags;
	pthread_t thread;
	bool started;
};

static void *vaapi_probe_thread(void *param)
{
	struct vaapi_probe_job *job = param;
	job->flags = vaapi_probe_device_uncached(job->path);
	return NULL;
}

/* The render nodes the default-device lookups walk (/dev/dri/renderD128 and
 * up) are opened at the same time, one thread each, so a machine with several
 * GPUs waits for the slowest one instead of the sum. Called with
 * device_caps_mutex held. */
static void vaapi_probe_render_nodes(void)
{
	struct vaapi_probe_job jobs[VAAPI_MAX_RENDER_NODES];
	size_t count = 0;

	if (render_nodes_probed)
		return;
	render_nodes_probed = true;

	for (int i = 28; count < VAAPI_MAX_RENDER_NODES; i++) {
		struct vaapi_probe_job *job = &jobs[count];
		snprintf(job->path, sizeof(job->path), "/dev/dri/renderD1%d", i);
		if (access(job->path, F_OK) != 0)
			break;
		job->flags = 0;
		job->started = pthread_create(&job->thread, NULL, vaapi_probe_thread, job) == 0;
		count++;
	}

	for (size_t i = 0; i < count; i++) {
		if (jobs[i].started)
			pthread_join(jobs[i].thread, NULL);
		else
			jobs[i].flags = vaapi_probe_device_uncached(jobs[i].path);
	}

	for (size_t i = 0; i < count; i++)
		vaapi_caps_cache_add(&device_caps, jobs[i].path, jobs[i].flags);
}

static uint32_t vaapi_probe_device_cb(const char *device_path, void *param)
{
	UNUSED_PARAMETER(param);
	return vaapi_probe_device_uncached(device_path);
}

static uint32_t vaapi_probe_device(const char *device_path)
{
	if (!device_path)
		return 0;

	pthread_mutex_lock(&device_caps_mutex);
	if (strncmp(device_path, "/dev/dri/renderD", 16) == 0)
		vaapi_probe_render_nodes();
	uint32_t flags = vaapi_caps_cache_get(&device_caps, device_path, vaapi_probe_device_cb, NULL);
	pthread_mutex_unlock(&device_caps_mutex);
	return flags;
}

void vaapi_free_device_caps(void)
{
	pthread_mutex_lock(&device_caps_mutex);
	vaapi_caps_cache_free(&device_caps);
	render_nodes_probed = false;
	pthread_mutex_unlock(&device_caps_mutex);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef CHECK_PROFILE
#undef CHECK_PROFILE_LP
#undef VAAPI_MAX_RENDER_NODES
