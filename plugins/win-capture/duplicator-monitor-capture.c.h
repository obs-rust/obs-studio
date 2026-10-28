#include "plugins/win-capture/build/win-capture.pch.h"

#define do_log(level, format, ...) \
	blog(level, "[duplicator-monitor-capture: '%s'] " format, obs_source_get_name(capture->source), ##__VA_ARGS__)

#define warn(format, ...) do_log(LOG_WARNING, format, ##__VA_ARGS__)
#define info(format, ...) do_log(LOG_INFO, format, ##__VA_ARGS__)
#define debug(format, ...) do_log(LOG_DEBUG, format, ##__VA_ARGS__)

/* clang-format off */

#define TEXT_MONITOR_CAPTURE obs_module_text("MonitorCapture")
#define TEXT_CAPTURE_CURSOR  obs_module_text("CaptureCursor")
#define TEXT_COMPATIBILITY   obs_module_text("Compatibility")
#define TEXT_MONITOR         obs_module_text("Monitor")
#define TEXT_PRIMARY_MONITOR obs_module_text("PrimaryMonitor")
#define TEXT_METHOD          obs_module_text("Method")
#define TEXT_METHOD_AUTO     obs_module_text("WindowCapture.Method.Auto")
#define TEXT_METHOD_DXGI     obs_module_text("Method.DXGI")
#define TEXT_METHOD_WGC      obs_module_text("Method.WindowsGraphicsCapture")
#define TEXT_FORCE_SDR       obs_module_text("ForceSdr")

/* clang-format on */

#define RESET_INTERVAL_SEC 3.0f

#define INVALID_DISPLAY "DUMMY"

typedef BOOL (*PFN_winrt_capture_supported)();
typedef BOOL (*PFN_winrt_capture_cursor_toggle_supported)();
typedef struct winrt_capture *(*PFN_winrt_capture_init_monitor)(BOOL cursor, HMONITOR monitor, BOOL force_sdr);
typedef void (*PFN_winrt_capture_free)(struct winrt_capture *capture);

typedef BOOL (*PFN_winrt_capture_active)(const struct winrt_capture *capture);
typedef enum gs_color_space (*PFN_winrt_capture_get_color_space)(const struct winrt_capture *capture);
typedef void (*PFN_winrt_capture_render)(struct winrt_capture *capture);
typedef uint32_t (*PFN_winrt_capture_width)(const struct winrt_capture *capture);
typedef uint32_t (*PFN_winrt_capture_height)(const struct winrt_capture *capture);

struct winrt_exports {
	PFN_winrt_capture_supported winrt_capture_supported;
	PFN_winrt_capture_cursor_toggle_supported winrt_capture_cursor_toggle_supported;
	PFN_winrt_capture_init_monitor winrt_capture_init_monitor;
	PFN_winrt_capture_free winrt_capture_free;
	PFN_winrt_capture_active winrt_capture_active;
	PFN_winrt_capture_get_color_space winrt_capture_get_color_space;
	PFN_winrt_capture_render winrt_capture_render;
	PFN_winrt_capture_width winrt_capture_width;
	PFN_winrt_capture_height winrt_capture_height;
};

enum display_capture_method {
	METHOD_AUTO,
	METHOD_DXGI,
	METHOD_WGC,
};

struct duplicator_capture {
	obs_source_t *source;
	pthread_mutex_t update_mutex;
	char monitor_id[128];
	char monitor_setting_id[128];
	char monitor_edid_identity[64];
	char id[128];
	char alt_id[128];
	char monitor_name[64];
	enum display_capture_method method;
	bool reset_wgc;
	HMONITOR handle;
	bool capture_cursor;
	bool force_sdr;
	bool showing;
	LONG logged_width;
	LONG logged_height;

	long x;
	long y;
	int rot;
	uint32_t width;
	uint32_t height;
	gs_duplicator_t *duplicator;
	float reset_timeout;
	struct cursor_data cursor_data;

	void *winrt_module;
	struct winrt_exports exports;
	struct winrt_capture *capture_winrt;
};

struct duplicator_monitor {
	struct duplicator_monitor_info info;
	RECT rect;
	HMONITOR handle;
};

/* ------------------------------------------------------------------------- */

static const char *get_method_name(int method)
{
	const char *method_name = "";
	switch (method) {
	case METHOD_DXGI:
		method_name = "DXGI";
		break;
	case METHOD_WGC:
		method_name = "WGC";
		break;
	}

	return method_name;
}

static bool GetMonitorTarget(LPCWSTR device, DISPLAYCONFIG_TARGET_DEVICE_NAME *target)
{
	bool found = false;

	UINT32 numPath, numMode;
	if (GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &numPath, &numMode) == ERROR_SUCCESS) {
		if (!numPath || !numMode) {
			return false;
		}
		DISPLAYCONFIG_PATH_INFO *paths = bmalloc(numPath * sizeof(DISPLAYCONFIG_PATH_INFO));
		DISPLAYCONFIG_MODE_INFO *modes = bmalloc(numMode * sizeof(DISPLAYCONFIG_MODE_INFO));
		if (QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &numPath, paths, &numMode, modes, NULL) ==
		    ERROR_SUCCESS) {
			for (size_t i = 0; i < numPath; ++i) {
				const DISPLAYCONFIG_PATH_INFO *const path = &paths[i];

				DISPLAYCONFIG_SOURCE_DEVICE_NAME
				source;
				source.header.type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
				source.header.size = sizeof(source);
				source.header.adapterId = path->sourceInfo.adapterId;
				source.header.id = path->sourceInfo.id;
				if (DisplayConfigGetDeviceInfo(&source.header) == ERROR_SUCCESS &&
				    wcscmp(device, source.viewGdiDeviceName) == 0) {
					target->header.type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
					target->header.size = sizeof(*target);
					target->header.adapterId = path->sourceInfo.adapterId;
					target->header.id = path->targetInfo.id;
					found = DisplayConfigGetDeviceInfo(&target->header) == ERROR_SUCCESS;
					break;
				}
			}
		}

		bfree(modes);
		bfree(paths);
	}

	return found;
}

/* monitorDevicePath locates the monitor's hardware key; its instance text
 * describes the connector and must never be mistaken for an EDID serial. */
static void duplicator_read_edid(LPCWSTR path, char identity[64])
{
	identity[0] = 0;
	HDEVINFO devices = SetupDiCreateDeviceInfoList(NULL, NULL);
	if (devices == INVALID_HANDLE_VALUE)
		return;
	SP_DEVICE_INTERFACE_DATA interface_data = {0};
	interface_data.cbSize = sizeof(interface_data);
	if (SetupDiOpenDeviceInterfaceW(devices, path, 0, &interface_data)) {
		DWORD needed = 0;
		SetupDiGetDeviceInterfaceDetailW(devices, &interface_data, NULL, 0, &needed, NULL);
		if (needed >= sizeof(SP_DEVICE_INTERFACE_DETAIL_DATA_W)) {
			SP_DEVICE_INTERFACE_DETAIL_DATA_W *detail = bmalloc(needed);
			detail->cbSize = sizeof(*detail);
			SP_DEVINFO_DATA device = {0};
			device.cbSize = sizeof(device);
			if (SetupDiGetDeviceInterfaceDetailW(devices, &interface_data, detail, needed, NULL, &device)) {
				HKEY key = SetupDiOpenDevRegKey(devices, &device, DICS_FLAG_GLOBAL, 0, DIREG_DEV,
								KEY_READ);
				if (key != INVALID_HANDLE_VALUE) {
					BYTE edid[1024];
					DWORD size = sizeof(edid), type = 0;
					if (RegQueryValueExW(key, L"EDID", NULL, &type, edid, &size) == ERROR_SUCCESS &&
					    type == REG_BINARY)
						monitor_edid_identity(edid, size, identity);
					RegCloseKey(key);
				}
			}
			bfree(detail);
		}
	}
	SetupDiDestroyDeviceInfoList(devices);
}

static void GetMonitorDetails(HMONITOR handle, char *name, size_t count, char *identity)
{
	MONITORINFOEXW mi;
	DISPLAYCONFIG_TARGET_DEVICE_NAME target;

	if (identity)
		identity[0] = 0;
	mi.cbSize = sizeof(mi);
	if (GetMonitorInfoW(handle, (LPMONITORINFO)&mi) && GetMonitorTarget(mi.szDevice, &target)) {
		char *friendly_name;
		os_wcs_to_utf8_ptr(target.monitorFriendlyDeviceName, 0, &friendly_name);

		strcpy_s(name, count, friendly_name);
		bfree(friendly_name);
		if (identity)
			duplicator_read_edid(target.monitorDevicePath, identity);
	} else {
		strcpy_s(name, count, "[OBS: Unknown]");
	}
}

static void GetMonitorName(HMONITOR handle, char *name, size_t count)
{
	GetMonitorDetails(handle, name, count, NULL);
}

static BOOL CALLBACK enum_monitor(HMONITOR handle, HDC hdc, LPRECT rect, LPARAM param)
{
	UNUSED_PARAMETER(hdc);

	struct duplicator_monitor *monitor = (struct duplicator_monitor *)param;

	bool match = false;

	MONITORINFOEXA mi;
	mi.cbSize = sizeof(mi);
	if (GetMonitorInfoA(handle, (LPMONITORINFO)&mi)) {
		DISPLAY_DEVICEA device;
		device.cb = sizeof(device);
		if (EnumDisplayDevicesA(mi.szDevice, 0, &device, EDD_GET_DEVICE_INTERFACE_NAME)) {
			struct duplicator_monitor_info candidate = {0};
			monitor_fill_info(&candidate, device.DeviceID, "", "", "");
			match = monitor_matches(&candidate, monitor->info.device_id, "");
			if (match) {
				GetMonitorDetails(handle, candidate.name, _countof(candidate.name),
						  candidate.edid_identity);
				strcpy_s(candidate.alt_id, sizeof(candidate.alt_id), mi.szDevice);
				monitor->info = candidate;
				monitor->rect = *rect;
				monitor->handle = handle;
			}
		}
	}

	return !match;
}

struct duplicator_monitor_pass {
	DARRAY(struct duplicator_monitor_info) infos;
	DARRAY(struct duplicator_monitor) monitors;
	bool identity_complete;
};

static BOOL CALLBACK enum_monitor_fallback(HMONITOR handle, HDC hdc, LPRECT rect, LPARAM param)
{
	UNUSED_PARAMETER(hdc);
	struct duplicator_monitor_pass *pass = (struct duplicator_monitor_pass *)param;
	MONITORINFOEXA mi = {0};
	mi.cbSize = sizeof(mi);
	if (GetMonitorInfoA(handle, (LPMONITORINFO)&mi)) {
		DISPLAY_DEVICEA device = {0};
		device.cb = sizeof(device);
		EnumDisplayDevicesA(mi.szDevice, 0, &device, EDD_GET_DEVICE_INTERFACE_NAME);
		struct duplicator_monitor monitor = {0};
		monitor_fill_info(&monitor.info, device.DeviceID, mi.szDevice, "", "");
		GetMonitorDetails(handle, monitor.info.name, sizeof(monitor.info.name), monitor.info.edid_identity);
		monitor.rect = *rect;
		monitor.handle = handle;
		da_push_back(pass->infos, &monitor.info);
		da_push_back(pass->monitors, &monitor);
	} else {
		pass->identity_complete = false;
	}
	return TRUE;
}

static void log_settings(struct duplicator_capture *capture, const char *monitor, LONG width, LONG height)
{
	info("update settings:\n"
	     "\tdisplay: %s (%ldx%ld)\n"
	     "\tcursor: %s\n"
	     "\tmethod: %s\n"
	     "\tid: %s\n"
	     "\talt_id: %s\n"
	     "\tsetting_id: %s\n"
	     "\tforce SDR: %s",
	     monitor, width, height, capture->capture_cursor ? "true" : "false", get_method_name(capture->method),
	     capture->id, capture->alt_id, capture->monitor_id, capture->force_sdr ? "true" : "false");
}

static enum display_capture_method duplicator_choose_method(enum display_capture_method method, bool wgc_supported,
							    HMONITOR monitor)
{
	if (!wgc_supported)
		method = METHOD_DXGI;

	if (method == METHOD_AUTO) {
		method = METHOD_DXGI;

		obs_enter_graphics();
		const int dxgi_index = gs_duplicator_get_monitor_index(monitor);
		obs_leave_graphics();

		if (dxgi_index == -1) {
			method = METHOD_WGC;
		} else {
			SYSTEM_POWER_STATUS status;
			if (GetSystemPowerStatus(&status) && status.BatteryFlag < 128) {
				obs_enter_graphics();
				const uint32_t count = gs_get_adapter_count();
				obs_leave_graphics();
				if (count >= 2)
					method = METHOD_WGC;
			}
		}
	}

	return method;
}

extern bool wgc_supported;

static struct duplicator_monitor find_monitor(const char *monitor_id, const char *edid_identity)
{
	struct duplicator_monitor monitor = {0};
	strcpy_s(monitor.info.device_id, _countof(monitor.info.device_id), monitor_id);
	EnumDisplayMonitors(NULL, NULL, &enum_monitor, (LPARAM)&monitor);
	if (monitor.handle == NULL) {
		struct duplicator_monitor_pass pass = {.identity_complete = true};
		/* A failed/incomplete enumeration cannot prove uniqueness. */
		if (EnumDisplayMonitors(NULL, NULL, &enum_monitor_fallback, (LPARAM)&pass)) {
			const size_t index = monitor_find(pass.infos.array, pass.infos.num, monitor_id,
							  pass.identity_complete ? edid_identity : "");
			if (index < pass.monitors.num) {
				monitor = pass.monitors.array[index];
				if (!monitor.info.device_id[0])
					strcpy_s(monitor.info.device_id, sizeof(monitor.info.device_id), monitor_id);
			}
		}
		da_free(pass.infos);
		da_free(pass.monitors);
	}

	return monitor;
}

static inline void duplicator_update_settings(struct duplicator_capture *capture, obs_data_t *settings)
{
	pthread_mutex_lock(&capture->update_mutex);

	const char *key = obs_data_get_string(settings, "monitor_id");
	const bool same_selection = strcmp(key, capture->monitor_setting_id) == 0;
	const char *identity = same_selection ? capture->monitor_edid_identity : "";
	struct duplicator_monitor monitor = find_monitor(same_selection ? capture->monitor_id : key, identity);
	if (!same_selection)
		capture->monitor_edid_identity[0] = 0;
	if (monitor.info.edid_identity[0])
		strcpy_s(capture->monitor_edid_identity, sizeof(capture->monitor_edid_identity),
			 monitor.info.edid_identity);
	strcpy_s(capture->monitor_setting_id, sizeof(capture->monitor_setting_id), key);

	capture->method =
		duplicator_choose_method((int)obs_data_get_int(settings, "method"), wgc_supported, monitor.handle);

	strcpy_s(capture->monitor_id, _countof(capture->monitor_id), monitor.info.device_id);
	strcpy_s(capture->id, _countof(capture->id), monitor.info.id);
	strcpy_s(capture->alt_id, _countof(capture->alt_id), monitor.info.alt_id);
	strcpy_s(capture->monitor_name, _countof(capture->monitor_name), monitor.info.name);
	capture->handle = monitor.handle;

	capture->capture_cursor = obs_data_get_bool(settings, "capture_cursor");
	capture->force_sdr = obs_data_get_bool(settings, "force_sdr");

	capture->logged_width = monitor.rect.right - monitor.rect.left;
	capture->logged_height = monitor.rect.bottom - monitor.rect.top;

	if (capture->duplicator) {
		obs_enter_graphics();

		gs_duplicator_destroy(capture->duplicator);
		capture->duplicator = NULL;

		obs_leave_graphics();
	}

	capture->width = 0;
	capture->height = 0;
	capture->x = 0;
	capture->y = 0;
	capture->rot = 0;
	capture->reset_timeout = RESET_INTERVAL_SEC;

	pthread_mutex_unlock(&capture->update_mutex);
}

/* ------------------------------------------------------------------------- */

static const char *duplicator_capture_getname(void *unused)
{
	UNUSED_PARAMETER(unused);
	return TEXT_MONITOR_CAPTURE;
}

static void duplicator_actual_destroy(void *data)
{
	struct duplicator_capture *capture = data;

	if (capture->capture_winrt) {
		capture->exports.winrt_capture_free(capture->capture_winrt);
		capture->capture_winrt = NULL;
	}

	obs_enter_graphics();

	if (capture->duplicator) {
		gs_duplicator_destroy(capture->duplicator);
		capture->duplicator = NULL;
	}

	cursor_data_free(&capture->cursor_data);

	obs_leave_graphics();

	if (capture->winrt_module) {
		os_dlclose(capture->winrt_module);
		capture->winrt_module = NULL;
	}

	pthread_mutex_destroy(&capture->update_mutex);

	bfree(capture);
}

static void duplicator_capture_destroy(void *data)
{
	obs_queue_task(OBS_TASK_GRAPHICS, duplicator_actual_destroy, data, false);
}

static void duplicator_capture_defaults(obs_data_t *settings)
{
	obs_data_set_default_int(settings, "method", METHOD_AUTO);
	obs_data_set_default_string(settings, "monitor_id", INVALID_DISPLAY);
	obs_data_set_default_int(settings, "monitor_wgc", 0);
	obs_data_set_default_bool(settings, "capture_cursor", true);
	obs_data_set_default_bool(settings, "force_sdr", false);
}

static void duplicator_capture_update(void *data, obs_data_t *settings)
{
	struct duplicator_capture *mc = data;
	duplicator_update_settings(mc, settings);
	log_settings(mc, mc->monitor_name, mc->logged_width, mc->logged_height);

	mc->reset_wgc = true;
}

#define WINRT_IMPORT(func)                                           \
	do {                                                         \
		exports->func = (PFN_##func)os_dlsym(module, #func); \
		if (!exports->func) {                                \
			success = false;                             \
			blog(LOG_ERROR,                              \
			     "Could not load function '%s' from "    \
			     "module '%s'",                          \
			     #func, module_name);                    \
		}                                                    \
	} while (false)

static bool load_winrt_imports(struct winrt_exports *exports, void *module, const char *module_name)
{
	bool success = true;

	WINRT_IMPORT(winrt_capture_supported);
	WINRT_IMPORT(winrt_capture_cursor_toggle_supported);
	WINRT_IMPORT(winrt_capture_init_monitor);
	WINRT_IMPORT(winrt_capture_free);
	WINRT_IMPORT(winrt_capture_active);
	WINRT_IMPORT(winrt_capture_get_color_space);
	WINRT_IMPORT(winrt_capture_render);
	WINRT_IMPORT(winrt_capture_width);
	WINRT_IMPORT(winrt_capture_height);

	return success;
}

extern bool graphics_uses_d3d11;

static void *duplicator_capture_create(obs_data_t *settings, obs_source_t *source)
{
	struct duplicator_capture *capture;

	capture = bzalloc(sizeof(struct duplicator_capture));
	capture->source = source;

	pthread_mutex_init(&capture->update_mutex, NULL);

	if (graphics_uses_d3d11) {
		static const char *const module = "libobs-winrt";
		capture->winrt_module = os_dlopen(module);
		if (capture->winrt_module) {
			load_winrt_imports(&capture->exports, capture->winrt_module, module);
		}
	}

	duplicator_update_settings(capture, settings);
	log_settings(capture, capture->monitor_name, capture->logged_width, capture->logged_height);

	return capture;
}

static void reset_capture_data(struct duplicator_capture *capture)
{
	struct gs_monitor_info monitor_info = {0};
	gs_texture_t *texture = gs_duplicator_get_texture(capture->duplicator);

	const int dxgi_index = gs_duplicator_get_monitor_index(capture->handle);
	gs_get_duplicator_monitor_info(dxgi_index, &monitor_info);
	if (texture) {
		capture->width = gs_texture_get_width(texture);
		capture->height = gs_texture_get_height(texture);
	} else {
		capture->width = 0;
		capture->height = 0;
	}
	capture->x = monitor_info.x;
	capture->y = monitor_info.y;
	capture->rot = monitor_info.rotation_degrees;
}

static void free_capture_data(struct duplicator_capture *capture)
{
	if (capture->capture_winrt) {
		capture->exports.winrt_capture_free(capture->capture_winrt);
		capture->capture_winrt = NULL;
	}

	if (capture->duplicator) {
		gs_duplicator_destroy(capture->duplicator);
		capture->duplicator = NULL;
	}

	cursor_data_free(&capture->cursor_data);

	capture->width = 0;
	capture->height = 0;
	capture->x = 0;
	capture->y = 0;
	capture->rot = 0;
	capture->reset_timeout = 0.0f;
}

static void update_monitor_handle(struct duplicator_capture *capture)
{
	struct duplicator_monitor monitor = find_monitor(capture->monitor_id, capture->monitor_edid_identity);
	capture->handle = monitor.handle;
	if (monitor.handle) {
		strcpy_s(capture->monitor_id, sizeof(capture->monitor_id), monitor.info.device_id);
		strcpy_s(capture->id, sizeof(capture->id), monitor.info.id);
		strcpy_s(capture->alt_id, sizeof(capture->alt_id), monitor.info.alt_id);
		strcpy_s(capture->monitor_name, sizeof(capture->monitor_name), monitor.info.name);
		if (monitor.info.edid_identity[0])
			strcpy_s(capture->monitor_edid_identity, sizeof(capture->monitor_edid_identity),
				 monitor.info.edid_identity);
	}
}

static void duplicator_capture_tick(void *data, float seconds)
{
	struct duplicator_capture *capture = data;

	/* completely shut down monitor capture if not in use, otherwise it can
	 * sometimes generate system lag when a game is in fullscreen mode */
	if (!obs_source_showing(capture->source)) {
		if (capture->showing) {
			obs_enter_graphics();
			free_capture_data(capture);
			obs_leave_graphics();

			capture->showing = false;
		}
		return;
	}

	/* always try to load the capture immediately when the source is first
	 * shown */
	if (!capture->showing) {
		capture->reset_timeout = RESET_INTERVAL_SEC;
	}

	obs_enter_graphics();

	if (capture->method == METHOD_WGC) {
		if (capture->reset_wgc && capture->capture_winrt) {
			capture->exports.winrt_capture_free(capture->capture_winrt);
			capture->capture_winrt = NULL;
			capture->reset_wgc = false;
			capture->reset_timeout = RESET_INTERVAL_SEC;
		}

		if (!capture->capture_winrt) {
			capture->reset_timeout += seconds;

			if (capture->reset_timeout >= RESET_INTERVAL_SEC) {
				if (!capture->handle)
					update_monitor_handle(capture);

				if (capture->handle) {
					capture->capture_winrt = capture->exports.winrt_capture_init_monitor(
						capture->capture_cursor, capture->handle, capture->force_sdr);
					if (!capture->capture_winrt) {
						update_monitor_handle(capture);

						if (capture->handle) {
							capture->capture_winrt =
								capture->exports.winrt_capture_init_monitor(
									capture->capture_cursor, capture->handle,
									capture->force_sdr);
						}
					}
				}

				capture->reset_timeout = 0.0f;
			}
		}
	} else {
		if (capture->capture_winrt) {
			capture->exports.winrt_capture_free(capture->capture_winrt);
			capture->capture_winrt = NULL;
		}

		if (!capture->duplicator) {
			capture->reset_timeout += seconds;

			if (capture->reset_timeout >= RESET_INTERVAL_SEC) {
				if (!capture->handle)
					update_monitor_handle(capture);

				if (capture->handle) {
					int dxgi_index = gs_duplicator_get_monitor_index(capture->handle);

					if (dxgi_index == -1) {
						update_monitor_handle(capture);

						if (capture->handle) {
							dxgi_index = gs_duplicator_get_monitor_index(capture->handle);
						}
					}

					if (dxgi_index != -1) {
						capture->duplicator = gs_duplicator_create(dxgi_index);
					}
				}

				capture->reset_timeout = 0.0f;
			}
		}

		if (capture->duplicator) {
			if (capture->capture_cursor)
				cursor_capture(&capture->cursor_data);

			if (!gs_duplicator_update_frame(capture->duplicator)) {
				free_capture_data(capture);

			} else if (capture->width == 0) {
				reset_capture_data(capture);
			}
		}
	}

	obs_leave_graphics();

	if (!capture->showing)
		capture->showing = true;
}

static uint32_t duplicator_capture_width(void *data)
{
	struct duplicator_capture *capture = data;
	return (capture->method == METHOD_WGC) ? capture->exports.winrt_capture_width(capture->capture_winrt)
					       : (capture->rot % 180 == 0 ? capture->width : capture->height);
}

static uint32_t duplicator_capture_height(void *data)
{
	struct duplicator_capture *capture = data;
	return (capture->method == METHOD_WGC) ? capture->exports.winrt_capture_height(capture->capture_winrt)
					       : (capture->rot % 180 == 0 ? capture->height : capture->width);
}

static void duplicator_draw_cursor(struct duplicator_capture *capture)
{
	cursor_draw(&capture->cursor_data, -capture->x, -capture->y,
		    capture->rot % 180 == 0 ? capture->width : capture->height,
		    capture->rot % 180 == 0 ? capture->height : capture->width);
}

static void duplicator_capture_render(void *data, gs_effect_t *unused)
{
	UNUSED_PARAMETER(unused);

	struct duplicator_capture *capture = data;

	if (capture->method == METHOD_WGC) {
		if (capture->capture_winrt) {
			if (capture->exports.winrt_capture_active(capture->capture_winrt)) {
				capture->exports.winrt_capture_render(capture->capture_winrt);
			} else {
				capture->exports.winrt_capture_free(capture->capture_winrt);
				capture->capture_winrt = NULL;
			}
		}
	} else {
		if (!capture->duplicator)
			return;

		gs_texture_t *const texture = gs_duplicator_get_texture(capture->duplicator);
		if (!texture)
			return;

		const bool previous = gs_framebuffer_srgb_enabled();
		gs_enable_framebuffer_srgb(true);
		gs_enable_blending(false);

		const int rot = capture->rot;
		if (rot != 0) {
			float x = 0.0f;
			float y = 0.0f;

			switch (rot) {
			case 90:
				x = (float)capture->height;
				break;
			case 180:
				x = (float)capture->width;
				y = (float)capture->height;
				break;
			case 270:
				y = (float)capture->width;
				break;
			}

			gs_matrix_push();
			gs_matrix_translate3f(x, y, 0.0f);
			gs_matrix_rotaa4f(0.0f, 0.0f, 1.0f, RAD((float)rot));
		}

		const char *tech_name = "Draw";
		float multiplier = 1.f;
		const enum gs_color_space current_space = gs_get_color_space();
		if (gs_duplicator_get_color_space(capture->duplicator) == GS_CS_709_SCRGB) {
			if (capture->force_sdr) {
				tech_name = "DrawMultiply";
				const float target_nits =
					(current_space == GS_CS_709_SCRGB) ? obs_get_video_sdr_white_level() : 80.f;
				multiplier = target_nits / gs_duplicator_get_sdr_white_level(capture->duplicator);
			} else {
				switch (current_space) {
				case GS_CS_SRGB:
				case GS_CS_SRGB_16F:
					tech_name = "DrawMultiplyTonemap";
					multiplier = 80.f / obs_get_video_sdr_white_level();
					break;
				case GS_CS_709_EXTENDED:
					tech_name = "DrawMultiply";
					multiplier = 80.f / obs_get_video_sdr_white_level();
				}
			}
		} else if (current_space == GS_CS_709_SCRGB) {
			tech_name = "DrawMultiply";
			multiplier = obs_get_video_sdr_white_level() / 80.f;
		}

		gs_effect_t *const opaque_effect = obs_get_base_effect(OBS_EFFECT_OPAQUE);
		gs_eparam_t *multiplier_param = gs_effect_get_param_by_name(opaque_effect, "multiplier");
		gs_effect_set_float(multiplier_param, multiplier);
		gs_eparam_t *image_param = gs_effect_get_param_by_name(opaque_effect, "image");
		gs_effect_set_texture_srgb(image_param, texture);

		while (gs_effect_loop(opaque_effect, tech_name)) {
			gs_draw_sprite(texture, 0, 0, 0);
		}

		if (rot != 0)
			gs_matrix_pop();

		gs_enable_blending(true);
		gs_enable_framebuffer_srgb(previous);

		if (capture->capture_cursor) {
			gs_effect_t *const default_effect = obs_get_base_effect(OBS_EFFECT_DEFAULT);

			while (gs_effect_loop(default_effect, "Draw")) {
				duplicator_draw_cursor(capture);
			}
		}
	}
}

static BOOL CALLBACK enum_monitor_props(HMONITOR handle, HDC hdc, LPRECT rect, LPARAM param)
{
	UNUSED_PARAMETER(hdc);
	UNUSED_PARAMETER(rect);

	char monitor_name[64];
	GetMonitorName(handle, monitor_name, sizeof(monitor_name));

	MONITORINFOEXA mi;
	mi.cbSize = sizeof(mi);
	if (GetMonitorInfoA(handle, (LPMONITORINFO)&mi)) {
		obs_property_t *monitor_list = (obs_property_t *)param;
		struct dstr monitor_desc = {0};
		dstr_printf(&monitor_desc, "%s: %dx%d @ %d,%d", monitor_name, mi.rcMonitor.right - mi.rcMonitor.left,
			    mi.rcMonitor.bottom - mi.rcMonitor.top, mi.rcMonitor.left, mi.rcMonitor.top);
		if (mi.dwFlags == MONITORINFOF_PRIMARY)
			dstr_catf(&monitor_desc, " (%s)", TEXT_PRIMARY_MONITOR);

		DISPLAY_DEVICEA device;
		device.cb = sizeof(device);
		if (EnumDisplayDevicesA(mi.szDevice, 0, &device, EDD_GET_DEVICE_INTERFACE_NAME)) {
			obs_property_list_add_string(monitor_list, monitor_desc.array, device.DeviceID);
		} else {
			blog(LOG_WARNING,
			     "[duplicator-monitor-capture] EnumDisplayDevices failed for monitor (%s), falling back to szDevice",
			     monitor_name);
			obs_property_list_add_string(monitor_list, monitor_desc.array, mi.szDevice);
		}

		dstr_free(&monitor_desc);
	} else {
		blog(LOG_WARNING, "[duplicator-monitor-capture] GetMonitorInfo failed for monitor: %s", monitor_name);
	}

	return TRUE;
}

static void update_settings_visibility(obs_properties_t *props, struct duplicator_capture *capture)
{
	pthread_mutex_lock(&capture->update_mutex);

	const enum display_capture_method method = capture->method;
	const bool dxgi_options = method == METHOD_DXGI;
	const bool wgc_options = method == METHOD_WGC;

	const bool wgc_cursor_toggle = wgc_options && capture->exports.winrt_capture_cursor_toggle_supported();

	obs_property_t *p = obs_properties_get(props, "cursor");
	obs_property_set_visible(p, dxgi_options || wgc_cursor_toggle);

	pthread_mutex_unlock(&capture->update_mutex);
}

static bool display_capture_method_changed(obs_properties_t *props, obs_property_t *p, obs_data_t *settings)
{
	UNUSED_PARAMETER(p);

	struct duplicator_capture *capture = obs_properties_get_param(props);
	if (!capture)
		return false;

	duplicator_update_settings(capture, settings);

	update_settings_visibility(props, capture);

	return true;
}

static obs_properties_t *duplicator_capture_properties(void *data)
{
	struct duplicator_capture *capture = data;

	obs_properties_t *props = obs_properties_create();
	obs_properties_set_param(props, capture, NULL);

	obs_property_t *p =
		obs_properties_add_list(props, "method", TEXT_METHOD, OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_INT);
	obs_property_list_add_int(p, TEXT_METHOD_AUTO, METHOD_AUTO);
	obs_property_list_add_int(p, TEXT_METHOD_WGC, METHOD_WGC);
	obs_property_list_add_int(p, TEXT_METHOD_DXGI, METHOD_DXGI);
	obs_property_list_item_disable(p, 1, !wgc_supported);
	obs_property_set_modified_callback(p, display_capture_method_changed);

	obs_property_t *monitors = obs_properties_add_list(props, "monitor_id", TEXT_MONITOR, OBS_COMBO_TYPE_LIST,
							   OBS_COMBO_FORMAT_STRING);

	if (capture && strcmp(capture->monitor_id, INVALID_DISPLAY) == 0) {
		obs_property_list_add_string(monitors, obs_module_text("SelectADisplay"), INVALID_DISPLAY);
		obs_property_list_item_disable(monitors, 0, true);
	}

	obs_properties_add_bool(props, "capture_cursor", TEXT_CAPTURE_CURSOR);
	obs_properties_add_bool(props, "force_sdr", TEXT_FORCE_SDR);

	EnumDisplayMonitors(NULL, NULL, enum_monitor_props, (LPARAM)monitors);

	return props;
}

enum gs_color_space duplicator_capture_get_color_space(void *data, size_t count,
						       const enum gs_color_space *preferred_spaces)
{
	enum gs_color_space capture_space = GS_CS_SRGB;

	struct duplicator_capture *capture = data;
	if (capture->method == METHOD_WGC) {
		if (capture->capture_winrt) {
			capture_space = capture->exports.winrt_capture_get_color_space(capture->capture_winrt);
		}
	} else if (capture->duplicator && !capture->force_sdr) {
		capture_space = gs_duplicator_get_color_space(capture->duplicator);
	}

	enum gs_color_space space = capture_space;
	for (size_t i = 0; i < count; ++i) {
		const enum gs_color_space preferred_space = preferred_spaces[i];
		space = preferred_space;
		if (preferred_space == capture_space)
			break;
	}

	return space;
}

struct obs_source_info duplicator_capture_info = {
	.id = "monitor_capture",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW | OBS_SOURCE_DO_NOT_DUPLICATE | OBS_SOURCE_SRGB,
	.get_name = duplicator_capture_getname,
	.create = duplicator_capture_create,
	.destroy = duplicator_capture_destroy,
	.video_render = duplicator_capture_render,
	.video_tick = duplicator_capture_tick,
	.update = duplicator_capture_update,
	.get_width = duplicator_capture_width,
	.get_height = duplicator_capture_height,
	.get_defaults = duplicator_capture_defaults,
	.get_properties = duplicator_capture_properties,
	.icon_type = OBS_ICON_TYPE_DESKTOP_CAPTURE,
	.video_get_color_space = duplicator_capture_get_color_space,
};

/* File-local macros: keep them out of the next source of a unity build. */
#undef TEXT_METHOD
#undef TEXT_METHOD_WGC
#undef do_log
#undef warn

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef info
#undef debug
#undef TEXT_MONITOR_CAPTURE
#undef TEXT_CAPTURE_CURSOR
#undef TEXT_COMPATIBILITY
#undef TEXT_MONITOR
#undef TEXT_PRIMARY_MONITOR
#undef TEXT_METHOD_AUTO
#undef TEXT_METHOD_DXGI
#undef TEXT_FORCE_SDR
#undef RESET_INTERVAL_SEC
#undef INVALID_DISPLAY
#undef WINRT_IMPORT
