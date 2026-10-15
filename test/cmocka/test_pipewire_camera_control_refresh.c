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

#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>
#include <spa/param/props.h>

#include <obs.h>
#include <obs-nix-platform.h>
#include <util/platform.h>
#include <util/threading.h>

#include "camera-portal.h"

/* Camera control sliders in the Properties window of the "Video Capture
 * Device (PipeWire)" source cannot be dragged: the handle "drops" after one
 * step, and the mouse wheel moves it by one step only (obs-rust/obs-studio
 * #827). Every camera control's modified callback, control_changed() in
 * camera-portal.c, returned true. A modified callback that returns true asks
 * the Properties window to rebuild all its widgets, so the first value change
 * of a drag destroyed the slider that held the mouse, and focus went to the
 * new spin box, which the wheel then skipped. Changing a control's value
 * changes nothing in the properties' layout: the callback must return false.
 *
 * End to end: the real camera portal code against a fake
 * org.freedesktop.portal.Camera on a private D-Bus bus (GTestDBus), with its
 * PipeWire connection on a private PipeWire daemon that
 * test_pipewire_cursor_hdr.sh starts (own runtime dir, no session manager, no
 * devices). A stream node in this test plays a camera with one control,
 * Brightness (0 to 255), as PipeWire's V4L2 and libcamera nodes publish them
 * (PropInfo and Props params), and records the Props the source sets on it.
 *
 * Needs dbus-daemon and an X display for obs_startup() (xvfb-run is enough);
 * the wrapper skips (77) without PipeWire, and the cases skip when
 * obs_startup() or obs_reset_video() fails. */

#define SKIP_EXIT 77

#define CAMERA_NAME "obs-test-camera"
#define WIDTH 64
#define HEIGHT 48
#define BRIGHTNESS "Brightness"

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
/* The camera: a video source node with a Brightness control          */

static pthread_mutex_t camera_mutex = PTHREAD_MUTEX_INITIALIZER;

static struct {
	struct pw_thread_loop *loop;
	struct pw_context *context;
	struct pw_core *core;
	struct pw_stream *stream;
	struct spa_hook stream_listener;
	uint32_t node;

	/* Written on the camera's PipeWire thread, under camera_mutex. */
	int props_set;  /* Props params received */
	int brightness; /* the last brightness set */
} camera;

/* PropInfo and Props for one Int control, 0 to 255 in steps of 1, default
 * 128: what PipeWire's V4L2 node publishes for a webcam's brightness. */
static uint32_t control_params(struct spa_pod_builder *b, const struct spa_pod **params, int brightness)
{
	params[0] = spa_pod_builder_add_object(b, SPA_TYPE_OBJECT_PropInfo, SPA_PARAM_PropInfo, SPA_PROP_INFO_id,
					       SPA_POD_Id(SPA_PROP_brightness), SPA_PROP_INFO_description,
					       SPA_POD_String(BRIGHTNESS), SPA_PROP_INFO_type,
					       SPA_POD_CHOICE_STEP_Int(128, 0, 255, 1));
	params[1] = spa_pod_builder_add_object(b, SPA_TYPE_OBJECT_Props, SPA_PARAM_Props, SPA_PROP_brightness,
					       SPA_POD_Int(brightness));
	return 2;
}

static void on_camera_param_changed(void *data, uint32_t id, const struct spa_pod *param)
{
	int brightness = -1;

	UNUSED_PARAMETER(data);

	if (!param || id != SPA_PARAM_Props)
		return;
	if (spa_pod_parse_object(param, SPA_TYPE_OBJECT_Props, NULL, SPA_PROP_brightness,
				 SPA_POD_OPT_Int(&brightness)) < 0 ||
	    brightness < 0)
		return;

	pthread_mutex_lock(&camera_mutex);
	camera.props_set++;
	camera.brightness = brightness;
	pthread_mutex_unlock(&camera_mutex);
}

static const struct pw_stream_events camera_events = {
	PW_VERSION_STREAM_EVENTS,
	.param_changed = on_camera_param_changed,
};

static bool camera_start(void)
{
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[3];
	uint32_t node_id = SPA_ID_INVALID;

	pw_init(NULL, NULL);

	camera.loop = pw_thread_loop_new("camera-control-test", NULL);
	camera.context = pw_context_new(pw_thread_loop_get_loop(camera.loop), NULL, 0);
	if (pw_thread_loop_start(camera.loop) < 0)
		return false;

	params[0] = spa_pod_builder_add_object(&b, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat, SPA_FORMAT_mediaType,
					       SPA_POD_Id(SPA_MEDIA_TYPE_video), SPA_FORMAT_mediaSubtype,
					       SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw), SPA_FORMAT_VIDEO_format,
					       SPA_POD_Id(SPA_VIDEO_FORMAT_YUY2), SPA_FORMAT_VIDEO_size,
					       SPA_POD_Rectangle(&SPA_RECTANGLE(WIDTH, HEIGHT)),
					       SPA_FORMAT_VIDEO_framerate, SPA_POD_Fraction(&SPA_FRACTION(30, 1)));
	control_params(&b, params + 1, 128);

	pw_thread_loop_lock(camera.loop);
	camera.core = pw_context_connect(camera.context, NULL, 0);
	if (camera.core) {
		camera.stream = pw_stream_new(camera.core, CAMERA_NAME,
					      pw_properties_new(PW_KEY_NODE_NAME, CAMERA_NAME, PW_KEY_NODE_DESCRIPTION,
								"Test Camera", PW_KEY_MEDIA_CLASS, "Video/Source",
								PW_KEY_MEDIA_ROLE, "Camera", NULL));
		pw_stream_add_listener(camera.stream, &camera.stream_listener, &camera_events, NULL);
		pw_stream_connect(camera.stream, PW_DIRECTION_OUTPUT, PW_ID_ANY,
				  PW_STREAM_FLAG_DRIVER | PW_STREAM_FLAG_MAP_BUFFERS, params, 3);
	}
	pw_thread_loop_unlock(camera.loop);
	if (!camera.core)
		return false;

	for (int i = 0; i < 200 && node_id == SPA_ID_INVALID; i++) {
		os_sleep_ms(10);
		pw_thread_loop_lock(camera.loop);
		node_id = pw_stream_get_node_id(camera.stream);
		pw_thread_loop_unlock(camera.loop);
	}
	camera.node = node_id;
	return node_id != SPA_ID_INVALID;
}

static void camera_stop(void)
{
	if (!camera.loop)
		return;
	pw_thread_loop_lock(camera.loop);
	g_clear_pointer(&camera.stream, pw_stream_destroy);
	g_clear_pointer(&camera.core, pw_core_disconnect);
	pw_thread_loop_unlock(camera.loop);
	pw_thread_loop_stop(camera.loop);
	pw_context_destroy(camera.context);
	pw_thread_loop_destroy(camera.loop);
	memset(&camera, 0, sizeof(camera));
}

static int camera_props_set(int *brightness)
{
	int n;

	pthread_mutex_lock(&camera_mutex);
	n = camera.props_set;
	if (brightness)
		*brightness = camera.brightness;
	pthread_mutex_unlock(&camera_mutex);
	return n;
}

/* ------------------------------------------------------------------ */
/* The camera source                                                  */

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

static obs_source_t *create_camera_source(void)
{
	obs_data_t *settings = obs_data_create();
	obs_source_t *source;

	obs_data_set_string(settings, "device_id", CAMERA_NAME);
	source = obs_source_create("pipewire-camera-source", "camera", settings, NULL);
	obs_data_release(settings);
	return source;
}

/* The source's properties, as the Properties window gets them, once the
 * camera and its control are known: NULL if the control never shows up. */
static obs_properties_t *properties_with_control(obs_source_t *source)
{
	for (int i = 0; i < 250; i++) {
		obs_properties_t *props = obs_source_properties(source);

		if (obs_properties_get(props, BRIGHTNESS))
			return props;
		obs_properties_destroy(props);
		iterate_main_loop(20);
	}
	return NULL;
}

/* The user moves the Brightness slider to `value`: what the Properties
 * window does for each step of a drag. Returns the callback's answer, true
 * meaning "rebuild the Properties window". */
static bool move_brightness(obs_source_t *source, obs_properties_t *props, int value)
{
	obs_data_t *settings = obs_source_get_settings(source);
	bool rebuild;

	obs_data_set_int(settings, BRIGHTNESS, value);
	rebuild = obs_property_modified(obs_properties_get(props, BRIGHTNESS), settings);
	obs_data_release(settings);
	return rebuild;
}

/* ------------------------------------------------------------------ */
/* Fixture                                                            */

static bool obs_up;
static GTestDBus *bus;
static struct portal portal;
static Display *display;
static obs_source_t *camera_source;

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

	if (!camera_start()) {
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
	camera_source = create_camera_source();

	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);

	if (obs_up) {
		obs_source_release(camera_source);
		camera_source = NULL;
		obs_wait_for_destroy_queue();
		iterate_main_loop(100);
		obs_shutdown();
		camera_portal_unload();
	}
	camera_stop();
	if (display)
		XCloseDisplay(display);
	/* The bus is not torn down: the portal code keeps its connection,
	 * which g_test_dbus_down() would wait for. GTestDBus stops the bus
	 * when the process exits. */
	return 0;
}

/* Guard: moving the Brightness slider sets the camera's brightness. */
static void control_change_reaches_the_camera(void **state)
{
	obs_properties_t *props;
	int brightness = -1;

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	assert_non_null(camera_source);
	props = properties_with_control(camera_source);
	if (!props) {
		fprintf(stderr, "the camera's Brightness control never showed up in the properties\n");
		fail();
	}

	move_brightness(camera_source, props, 100);
	for (int i = 0; i < 250; i++) {
		if (camera_props_set(&brightness) > 0 && brightness == 100)
			break;
		iterate_main_loop(20);
	}
	obs_properties_destroy(props);
	assert_int_equal(brightness, 100);
}

/* Each step of a slider drag must leave the Properties window's widgets in
 * place: the control's callback must not ask for a rebuild. */
static void control_change_does_not_rebuild_properties(void **state)
{
	obs_properties_t *props;
	int rebuilds = 0;

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	assert_non_null(camera_source);
	props = properties_with_control(camera_source);
	if (!props) {
		fprintf(stderr, "the camera's Brightness control never showed up in the properties\n");
		fail();
	}

	/* A short drag: 128 down to 120. */
	for (int value = 127; value >= 120; value--) {
		if (move_brightness(camera_source, props, value))
			rebuilds++;
	}
	obs_properties_destroy(props);

	if (rebuilds) {
		fprintf(stderr, "%d of 8 Brightness changes asked to rebuild the Properties window\n", rebuilds);
		fail();
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(control_change_reaches_the_camera),
		cmocka_unit_test(control_change_does_not_rebuild_properties),
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
