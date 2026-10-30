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
#include <spa/param/buffers.h>

#include <obs.h>
#include <obs-nix-platform.h>
#include <media-io/frame-rate.h>
#include <util/platform.h>
#include <util/threading.h>

#include "camera-portal.h"

/* A libcamera camera repeats its default size in its formats
 * (obs-rust/obs-studio#1068): PipeWire's libcamera plugin
 * (spa_libcamera_enum_format) emits, for each pixel format, first the size
 * closest to 640x480, then every size, so the same format and size come as
 * two EnumFormat params, and the camera source's "format" list showed that
 * entry twice.
 *
 * The real camera portal code against a fake org.freedesktop.portal.Camera on
 * a private D-Bus bus (GTestDBus), with its PipeWire connection on a private
 * PipeWire daemon that test_pipewire_cursor_hdr.py starts. A stream node in
 * this test plays the camera, with libcamera's order: YUY2 64x48 (the
 * default), then YUY2 32x24 and YUY2 64x48 again.
 *
 * Needs dbus-daemon and an X display for obs_startup() (xvfb-run is enough);
 * the wrapper skips (77) without PipeWire, and the case skips when
 * obs_startup() or obs_reset_video() fails. */

#define SKIP_EXIT 77

#define CAMERA_NAME "obs-test-camera"
#define WIDTH 64
#define HEIGHT 48
#define SMALL_WIDTH 32
#define SMALL_HEIGHT 24
#define SMALL_HEIGHT 24

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
/* The camera: a video source node on the private daemon               */

static struct {
	struct pw_thread_loop *loop;
	struct pw_context *context;
	struct pw_core *core;
	struct pw_stream *stream;
} camera;

static const struct spa_pod *yuy2_format(struct spa_pod_builder *b, uint32_t width, uint32_t height)
{
	return spa_pod_builder_add_object(b, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat, SPA_FORMAT_mediaType,
					  SPA_POD_Id(SPA_MEDIA_TYPE_video), SPA_FORMAT_mediaSubtype,
					  SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw), SPA_FORMAT_VIDEO_format,
					  SPA_POD_Id(SPA_VIDEO_FORMAT_YUY2), SPA_FORMAT_VIDEO_size,
					  SPA_POD_Rectangle(&SPA_RECTANGLE(width, height)), SPA_FORMAT_VIDEO_framerate,
					  SPA_POD_CHOICE_ENUM_Fraction(2, &SPA_FRACTION(30, 1), &SPA_FRACTION(30, 1)));
}

/* libcamera's order: the default size, then every size. */
static bool camera_start(void)
{
	uint8_t buffer[2048];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[3];
	uint32_t node_id = SPA_ID_INVALID;

	pw_init(NULL, NULL);

	camera.loop = pw_thread_loop_new("camera-format-dedup-test", NULL);
	camera.context = pw_context_new(pw_thread_loop_get_loop(camera.loop), NULL, 0);
	if (pw_thread_loop_start(camera.loop) < 0)
		return false;

	params[0] = yuy2_format(&b, WIDTH, HEIGHT);
	params[1] = yuy2_format(&b, SMALL_WIDTH, SMALL_HEIGHT);
	params[2] = yuy2_format(&b, WIDTH, HEIGHT);

	pw_thread_loop_lock(camera.loop);
	camera.core = pw_context_connect(camera.context, NULL, 0);
	if (camera.core) {
		camera.stream = pw_stream_new(camera.core, CAMERA_NAME,
					      pw_properties_new(PW_KEY_NODE_NAME, CAMERA_NAME, PW_KEY_NODE_DESCRIPTION,
								"Test Camera", PW_KEY_MEDIA_CLASS, "Video/Source",
								PW_KEY_MEDIA_ROLE, "Camera", NULL));
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

/* ------------------------------------------------------------------ */
/* Log                                                                */

static pthread_mutex_t log_mutex = PTHREAD_MUTEX_INITIALIZER;
static bool found;

static void log_handler(int level, const char *format, va_list args, void *param)
{
	char line[256];

	UNUSED_PARAMETER(param);

	vsnprintf(line, sizeof(line), format, args);

	pthread_mutex_lock(&log_mutex);
	if (strcmp(line, "[camera-portal] Found device " CAMERA_NAME) == 0)
		found = true;
	pthread_mutex_unlock(&log_mutex);

	if (level <= LOG_WARNING || getenv("CAMERA_FORMAT_DEDUP_TEST_VERBOSE"))
		fprintf(stderr, "%s\n", line);
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

static bool wait_for_found(void)
{
	for (int i = 0; i < 250; i++) {
		bool is_found;

		pthread_mutex_lock(&log_mutex);
		is_found = found;
		pthread_mutex_unlock(&log_mutex);
		if (is_found)
			return true;
		iterate_main_loop(20);
	}
	return false;
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

	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "device_id", CAMERA_NAME);
	camera_source = obs_source_create("pipewire-camera-source", "camera", settings, NULL);
	obs_data_release(settings);

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

/* The format list, as the Properties window fills it when the camera is
 * selected. Returns the number of items, and logs them. */
static size_t list_formats(obs_properties_t *props)
{
	obs_data_t *settings = obs_source_get_settings(camera_source);
	obs_property_t *format_list = obs_properties_get(props, "format");
	size_t count;

	obs_property_modified(obs_properties_get(props, "device_id"), settings);
	obs_data_release(settings);

	count = obs_property_list_item_count(format_list);
	for (size_t i = 0; i < count; i++)
		fprintf(stderr, "format %zu: %s\n", i, obs_property_list_item_name(format_list, i));
	return count;
}

/* Each format and size of the camera is listed once: 64x48 and 32x24. */
static void each_format_and_size_listed_once(void **state)
{
	obs_properties_t *props = NULL;
	obs_property_t *format_list;
	size_t count = 0;

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	assert_non_null(camera_source);
	assert_true(wait_for_found());

	/* The camera's params arrive after the device: wait for them. */
	for (int i = 0; i < 100 && count < 2; i++) {
		g_clear_pointer(&props, obs_properties_destroy);
		iterate_main_loop(20);
		props = obs_source_properties(camera_source);
		count = list_formats(props);
	}
	iterate_main_loop(300);
	g_clear_pointer(&props, obs_properties_destroy);
	props = obs_source_properties(camera_source);
	count = list_formats(props);

	format_list = obs_properties_get(props, "format");
	for (size_t i = 0; i < count; i++) {
		for (size_t j = i + 1; j < count; j++) {
			if (strcmp(obs_property_list_item_name(format_list, i),
				   obs_property_list_item_name(format_list, j)) == 0) {
				fprintf(stderr, "format \"%s\" is listed twice (items %zu and %zu)\n",
					obs_property_list_item_name(format_list, i), i, j);
				fail();
			}
		}
	}
	if (count != 2) {
		fprintf(stderr, "%zu formats listed, expected 2 (64x48 and 32x24)\n", count);
		fail();
	}
	/* The default size stays first. */
	assert_non_null(strstr(obs_property_list_item_name(format_list, 0), "64x48"));

	obs_properties_destroy(props);
}

/* A source whose settings carry no valid "format" yet (a freshly added
 * source, or one saved before the field existed) still shows a format label
 * that names its framerate ("64x48 (4:3) - 30 FPS - YUY2"). The framerate
 * list next to it must offer that framerate instead of staying empty and
 * disabled (obs-rust/obs-studio#1068 part 2). */
static void initial_settings_fill_framerate_list(void **state)
{
	obs_properties_t *props = NULL;
	obs_property_t *format_list;
	obs_property_t *framerate_prop;
	size_t count = 0;
	size_t framerate_count;
	const char *first_label;

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	assert_non_null(camera_source);
	assert_true(wait_for_found());

	/* The camera's params arrive after the device: wait for them. */
	for (int i = 0; i < 100 && count < 2; i++) {
		g_clear_pointer(&props, obs_properties_destroy);
		iterate_main_loop(20);
		props = obs_source_properties(camera_source);
		count = list_formats(props);
	}
	iterate_main_loop(300);
	g_clear_pointer(&props, obs_properties_destroy);
	props = obs_source_properties(camera_source);
	count = list_formats(props);
	if (count < 1) {
		fprintf(stderr, "no formats listed\n");
		fail();
	}

	format_list = obs_properties_get(props, "format");
	first_label = obs_property_list_item_name(format_list, 0);
	fprintf(stderr, "first format: %s\n", first_label);

	framerate_prop = obs_properties_get(props, "framerate");
	framerate_count = obs_property_list_item_count(framerate_prop);
	fprintf(stderr, "framerate items: %zu, enabled: %d\n", framerate_count, obs_property_enabled(framerate_prop));

	/* The fake camera lists one framerate, 30 FPS, for every size. */
	if (framerate_count != 1) {
		fprintf(stderr, "%zu framerates listed under a format label that names one\n", framerate_count);
		fail();
	}
	if (!obs_property_enabled(framerate_prop)) {
		fprintf(stderr, "the framerate list is disabled while its format names a framerate\n");
		fail();
	}

	obs_properties_destroy(props);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(each_format_and_size_listed_once),
		cmocka_unit_test(initial_settings_fill_framerate_list),
	};
	g_autofree char *dbus_daemon = g_find_program_in_path("dbus-daemon");

	if (!dbus_daemon) {
		fprintf(stderr, "skipped: needs dbus-daemon for a private bus\n");
		return SKIP_EXIT;
	}
	if (!getenv("PIPEWIRE_RUNTIME_DIR")) {
		fprintf(stderr, "skipped: run through test_pipewire_cursor_hdr.py (private PipeWire)\n");
		return SKIP_EXIT;
	}

	return cmocka_run_group_tests(tests, setup, teardown);
}
