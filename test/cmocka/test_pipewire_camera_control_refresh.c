#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <cmocka.h>

#include <gio/gio.h>
#include <gio/gunixfdlist.h>
#include <X11/Xlib.h>
#include <linux/videodev2.h>

#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>
#include <spa/param/props.h>

#include <obs.h>
#include <obs-nix-platform.h>
#include <util/platform.h>
#include <util/threading.h>

#include "camera-portal.h"

/* Camera controls in the Properties window of the "Video Capture Device
 * (PipeWire)" source (obs-rust/obs-studio#827, reported by alerque).
 *
 * Sliders could not be dragged: the handle "dropped" after one step, and the
 * mouse wheel moved it by one step only. Every camera control's modified
 * callback, control_changed() in camera-portal.c, returned true, which asks
 * the Properties window to rebuild all its widgets, so the first value change
 * of a drag destroyed the slider that held the mouse. Changing a control's
 * value must not rebuild the window.
 *
 * A manual control stayed enabled while its automatic mode was on: with
 * "Focus, Automatic Continuous" ticked, "Focus, Absolute" could be dragged and
 * did nothing. The V4L2 driver flags such a control inactive
 * (V4L2_CTRL_FLAG_INACTIVE) and ignores or refuses it, but PipeWire's V4L2
 * node does not pass the flag on: its PropInfo params only carry the id, the
 * name and the range. The automatic/manual pairs are the ones the V4L2 spec
 * defines (and uvcvideo implements): Auto Exposure / Exposure Time, Absolute
 * (manual in Manual Mode); White Balance, Automatic / White Balance
 * Temperature, Red Balance and Blue Balance; Hue, Automatic / Hue; Focus,
 * Automatic Continuous / Focus, Absolute. Those manual controls must be shown
 * disabled while the automatic one, as the window shows it, is on, whatever
 * the order the camera lists them in (uvcvideo lists Focus, Absolute before
 * Focus, Automatic Continuous, and Hue before Hue, Automatic).
 *
 * Menu controls (Auto Exposure, Power Line Frequency) were missing from the
 * window: PipeWire's V4L2 node publishes a menu as a plain Int with labels,
 * not as a choice, and the source skipped it, so Auto Exposure could not be
 * switched to Manual Mode at all.
 *
 * End to end: the real camera portal code against a fake
 * org.freedesktop.portal.Camera on a private D-Bus bus (GTestDBus), with its
 * PipeWire connection on a private PipeWire daemon that
 * test_pipewire_cursor_hdr.sh starts (own runtime dir, no session manager, no
 * devices). Two stream nodes in this test play two cameras the way
 * PipeWire's V4L2 node publishes them (PropInfo and Props params, the same
 * prop ids, and the "device.api" and "object.path" node properties), and
 * record the Props the source sets on them:
 * - the "UHD Camera" of #827, with the exact control list its driver reports
 *   (uvcvideo, kernel 7.2.9), including Iris, Absolute with a step of 0;
 * - a webcam with White Balance, Automatic and Hue, Automatic, which starts
 *   in Aperture Priority Mode.
 *
 * Needs dbus-daemon and an X display for obs_startup() (xvfb-run is enough);
 * the wrapper skips (77) without PipeWire, and the cases skip when
 * obs_startup() or obs_reset_video() fails. */

#define SKIP_EXIT 77

#define WIDTH 64
#define HEIGHT 48

/* The names V4L2 gives the controls (v4l2_ctrl_get_name()) */
#define BRIGHTNESS "Brightness"
#define HUE "Hue"
#define HUE_AUTO "Hue, Automatic"
#define POWER_LINE "Power Line Frequency"
#define AUTO_EXPOSURE "Auto Exposure"
#define EXPOSURE "Exposure Time, Absolute"
#define FOCUS "Focus, Absolute"
#define FOCUS_AUTO "Focus, Automatic Continuous"
#define WB_AUTO "White Balance, Automatic"
#define WB_TEMPERATURE "White Balance Temperature"
#define RED_BALANCE "Red Balance"
#define BLUE_BALANCE "Blue Balance"
#define IRIS "Iris, Absolute"
#define PAN_SPEED "Pan, Speed"
#define TILT_SPEED "Tilt, Speed"

const char *obs_module_text(const char *val)
{
	return val;
}

/* ------------------------------------------------------------------ */
/* Fake org.freedesktop.portal.Camera, served from its own thread      */

static const char portal_xml[] = "<node>"
				 " <interface name='org.freedesktop.portal.Camera'>"
				 "  <method name='AccessCamera'>"
				 "   <arg type='a{sv}' direction='in'/><arg type='o' direction='out'/>"
				 "  </method>"
				 "  <method name='OpenPipeWireRemote'>"
				 "   <arg type='a{sv}' direction='in'/><arg type='h' direction='out'/>"
				 "  </method>"
				 "  <property name='IsCameraPresent' type='b' access='read'/>"
				 "  <property name='version' type='u' access='read'/>"
				 " </interface>"
				 "</node>";

struct portal {
	GMainContext *context;
	GMainLoop *loop;
	GDBusConnection *connection;
	const char *address;
	os_event_t *ready;
	pthread_t thread;
};

static char *sender_path_part(const char *sender)
{
	char *part = g_strdup(sender + 1);

	g_strdelimit(part, ".", '_');
	return part;
}

static int connect_pipewire_socket(void)
{
	const char *dir = getenv("PIPEWIRE_RUNTIME_DIR");
	struct sockaddr_un addr = {.sun_family = AF_UNIX};
	int fd;

	if (!dir)
		return -1;
	snprintf(addr.sun_path, sizeof(addr.sun_path), "%s/pipewire-0", dir);
	fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
	if (fd < 0)
		return -1;
	if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
		close(fd);
		return -1;
	}
	return fd;
}

static void portal_method_call(GDBusConnection *connection, const char *sender, const char *object_path,
			       const char *interface_name, const char *method_name, GVariant *parameters,
			       GDBusMethodInvocation *invocation, gpointer user_data)
{
	g_autoptr(GVariant) options = NULL;
	g_autofree char *sender_part = sender_path_part(sender);
	g_autofree char *request_path = NULL;
	const char *handle_token = NULL;
	GVariantBuilder results;

	UNUSED_PARAMETER(object_path);
	UNUSED_PARAMETER(interface_name);
	UNUSED_PARAMETER(user_data);

	if (strcmp(method_name, "OpenPipeWireRemote") == 0) {
		g_autoptr(GUnixFDList) fd_list = g_unix_fd_list_new();
		int fd = connect_pipewire_socket();
		int index = fd >= 0 ? g_unix_fd_list_append(fd_list, fd, NULL) : -1;

		if (fd >= 0)
			close(fd);
		if (index < 0) {
			g_dbus_method_invocation_return_dbus_error(invocation, "org.freedesktop.portal.Error.Failed",
								   "no PipeWire socket");
			return;
		}
		g_dbus_method_invocation_return_value_with_unix_fd_list(invocation, g_variant_new("(h)", index),
									fd_list);
		return;
	}

	/* AccessCamera: granted at once. */
	options = g_variant_get_child_value(parameters, 0);
	g_variant_lookup(options, "handle_token", "&s", &handle_token);
	request_path = g_strdup_printf("/org/freedesktop/portal/desktop/request/%s/%s", sender_part, handle_token);
	g_dbus_method_invocation_return_value(invocation, g_variant_new("(o)", request_path));

	g_variant_builder_init(&results, G_VARIANT_TYPE_VARDICT);
	g_dbus_connection_emit_signal(connection, sender, request_path, "org.freedesktop.portal.Request", "Response",
				      g_variant_new("(u@a{sv})", 0, g_variant_builder_end(&results)), NULL);
}

static GVariant *portal_get_property(GDBusConnection *connection, const char *sender, const char *object_path,
				     const char *interface_name, const char *property_name, GError **error,
				     gpointer user_data)
{
	UNUSED_PARAMETER(connection);
	UNUSED_PARAMETER(sender);
	UNUSED_PARAMETER(object_path);
	UNUSED_PARAMETER(interface_name);
	UNUSED_PARAMETER(error);
	UNUSED_PARAMETER(user_data);

	if (strcmp(property_name, "IsCameraPresent") == 0)
		return g_variant_new_boolean(TRUE);
	return g_variant_new_uint32(1);
}

static const GDBusInterfaceVTable portal_vtable = {
	.method_call = portal_method_call,
	.get_property = portal_get_property,
};

static void *portal_thread(void *data)
{
	struct portal *portal = data;
	g_autoptr(GDBusNodeInfo) node = NULL;
	g_autoptr(GVariant) reply = NULL;

	g_main_context_push_thread_default(portal->context);

	portal->connection = g_dbus_connection_new_for_address_sync(
		portal->address,
		G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT | G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION, NULL,
		NULL, NULL);
	node = g_dbus_node_info_new_for_xml(portal_xml, NULL);
	g_dbus_connection_register_object(portal->connection, "/org/freedesktop/portal/desktop", node->interfaces[0],
					  &portal_vtable, NULL, NULL, NULL);
	reply = g_dbus_connection_call_sync(portal->connection, "org.freedesktop.DBus", "/org/freedesktop/DBus",
					    "org.freedesktop.DBus", "RequestName",
					    g_variant_new("(su)", "org.freedesktop.portal.Desktop", 0), NULL,
					    G_DBUS_CALL_FLAGS_NONE, -1, NULL, NULL);
	os_event_signal(portal->ready);

	g_main_loop_run(portal->loop);

	g_main_context_pop_thread_default(portal->context);
	return NULL;
}

/* ------------------------------------------------------------------ */
/* The cameras: video source nodes with the controls of a V4L2 camera */

enum control_kind { CONTROL_INT, CONTROL_BOOL, CONTROL_MENU };

struct menu_item {
	int32_t value;
	const char *label;
};

struct test_control {
	uint32_t prop_id;
	const char *name;
	enum control_kind kind;
	int32_t def, min, max, step;
	const struct menu_item *menu;
	uint32_t n_menu;
};

/* PipeWire's V4L2 node gives a few controls a standard prop id, and every
 * other one SPA_PROP_START_CUSTOM + its V4L2 control id (control_to_prop_id()
 * in spa/plugins/v4l2/v4l2-utils.c). */
#define V4L2_PROP(cid) (SPA_PROP_START_CUSTOM + (cid))
#define INT_CONTROL(id, name, def, min, max, step) {(id), (name), CONTROL_INT, (def), (min), (max), (step), NULL, 0}
#define BOOL_CONTROL(id, name, def) {(id), (name), CONTROL_BOOL, (def), 0, 1, 1, NULL, 0}
#define MENU_CONTROL(id, name, def, items) \
	{(id), (name), CONTROL_MENU, (def), 0, 0, 0, (items), sizeof(items) / sizeof((items)[0])}

static const struct menu_item power_line_menu[] = {{0, "Disabled"}, {1, "50 Hz"}, {2, "60 Hz"}};
static const struct menu_item uhd_exposure_menu[] = {{0, "Auto Mode"}, {1, "Manual Mode"}};
static const struct menu_item webcam_exposure_menu[] = {{1, "Manual Mode"}, {3, "Aperture Priority Mode"}};

/* The "UHD Camera" of #827, as `v4l2-ctl --list-ctrls` shows it with auto
 * focus on (uvcvideo, kernel 7.2.9), in the driver's order. */
static const struct test_control uhd_controls[] = {
	INT_CONTROL(SPA_PROP_brightness, BRIGHTNESS, 7, 0, 14, 1),
	INT_CONTROL(SPA_PROP_contrast, "Contrast", 7, 0, 14, 1),
	INT_CONTROL(SPA_PROP_saturation, "Saturation", 100, 60, 200, 10),
	INT_CONTROL(SPA_PROP_hue, HUE, 7, 0, 14, 1),
	INT_CONTROL(SPA_PROP_gain, "Gain", 2, 0, 7, 1),
	MENU_CONTROL(V4L2_PROP(V4L2_CID_POWER_LINE_FREQUENCY), POWER_LINE, 1, power_line_menu),
	INT_CONTROL(SPA_PROP_sharpness, "Sharpness", 6, 0, 11, 1),
	INT_CONTROL(V4L2_PROP(V4L2_CID_BACKLIGHT_COMPENSATION), "Backlight Compensation", 0, 0, 1, 1),
	MENU_CONTROL(V4L2_PROP(V4L2_CID_EXPOSURE_AUTO), AUTO_EXPOSURE, V4L2_EXPOSURE_AUTO, uhd_exposure_menu),
	INT_CONTROL(SPA_PROP_exposure, EXPOSURE, 80, 1, 333, 1),
	INT_CONTROL(V4L2_PROP(V4L2_CID_PAN_ABSOLUTE), "Pan, Absolute", 0, -612000, 612000, 1),
	INT_CONTROL(V4L2_PROP(V4L2_CID_TILT_ABSOLUTE), "Tilt, Absolute", 0, -108000, 324000, 1),
	INT_CONTROL(V4L2_PROP(V4L2_CID_FOCUS_ABSOLUTE), FOCUS, 1675, 0, 4148, 1),
	BOOL_CONTROL(V4L2_PROP(V4L2_CID_FOCUS_AUTO), FOCUS_AUTO, 1),
	INT_CONTROL(V4L2_PROP(V4L2_CID_ZOOM_ABSOLUTE), "Zoom, Absolute", 0, 0, 4912, 1),
	INT_CONTROL(V4L2_PROP(V4L2_CID_ZOOM_CONTINUOUS), "Zoom, Continuous", 4, -7, 7, 1),
	INT_CONTROL(V4L2_PROP(V4L2_CID_IRIS_ABSOLUTE), IRIS, 180, 180, 1200, 0),
	INT_CONTROL(V4L2_PROP(V4L2_CID_PAN_SPEED), PAN_SPEED, 16, -24, 24, 1),
	INT_CONTROL(V4L2_PROP(V4L2_CID_TILT_SPEED), TILT_SPEED, 16, -20, 20, 1),
};

/* A webcam with automatic white balance and hue, starting in Aperture
 * Priority Mode. Hue, Automatic comes after Hue; White Balance, Automatic
 * before its manual controls. */
static const struct test_control webcam_controls[] = {
	INT_CONTROL(SPA_PROP_brightness, BRIGHTNESS, 128, 0, 255, 1),
	INT_CONTROL(SPA_PROP_hue, HUE, 0, -180, 180, 1),
	BOOL_CONTROL(V4L2_PROP(V4L2_CID_AUTO_WHITE_BALANCE), WB_AUTO, 1),
	INT_CONTROL(V4L2_PROP(V4L2_CID_RED_BALANCE), RED_BALANCE, 128, 0, 255, 1),
	INT_CONTROL(V4L2_PROP(V4L2_CID_BLUE_BALANCE), BLUE_BALANCE, 128, 0, 255, 1),
	BOOL_CONTROL(V4L2_PROP(V4L2_CID_HUE_AUTO), HUE_AUTO, 1),
	INT_CONTROL(V4L2_PROP(V4L2_CID_WHITE_BALANCE_TEMPERATURE), WB_TEMPERATURE, 4000, 2800, 6500, 10),
	MENU_CONTROL(V4L2_PROP(V4L2_CID_EXPOSURE_AUTO), AUTO_EXPOSURE, V4L2_EXPOSURE_APERTURE_PRIORITY,
		     webcam_exposure_menu),
	INT_CONTROL(SPA_PROP_exposure, EXPOSURE, 250, 3, 2047, 1),
	INT_CONTROL(V4L2_PROP(V4L2_CID_FOCUS_ABSOLUTE), FOCUS, 0, 0, 250, 5),
	BOOL_CONTROL(V4L2_PROP(V4L2_CID_FOCUS_AUTO), FOCUS_AUTO, 1),
};

#define MAX_CONTROLS 32

struct test_camera {
	const char *node_name;
	const char *description;
	const char *device;
	const struct test_control *controls;
	uint32_t n_controls;

	struct pw_stream *stream;
	struct spa_hook stream_listener;
	uint32_t node;

	/* Written on the camera's PipeWire thread, under camera_mutex. */
	int props_set;               /* Props params received */
	int32_t value[MAX_CONTROLS]; /* each control's value */
	int sets[MAX_CONTROLS];      /* how many times a Props param set it */
};

enum { UHD_CAMERA, WEBCAM, CAMERA_COUNT };

static struct test_camera cameras[CAMERA_COUNT] = {
	[UHD_CAMERA] = {"obs-test-uhd-camera", "UHD Camera (V4L2)", "/dev/video2", uhd_controls,
			sizeof(uhd_controls) / sizeof(uhd_controls[0])},
	[WEBCAM] = {"obs-test-webcam", "Webcam (V4L2)", "/dev/video0", webcam_controls,
		    sizeof(webcam_controls) / sizeof(webcam_controls[0])},
};

static pthread_mutex_t camera_mutex = PTHREAD_MUTEX_INITIALIZER;

static struct {
	struct pw_thread_loop *loop;
	struct pw_context *context;
	struct pw_core *core;
} pw;

static const struct spa_pod *control_info(struct spa_pod_builder *b, const struct test_control *c)
{
	struct spa_pod_frame f[2];

	switch (c->kind) {
	case CONTROL_INT:
		return spa_pod_builder_add_object(b, SPA_TYPE_OBJECT_PropInfo, SPA_PARAM_PropInfo, SPA_PROP_INFO_id,
						  SPA_POD_Id(c->prop_id), SPA_PROP_INFO_type,
						  SPA_POD_CHOICE_STEP_Int(c->def, c->min, c->max, c->step),
						  SPA_PROP_INFO_description, SPA_POD_String(c->name));
	case CONTROL_BOOL:
		return spa_pod_builder_add_object(b, SPA_TYPE_OBJECT_PropInfo, SPA_PARAM_PropInfo, SPA_PROP_INFO_id,
						  SPA_POD_Id(c->prop_id), SPA_PROP_INFO_type,
						  SPA_POD_CHOICE_Bool(c->def != 0), SPA_PROP_INFO_description,
						  SPA_POD_String(c->name));
	case CONTROL_MENU:
		/* A plain Int with labels, as spa_v4l2_enum_controls() does */
		spa_pod_builder_push_object(b, &f[0], SPA_TYPE_OBJECT_PropInfo, SPA_PARAM_PropInfo);
		spa_pod_builder_add(b, SPA_PROP_INFO_id, SPA_POD_Id(c->prop_id), SPA_PROP_INFO_type,
				    SPA_POD_Int(c->def), SPA_PROP_INFO_description, SPA_POD_String(c->name), 0);
		spa_pod_builder_prop(b, SPA_PROP_INFO_labels, 0);
		spa_pod_builder_push_struct(b, &f[1]);
		for (uint32_t i = 0; i < c->n_menu; i++) {
			spa_pod_builder_int(b, c->menu[i].value);
			spa_pod_builder_string(b, c->menu[i].label);
		}
		spa_pod_builder_pop(b, &f[1]);
		return spa_pod_builder_pop(b, &f[0]);
	}
	return NULL;
}

/* The params of PipeWire's V4L2 node: the format, PropInfo for the device
 * path, name and fd, then one per control, and Props with their values. */
static uint32_t camera_params(struct spa_pod_builder *b, const struct spa_pod **params, struct test_camera *camera)
{
	struct spa_pod_frame f;
	uint32_t n = 0;

	params[n++] = spa_pod_builder_add_object(b, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat, SPA_FORMAT_mediaType,
						 SPA_POD_Id(SPA_MEDIA_TYPE_video), SPA_FORMAT_mediaSubtype,
						 SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw), SPA_FORMAT_VIDEO_format,
						 SPA_POD_Id(SPA_VIDEO_FORMAT_YUY2), SPA_FORMAT_VIDEO_size,
						 SPA_POD_Rectangle(&SPA_RECTANGLE(WIDTH, HEIGHT)),
						 SPA_FORMAT_VIDEO_framerate, SPA_POD_Fraction(&SPA_FRACTION(30, 1)));
	params[n++] = spa_pod_builder_add_object(b, SPA_TYPE_OBJECT_PropInfo, SPA_PARAM_PropInfo, SPA_PROP_INFO_id,
						 SPA_POD_Id(SPA_PROP_device), SPA_PROP_INFO_description,
						 SPA_POD_String("The V4L2 device"), SPA_PROP_INFO_type,
						 SPA_POD_String(camera->device));
	params[n++] = spa_pod_builder_add_object(b, SPA_TYPE_OBJECT_PropInfo, SPA_PARAM_PropInfo, SPA_PROP_INFO_id,
						 SPA_POD_Id(SPA_PROP_deviceName), SPA_PROP_INFO_description,
						 SPA_POD_String("The V4L2 device name"), SPA_PROP_INFO_type,
						 SPA_POD_String(camera->description));
	params[n++] = spa_pod_builder_add_object(b, SPA_TYPE_OBJECT_PropInfo, SPA_PARAM_PropInfo, SPA_PROP_INFO_id,
						 SPA_POD_Id(SPA_PROP_deviceFd), SPA_PROP_INFO_description,
						 SPA_POD_String("The V4L2 fd"), SPA_PROP_INFO_type, SPA_POD_Int(-1));
	for (uint32_t i = 0; i < camera->n_controls; i++)
		params[n++] = control_info(b, &camera->controls[i]);

	spa_pod_builder_push_object(b, &f, SPA_TYPE_OBJECT_Props, SPA_PARAM_Props);
	spa_pod_builder_add(b, SPA_PROP_device, SPA_POD_String(camera->device), SPA_PROP_deviceName,
			    SPA_POD_String(camera->description), SPA_PROP_deviceFd, SPA_POD_Int(-1), 0);
	for (uint32_t i = 0; i < camera->n_controls; i++) {
		spa_pod_builder_prop(b, camera->controls[i].prop_id, 0);
		if (camera->controls[i].kind == CONTROL_BOOL)
			spa_pod_builder_bool(b, camera->controls[i].def != 0);
		else
			spa_pod_builder_int(b, camera->controls[i].def);
	}
	params[n++] = spa_pod_builder_pop(b, &f);
	return n;
}

static int find_control(const struct test_camera *camera, const char *name)
{
	for (uint32_t i = 0; i < camera->n_controls; i++) {
		if (strcmp(camera->controls[i].name, name) == 0)
			return (int)i;
	}
	return -1;
}

static void on_camera_param_changed(void *data, uint32_t id, const struct spa_pod *param)
{
	struct test_camera *camera = data;
	struct spa_pod_object *obj = (struct spa_pod_object *)(uintptr_t)param;
	struct spa_pod_prop *prop;

	if (!param || id != SPA_PARAM_Props || !spa_pod_is_object(param))
		return;

	pthread_mutex_lock(&camera_mutex);
	camera->props_set++;
	SPA_POD_OBJECT_FOREACH(obj, prop)
	{
		for (uint32_t i = 0; i < camera->n_controls; i++) {
			int32_t value;
			bool b;

			if (camera->controls[i].prop_id != prop->key)
				continue;
			if (spa_pod_get_int(&prop->value, &value) == 0) {
				camera->value[i] = value;
				camera->sets[i]++;
			} else if (spa_pod_get_bool(&prop->value, &b) == 0) {
				camera->value[i] = b;
				camera->sets[i]++;
			}
		}
	}
	pthread_mutex_unlock(&camera_mutex);
}

static const struct pw_stream_events camera_events = {
	PW_VERSION_STREAM_EVENTS,
	.param_changed = on_camera_param_changed,
};

static bool camera_connect(struct test_camera *camera)
{
	uint8_t buffer[16384];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[MAX_CONTROLS + 8];
	uint32_t node_id = SPA_ID_INVALID;
	uint32_t n_params;
	char object_path[64];

	for (uint32_t i = 0; i < camera->n_controls; i++)
		camera->value[i] = camera->controls[i].def;
	n_params = camera_params(&b, params, camera);
	snprintf(object_path, sizeof(object_path), "v4l2:%s", camera->device);

	pw_thread_loop_lock(pw.loop);
	camera->stream = pw_stream_new(pw.core, camera->node_name,
				       pw_properties_new(PW_KEY_NODE_NAME, camera->node_name, PW_KEY_NODE_DESCRIPTION,
							 camera->description, PW_KEY_MEDIA_CLASS, "Video/Source",
							 PW_KEY_MEDIA_ROLE, "Camera", PW_KEY_DEVICE_API, "v4l2",
							 PW_KEY_OBJECT_PATH, object_path, NULL));
	pw_stream_add_listener(camera->stream, &camera->stream_listener, &camera_events, camera);
	pw_stream_connect(camera->stream, PW_DIRECTION_OUTPUT, PW_ID_ANY,
			  PW_STREAM_FLAG_DRIVER | PW_STREAM_FLAG_MAP_BUFFERS, params, n_params);
	pw_thread_loop_unlock(pw.loop);

	for (int i = 0; i < 200 && node_id == SPA_ID_INVALID; i++) {
		os_sleep_ms(10);
		pw_thread_loop_lock(pw.loop);
		node_id = pw_stream_get_node_id(camera->stream);
		pw_thread_loop_unlock(pw.loop);
	}
	camera->node = node_id;
	return node_id != SPA_ID_INVALID;
}

static bool cameras_start(void)
{
	pw_init(NULL, NULL);

	pw.loop = pw_thread_loop_new("camera-control-test", NULL);
	pw.context = pw_context_new(pw_thread_loop_get_loop(pw.loop), NULL, 0);
	if (pw_thread_loop_start(pw.loop) < 0)
		return false;

	pw_thread_loop_lock(pw.loop);
	pw.core = pw_context_connect(pw.context, NULL, 0);
	pw_thread_loop_unlock(pw.loop);
	if (!pw.core)
		return false;

	for (int i = 0; i < CAMERA_COUNT; i++) {
		if (!camera_connect(&cameras[i]))
			return false;
	}
	return true;
}

static void cameras_stop(void)
{
	if (!pw.loop)
		return;
	pw_thread_loop_lock(pw.loop);
	for (int i = 0; i < CAMERA_COUNT; i++)
		g_clear_pointer(&cameras[i].stream, pw_stream_destroy);
	g_clear_pointer(&pw.core, pw_core_disconnect);
	pw_thread_loop_unlock(pw.loop);
	pw_thread_loop_stop(pw.loop);
	pw_context_destroy(pw.context);
	pw_thread_loop_destroy(pw.loop);
	memset(&pw, 0, sizeof(pw));
}

/* A control's value on the camera, and how many times it was set */
static int32_t camera_value(struct test_camera *camera, const char *name, int *sets)
{
	int i = find_control(camera, name);
	int32_t value;

	assert_true(i >= 0);
	pthread_mutex_lock(&camera_mutex);
	value = camera->value[i];
	if (sets)
		*sets = camera->sets[i];
	pthread_mutex_unlock(&camera_mutex);
	return value;
}

/* ------------------------------------------------------------------ */
/* The camera sources                                                 */

static void log_handler(int level, const char *format, va_list args, void *param)
{
	UNUSED_PARAMETER(param);

	if (level <= LOG_WARNING || getenv("CAMERA_CONTROL_TEST_VERBOSE")) {
		vfprintf(stderr, format, args);
		fprintf(stderr, "\n");
	}
}

static void iterate_main_loop(uint32_t ms)
{
	uint64_t end = os_gettime_ns() + ms * 1000000ULL;

	while (os_gettime_ns() < end) {
		while (g_main_context_iteration(NULL, FALSE))
			;
		os_sleep_ms(1);
	}
}

static obs_source_t *sources[CAMERA_COUNT];

static obs_source_t *create_camera_source(const struct test_camera *camera)
{
	obs_data_t *settings = obs_data_create();
	obs_source_t *source;

	obs_data_set_string(settings, "device_id", camera->node_name);
	source = obs_source_create("pipewire-camera-source", camera->node_name, settings, NULL);
	obs_data_release(settings);
	return source;
}

/* The source's properties, as the Properties window gets them when it opens,
 * once the camera and its last control are known. Fails if they never show
 * up. */
static obs_properties_t *open_properties(int cam)
{
	const struct test_camera *camera = &cameras[cam];
	const char *last = camera->controls[camera->n_controls - 1].name;

	assert_non_null(sources[cam]);
	for (int i = 0; i < 250; i++) {
		obs_properties_t *props = obs_source_properties(sources[cam]);

		if (obs_properties_get(props, last))
			return props;
		obs_properties_destroy(props);
		iterate_main_loop(20);
	}
	fprintf(stderr, "the controls of '%s' never showed up in the properties\n", camera->description);
	fail();
	return NULL;
}

static obs_property_t *control_property(obs_properties_t *props, const char *name)
{
	obs_property_t *prop = obs_properties_get(props, name);

	if (!prop) {
		fprintf(stderr, "no '%s' in the Camera Controls group\n", name);
		fail();
	}
	return prop;
}

static bool control_enabled(obs_properties_t *props, const char *name)
{
	return obs_property_enabled(control_property(props, name));
}

/* What the Properties window does for one step of a drag, a click on a
 * checkbox or a pick in a list: set the value and run the modified callback.
 * True means "rebuild the window". */
static bool change_in_properties(int cam, obs_properties_t *props, const char *name, long long value)
{
	obs_property_t *prop = control_property(props, name);
	obs_data_t *settings = obs_source_get_settings(sources[cam]);
	bool rebuild;

	if (obs_property_get_type(prop) == OBS_PROPERTY_BOOL)
		obs_data_set_bool(settings, name, value != 0);
	else
		obs_data_set_int(settings, name, value);
	rebuild = obs_property_modified(prop, settings);
	obs_data_release(settings);
	return rebuild;
}

/* A setting saved by an earlier session */
static void set_saved(int cam, const char *name, long long value, bool is_bool)
{
	obs_data_t *settings = obs_source_get_settings(sources[cam]);

	if (is_bool)
		obs_data_set_bool(settings, name, value != 0);
	else
		obs_data_set_int(settings, name, value);
	obs_data_release(settings);
}

/* Waits for the camera to get `expected` for a control; returns its value */
static int32_t wait_for_camera_value(int cam, const char *name, int32_t expected)
{
	int32_t value = camera_value(&cameras[cam], name, NULL);

	for (int i = 0; i < 250 && value != expected; i++) {
		iterate_main_loop(20);
		value = camera_value(&cameras[cam], name, NULL);
	}
	return value;
}

/* The checkbox or list `master` goes to `value`: the window must be rebuilt,
 * and each of the `slaves` must then be enabled or disabled. */
static void check_switch(int cam, obs_properties_t *props, const char *master, long long value, bool slaves_enabled,
			 const char *const *slaves, size_t n_slaves)
{
	bool rebuild = change_in_properties(cam, props, master, value);
	bool ok = rebuild;

	for (size_t i = 0; i < n_slaves; i++)
		ok = ok && control_enabled(props, slaves[i]) == slaves_enabled;
	if (!ok) {
		fprintf(stderr, "'%s' set to %lld: rebuild %d (expected 1)", master, value, rebuild);
		for (size_t i = 0; i < n_slaves; i++)
			fprintf(stderr, ", '%s' enabled %d", slaves[i], control_enabled(props, slaves[i]));
		fprintf(stderr, " (expected %d)\n", slaves_enabled);
		fail();
	}
	int32_t on_camera = wait_for_camera_value(cam, master, (int32_t)value);
	if (on_camera != value) {
		fprintf(stderr, "'%s' is %d on the camera, expected %lld\n", master, on_camera, value);
		fail();
	}
}

/* ------------------------------------------------------------------ */
/* Fixture                                                            */

static bool obs_up;
static GTestDBus *bus;
static struct portal portal;
static Display *display;

static int setup(void **state)
{
	g_autofree char *display_name = g_strdup(getenv("DISPLAY"));

	UNUSED_PARAMETER(state);

	if (!display_name) {
		fprintf(stderr, "no X display: cases skip\n");
		return 0;
	}

	/* A private bus: never the user's session bus. Bringing it up unsets
	 * DISPLAY, which obs_startup() needs. */
	bus = g_test_dbus_new(G_TEST_DBUS_NONE);
	g_test_dbus_up(bus);
	g_setenv("DISPLAY", display_name, TRUE);

	if (!cameras_start()) {
		fprintf(stderr, "the private PipeWire daemon is not reachable: cases skip\n");
		return 0;
	}

	portal.address = g_test_dbus_get_bus_address(bus);
	portal.context = g_main_context_new();
	portal.loop = g_main_loop_new(portal.context, FALSE);
	os_event_init(&portal.ready, OS_EVENT_TYPE_MANUAL);
	pthread_create(&portal.thread, NULL, portal_thread, &portal);
	os_event_wait(portal.ready);

	display = XOpenDisplay(NULL);
	if (!display) {
		fprintf(stderr, "cannot open the X display: cases skip\n");
		return 0;
	}

	base_set_log_handler(log_handler, NULL);
	obs_set_nix_platform(OBS_NIX_PLATFORM_X11_EGL);
	obs_set_nix_platform_display(display);
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "obs_startup failed: cases skip\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = WIDTH,
		.base_height = HEIGHT,
		.output_width = WIDTH,
		.output_height = HEIGHT,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.adapter = 0,
		.gpu_conversion = true,
		.scale_type = OBS_SCALE_POINT,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: cases skip\n");
		obs_shutdown();
		return 0;
	}
	obs_up = true;

	camera_portal_load();
	for (int i = 0; i < CAMERA_COUNT; i++)
		sources[i] = create_camera_source(&cameras[i]);

	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);

	if (obs_up) {
		for (int i = 0; i < CAMERA_COUNT; i++) {
			obs_source_release(sources[i]);
			sources[i] = NULL;
		}
		obs_wait_for_destroy_queue();
		iterate_main_loop(100);
		obs_shutdown();
		camera_portal_unload();
	}
	cameras_stop();
	if (display)
		XCloseDisplay(display);
	/* The bus is not torn down: the portal code keeps its connection,
	 * which g_test_dbus_down() would wait for. GTestDBus stops the bus
	 * when the process exits. */
	return 0;
}

/* Each case starts from the controls as the cameras report them: no value
 * saved in the sources' settings. */
static int forget_saved_controls(void **state)
{
	UNUSED_PARAMETER(state);

	if (!obs_up)
		return 0;
	for (int cam = 0; cam < CAMERA_COUNT; cam++) {
		obs_data_t *settings = obs_source_get_settings(sources[cam]);

		for (uint32_t i = 0; i < cameras[cam].n_controls; i++)
			obs_data_unset_user_value(settings, cameras[cam].controls[i].name);
		obs_data_release(settings);
	}
	return 0;
}

/* ------------------------------------------------------------------ */
/* Moving a control                                                   */

/* Guard: moving the Brightness slider sets the camera's brightness. */
static void control_change_reaches_the_camera(void **state)
{
	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	obs_properties_t *props = open_properties(UHD_CAMERA);

	change_in_properties(UHD_CAMERA, props, BRIGHTNESS, 3);
	int32_t brightness = wait_for_camera_value(UHD_CAMERA, BRIGHTNESS, 3);
	obs_properties_destroy(props);
	assert_int_equal(brightness, 3);
}

/* Each step of a slider drag must leave the Properties window's widgets in
 * place: the control's callback must not ask for a rebuild. */
static void control_change_does_not_rebuild_properties(void **state)
{
	int rebuilds = 0;

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	obs_properties_t *props = open_properties(UHD_CAMERA);

	/* A short drag: 14 down to 7. */
	for (int value = 14; value >= 7; value--)
		rebuilds += change_in_properties(UHD_CAMERA, props, BRIGHTNESS, value);
	obs_properties_destroy(props);

	if (rebuilds) {
		fprintf(stderr, "%d of 8 Brightness changes asked to rebuild the Properties window\n", rebuilds);
		fail();
	}
}

/* Dragging manual controls whose automatic mode is off (Focus, Absolute
 * after unticking Focus, Automatic Continuous; Iris, Absolute, whose step is
 * 0) writes them and never rebuilds the window or disables anything. Moving
 * the disabled Focus, Absolute (from a script) neither rebuilds the window
 * nor reaches the camera, which ignores or refuses it. */
static void moving_a_manual_control_keeps_the_properties(void **state)
{
	int rebuilds = 0;
	int sets_before;
	int sets_after;

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	set_saved(UHD_CAMERA, FOCUS_AUTO, 0, true);
	obs_properties_t *props = open_properties(UHD_CAMERA);
	change_in_properties(UHD_CAMERA, props, FOCUS_AUTO, 0);

	for (int step = 0; step < 8; step++) {
		rebuilds += change_in_properties(UHD_CAMERA, props, FOCUS, 1000 + step * 100);
		rebuilds += change_in_properties(UHD_CAMERA, props, IRIS, 200 + step * 50);
	}
	bool focus_enabled = control_enabled(props, FOCUS);
	bool iris_enabled = control_enabled(props, IRIS);
	int32_t focus = wait_for_camera_value(UHD_CAMERA, FOCUS, 1700);
	int32_t iris = wait_for_camera_value(UHD_CAMERA, IRIS, 550);

	/* Autofocus on again: Focus, Absolute is disabled; moving it anyway
	 * (a script) keeps the window. */
	change_in_properties(UHD_CAMERA, props, FOCUS_AUTO, 1);
	camera_value(&cameras[UHD_CAMERA], FOCUS, &sets_before);
	for (int step = 0; step < 8; step++)
		rebuilds += change_in_properties(UHD_CAMERA, props, FOCUS, 2000 + step * 100);
	iterate_main_loop(100);
	camera_value(&cameras[UHD_CAMERA], FOCUS, &sets_after);
	obs_properties_destroy(props);

	assert_int_equal(focus, 1700);
	assert_int_equal(iris, 550);
	if (rebuilds || !focus_enabled || !iris_enabled) {
		fprintf(stderr, "%d of 24 moves asked to rebuild; '%s' enabled %d, '%s' enabled %d\n", rebuilds, FOCUS,
			focus_enabled, IRIS, iris_enabled);
		fail();
	}
	/* The camera ignores or refuses it while autofocus is on */
	if (sets_after != sets_before) {
		fprintf(stderr, "the disabled '%s' was sent to the camera %d times\n", FOCUS, sets_after - sets_before);
		fail();
	}
}

/* ------------------------------------------------------------------ */
/* Menu controls                                                      */

/* Auto Exposure and Power Line Frequency are lists with the camera's
 * labels; the node's device path, name and fd are not controls. */
static void menu_controls_are_shown(void **state)
{
	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	obs_properties_t *props = open_properties(UHD_CAMERA);
	obs_property_t *exposure_mode = obs_properties_get(props, AUTO_EXPOSURE);
	obs_property_t *power_line = obs_properties_get(props, POWER_LINE);
	bool fd_shown = obs_properties_get(props, "The V4L2 fd") != NULL;
	bool device_shown = obs_properties_get(props, "The V4L2 device") != NULL;

	if (!exposure_mode || !power_line) {
		fprintf(stderr, "menus missing from the Camera Controls group: '%s' %s, '%s' %s\n", AUTO_EXPOSURE,
			exposure_mode ? "shown" : "missing", POWER_LINE, power_line ? "shown" : "missing");
		obs_properties_destroy(props);
		fail();
	}
	assert_int_equal(obs_property_get_type(exposure_mode), OBS_PROPERTY_LIST);
	assert_int_equal(obs_property_list_item_count(exposure_mode), 2);
	assert_string_equal(obs_property_list_item_name(exposure_mode, 0), "Auto Mode");
	assert_int_equal(obs_property_list_item_int(exposure_mode, 1), V4L2_EXPOSURE_MANUAL);
	assert_string_equal(obs_property_list_item_name(exposure_mode, 1), "Manual Mode");
	assert_int_equal(obs_property_list_item_count(power_line), 3);
	assert_string_equal(obs_property_list_item_name(power_line, 2), "60 Hz");
	obs_properties_destroy(props);

	assert_false(fd_shown);
	assert_false(device_shown);
}

/* ------------------------------------------------------------------ */
/* Manual controls while their automatic mode is on                   */

static const char *const uhd_auto_on[] = {FOCUS, EXPOSURE};
static const char *const uhd_always_on[] = {BRIGHTNESS,       HUE,  "Gain",    FOCUS_AUTO,
					    "Zoom, Absolute", IRIS, PAN_SPEED, TILT_SPEED};
static const char *const webcam_auto_on[] = {WB_TEMPERATURE, RED_BALANCE, BLUE_BALANCE, HUE, EXPOSURE, FOCUS};

static void check_enabled(obs_properties_t *props, const char *const *names, size_t n, bool enabled, const char *when)
{
	for (size_t i = 0; i < n; i++) {
		if (control_enabled(props, names[i]) != enabled) {
			fprintf(stderr, "%s: '%s' is %s\n", when, names[i], enabled ? "disabled" : "enabled");
			fail();
		}
	}
}

/* The UHD Camera of #827 as it starts: Auto Exposure in Auto Mode and
 * autofocus on. Focus, Absolute (listed before its automatic control) and
 * Exposure Time, Absolute are disabled when the window opens; the others,
 * Iris, Absolute (step 0) and the controls after it included, are enabled.
 * The same for the webcam's white balance, hue, exposure (Aperture Priority
 * Mode) and focus. */
static void manual_controls_are_disabled_on_load(void **state)
{
	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	obs_properties_t *props = open_properties(UHD_CAMERA);
	check_enabled(props, uhd_always_on, sizeof(uhd_always_on) / sizeof(uhd_always_on[0]), true,
		      "UHD Camera on load");
	check_enabled(props, uhd_auto_on, sizeof(uhd_auto_on) / sizeof(uhd_auto_on[0]), false,
		      "UHD Camera on load, Auto Mode and autofocus");
	obs_properties_destroy(props);

	props = open_properties(WEBCAM);
	check_enabled(props, webcam_auto_on, sizeof(webcam_auto_on) / sizeof(webcam_auto_on[0]), false,
		      "webcam on load, all automatic");
	check_enabled(props, (const char *const[]){BRIGHTNESS, WB_AUTO, HUE_AUTO, FOCUS_AUTO}, 4, true,
		      "webcam on load");
	obs_properties_destroy(props);
}

/* Saved settings with the automatic modes off (Manual Mode, autofocus, white
 * balance and hue unticked): the manual controls are enabled when the window
 * opens. */
static void saved_manual_modes_enable_the_controls_on_load(void **state)
{
	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	set_saved(UHD_CAMERA, FOCUS_AUTO, 0, true);
	set_saved(UHD_CAMERA, AUTO_EXPOSURE, V4L2_EXPOSURE_MANUAL, false);
	set_saved(WEBCAM, WB_AUTO, 0, true);
	set_saved(WEBCAM, HUE_AUTO, 0, true);
	set_saved(WEBCAM, AUTO_EXPOSURE, V4L2_EXPOSURE_MANUAL, false);
	set_saved(WEBCAM, FOCUS_AUTO, 0, true);

	obs_properties_t *props = open_properties(UHD_CAMERA);
	check_enabled(props, uhd_auto_on, sizeof(uhd_auto_on) / sizeof(uhd_auto_on[0]), true,
		      "UHD Camera on load, saved Manual Mode and autofocus off");
	obs_properties_destroy(props);

	props = open_properties(WEBCAM);
	check_enabled(props, webcam_auto_on, sizeof(webcam_auto_on) / sizeof(webcam_auto_on[0]), true,
		      "webcam on load, all saved manual");
	obs_properties_destroy(props);
}

/* #827: ticking Focus, Automatic Continuous (a checkbox listed after Focus,
 * Absolute) disables Focus, Absolute, and unticking it enables it, each time
 * with one rebuild of the window. Three times over. */
static void toggling_autofocus_disables_and_enables_the_focus(void **state)
{
	static const char *const focus[] = {FOCUS};

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	obs_properties_t *props = open_properties(UHD_CAMERA);
	for (int round = 0; round < 3; round++) {
		check_switch(UHD_CAMERA, props, FOCUS_AUTO, 0, true, focus, 1);
		check_switch(UHD_CAMERA, props, FOCUS_AUTO, 1, false, focus, 1);
	}
	obs_properties_destroy(props);
}

/* Auto Exposure (a list): Manual Mode enables Exposure Time, Absolute, and
 * Auto Mode or Aperture Priority Mode disable it. */
static void switching_auto_exposure_disables_and_enables_the_exposure(void **state)
{
	static const char *const exposure[] = {EXPOSURE};

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	obs_properties_t *props = open_properties(UHD_CAMERA);
	for (int round = 0; round < 2; round++) {
		check_switch(UHD_CAMERA, props, AUTO_EXPOSURE, V4L2_EXPOSURE_MANUAL, true, exposure, 1);
		check_switch(UHD_CAMERA, props, AUTO_EXPOSURE, V4L2_EXPOSURE_AUTO, false, exposure, 1);
	}
	obs_properties_destroy(props);

	props = open_properties(WEBCAM);
	check_switch(WEBCAM, props, AUTO_EXPOSURE, V4L2_EXPOSURE_MANUAL, true, exposure, 1);
	check_switch(WEBCAM, props, AUTO_EXPOSURE, V4L2_EXPOSURE_APERTURE_PRIORITY, false, exposure, 1);
	obs_properties_destroy(props);
}

/* White Balance, Automatic (a checkbox listed before its controls) and Hue,
 * Automatic (listed after Hue) */
static void toggling_white_balance_and_hue_automatic(void **state)
{
	static const char *const white_balance[] = {WB_TEMPERATURE, RED_BALANCE, BLUE_BALANCE};
	static const char *const hue[] = {HUE};

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	obs_properties_t *props = open_properties(WEBCAM);
	for (int round = 0; round < 2; round++) {
		check_switch(WEBCAM, props, WB_AUTO, 0, true, white_balance, 3);
		check_switch(WEBCAM, props, HUE_AUTO, 0, true, hue, 1);
		check_switch(WEBCAM, props, WB_AUTO, 1, false, white_balance, 3);
		check_switch(WEBCAM, props, HUE_AUTO, 1, false, hue, 1);
	}
	/* Hue, Automatic off does not enable the white balance */
	check_switch(WEBCAM, props, HUE_AUTO, 0, true, hue, 1);
	check_enabled(props, white_balance, 3, false, "Hue, Automatic off, White Balance, Automatic on");
	obs_properties_destroy(props);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(control_change_reaches_the_camera, forget_saved_controls),
		cmocka_unit_test_setup(control_change_does_not_rebuild_properties, forget_saved_controls),
		cmocka_unit_test_setup(moving_a_manual_control_keeps_the_properties, forget_saved_controls),
		cmocka_unit_test_setup(menu_controls_are_shown, forget_saved_controls),
		cmocka_unit_test_setup(manual_controls_are_disabled_on_load, forget_saved_controls),
		cmocka_unit_test_setup(saved_manual_modes_enable_the_controls_on_load, forget_saved_controls),
		cmocka_unit_test_setup(toggling_autofocus_disables_and_enables_the_focus, forget_saved_controls),
		cmocka_unit_test_setup(switching_auto_exposure_disables_and_enables_the_exposure,
				       forget_saved_controls),
		cmocka_unit_test_setup(toggling_white_balance_and_hue_automatic, forget_saved_controls),
	};
	g_autofree char *dbus_daemon = g_find_program_in_path("dbus-daemon");

	if (!dbus_daemon) {
		fprintf(stderr, "skipped: needs dbus-daemon for a private bus\n");
		return SKIP_EXIT;
	}
	if (!getenv("PIPEWIRE_RUNTIME_DIR")) {
		fprintf(stderr, "skipped: run through test_pipewire_cursor_hdr.sh (private PipeWire)\n");
		return SKIP_EXIT;
	}

	return cmocka_run_group_tests(tests, setup, teardown);
}
