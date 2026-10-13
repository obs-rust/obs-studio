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

#include <obs.h>
#include <obs-nix-platform.h>
#include <util/platform.h>
#include <util/threading.h>

#include "camera-portal.h"
#include "pipewire.h"

/* Quitting OBS with a "Video Capture Device (PipeWire)" source could abort
 * (double free, SIGABRT), and PipeWire printed
 * "impl_ext_end_proxy called from wrong context" on every quit:
 * camera_portal_unload() destroyed the camera portal's device proxies from
 * the unloading thread without the PipeWire thread-loop lock, while the
 * PipeWire thread was still running and could be dispatching events on the
 * same connection.
 *
 * End to end, like test_pipewire_camera_unplug: the real camera portal code
 * talks to a fake org.freedesktop.portal.Camera on a private D-Bus bus
 * (GTestDBus) and gets its PipeWire connection from a private PipeWire daemon
 * that test_pipewire_cursor_hdr.sh starts for it. A stream node in this test
 * plays the camera, so the camera portal binds a device proxy for it.
 *
 * The race itself is rare; what makes it possible is deterministic. The test
 * links with --wrap to follow the camera portal's PipeWire thread loop (its
 * lock depth per thread, and whether it is still running) and checks every
 * pw_proxy_destroy() made by the camera portal or pipewire.c: while the loop
 * runs, it must happen on the loop thread or with the loop locked.
 *
 * Needs dbus-daemon and an X display for obs_startup() (xvfb-run is enough);
 * the wrapper skips (77) without PipeWire, and the cases skip when
 * obs_startup() fails. */

#define SKIP_EXIT 77

#define CAMERA_NAME "obs-test-camera"
#define FOUND_LINE "[camera-portal] Found device " CAMERA_NAME
#define PORTAL_LOOP_NAME "PipeWire thread loop"

const char *obs_module_text(const char *val)
{
	return val;
}

/* ------------------------------------------------------------------ */
/* Link-time hooks (--wrap) on the camera portal's PipeWire thread loop */

extern struct pw_thread_loop *__real_pw_thread_loop_new(const char *name, const struct spa_dict *props);
extern void __real_pw_thread_loop_lock(struct pw_thread_loop *loop);
extern void __real_pw_thread_loop_unlock(struct pw_thread_loop *loop);
extern void __real_pw_thread_loop_stop(struct pw_thread_loop *loop);
extern void __real_pw_proxy_destroy(struct pw_proxy *proxy);

/* Visible: the link checks them against the libraries that use these names. */
EXPORT struct pw_thread_loop *__wrap_pw_thread_loop_new(const char *name, const struct spa_dict *props);
EXPORT void __wrap_pw_thread_loop_lock(struct pw_thread_loop *loop);
EXPORT void __wrap_pw_thread_loop_unlock(struct pw_thread_loop *loop);
EXPORT void __wrap_pw_thread_loop_stop(struct pw_thread_loop *loop);
EXPORT void __wrap_pw_proxy_destroy(struct pw_proxy *proxy);

static struct pw_thread_loop *volatile portal_loop;
static volatile bool portal_loop_stopped;
static __thread int portal_lock_depth;

static pthread_mutex_t destroys_mutex = PTHREAD_MUTEX_INITIALIZER;
static int destroys_while_running;
static int unlocked_destroys;

/* pipewire.c names its loop; the camera in this test uses another name. */
struct pw_thread_loop *__wrap_pw_thread_loop_new(const char *name, const struct spa_dict *props)
{
	struct pw_thread_loop *loop = __real_pw_thread_loop_new(name, props);

	if (name && strcmp(name, PORTAL_LOOP_NAME) == 0) {
		portal_loop = loop;
		portal_loop_stopped = false;
	}
	return loop;
}

void __wrap_pw_thread_loop_lock(struct pw_thread_loop *loop)
{
	__real_pw_thread_loop_lock(loop);
	if (loop == portal_loop)
		portal_lock_depth++;
}

void __wrap_pw_thread_loop_unlock(struct pw_thread_loop *loop)
{
	if (loop == portal_loop)
		portal_lock_depth--;
	__real_pw_thread_loop_unlock(loop);
}

void __wrap_pw_thread_loop_stop(struct pw_thread_loop *loop)
{
	__real_pw_thread_loop_stop(loop);
	if (loop == portal_loop)
		portal_loop_stopped = true;
}

/* Only camera-portal.c and pipewire.c call pw_proxy_destroy() in this binary,
 * and only on the camera portal's connection. After the loop is stopped no
 * other thread touches the connection, so any thread may destroy. */
void __wrap_pw_proxy_destroy(struct pw_proxy *proxy)
{
	struct pw_thread_loop *loop = portal_loop;

	if (loop && !portal_loop_stopped) {
		bool safe = portal_lock_depth > 0 || pw_thread_loop_in_thread(loop);

		pthread_mutex_lock(&destroys_mutex);
		destroys_while_running++;
		if (!safe)
			unlocked_destroys++;
		pthread_mutex_unlock(&destroys_mutex);
	}
	__real_pw_proxy_destroy(proxy);
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
/* The camera: a video source node on the private PipeWire daemon      */

static struct {
	struct pw_thread_loop *loop;
	struct pw_context *context;
	struct pw_core *core;
	struct pw_stream *stream;
} camera;

static bool camera_start(void)
{
	pw_init(NULL, NULL);

	camera.loop = pw_thread_loop_new("camera-unload-test", NULL);
	camera.context = pw_context_new(pw_thread_loop_get_loop(camera.loop), NULL, 0);
	if (pw_thread_loop_start(camera.loop) < 0)
		return false;

	pw_thread_loop_lock(camera.loop);
	camera.core = pw_context_connect(camera.context, NULL, 0);
	pw_thread_loop_unlock(camera.loop);
	return camera.core != NULL;
}

static bool camera_plug(void)
{
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];
	uint32_t node_id = SPA_ID_INVALID;

	params[0] = spa_pod_builder_add_object(&b, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat, SPA_FORMAT_mediaType,
					       SPA_POD_Id(SPA_MEDIA_TYPE_video), SPA_FORMAT_mediaSubtype,
					       SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw), SPA_FORMAT_VIDEO_format,
					       SPA_POD_Id(SPA_VIDEO_FORMAT_YUY2), SPA_FORMAT_VIDEO_size,
					       SPA_POD_Rectangle(&SPA_RECTANGLE(64, 64)), SPA_FORMAT_VIDEO_framerate,
					       SPA_POD_Fraction(&SPA_FRACTION(30, 1)));

	pw_thread_loop_lock(camera.loop);
	camera.stream =
		pw_stream_new(camera.core, CAMERA_NAME,
			      pw_properties_new(PW_KEY_NODE_NAME, CAMERA_NAME, PW_KEY_NODE_DESCRIPTION, "Test Camera",
						PW_KEY_MEDIA_CLASS, "Video/Source", PW_KEY_MEDIA_ROLE, "Camera", NULL));
	pw_stream_connect(camera.stream, PW_DIRECTION_OUTPUT, PW_ID_ANY, PW_STREAM_FLAG_NONE, params, 1);
	pw_thread_loop_unlock(camera.loop);

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
/* Log                                                                 */

static pthread_mutex_t log_mutex = PTHREAD_MUTEX_INITIALIZER;
static bool camera_found;

static void log_handler(int level, const char *format, va_list args, void *param)
{
	char line[256];

	UNUSED_PARAMETER(param);

	vsnprintf(line, sizeof(line), format, args);

	pthread_mutex_lock(&log_mutex);
	if (strcmp(line, FOUND_LINE) == 0)
		camera_found = true;
	pthread_mutex_unlock(&log_mutex);

	if (level <= LOG_WARNING || getenv("CAMERA_UNLOAD_TEST_VERBOSE"))
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
		bool found;

		pthread_mutex_lock(&log_mutex);
		found = camera_found;
		pthread_mutex_unlock(&log_mutex);
		if (found)
			return true;
		iterate_main_loop(20);
	}
	return false;
}

/* ------------------------------------------------------------------ */
/* Fixture                                                            */

static bool obs_up;
static bool portal_unloaded;
static obs_source_t *camera_source;
static GTestDBus *bus;
static struct portal portal;
static Display *display;

static int setup(void **state)
{
	g_autofree char *display_name = g_strdup(getenv("DISPLAY"));
	obs_data_t *settings;

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

	if (!camera_start() || !camera_plug()) {
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
	obs_up = true;

	camera_portal_load();

	/* Set to another camera, so that the source does not stream from the
	 * test camera: only the device list is involved. */
	settings = obs_data_create();
	obs_data_set_string(settings, "device_id", "another-camera");
	camera_source = obs_source_create("pipewire-camera-source", "camera", settings, NULL);
	obs_data_release(settings);

	/* AccessCamera, OpenPipeWireRemote, then the registry lists the
	 * camera. */
	if (camera_source)
		wait_for_found();

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
		if (!portal_unloaded)
			camera_portal_unload();
		obs_shutdown();
	}
	camera_stop();
	/* The bus is not torn down: the portal code keeps its connection,
	 * which g_test_dbus_down() would wait for. GTestDBus stops the bus
	 * when the process exits. */
	return 0;
}

/* Guard: the camera shows up in the camera portal's device list. */
static void plugged_camera_is_found(void **state)
{
	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();

	assert_non_null(camera_source);
	if (!wait_for_found()) {
		fprintf(stderr, "the camera portal never listed the camera \"%s\"\n", CAMERA_NAME);
		fail();
	}
}

/* What obs_shutdown() does on quit: the sources are gone, then the module
 * unloads while the camera portal's PipeWire thread still runs. */
static void unload_destroys_device_proxies_under_loop_lock(void **state)
{
	int running, unlocked;

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();

	assert_non_null(camera_source);
	assert_true(wait_for_found());

	obs_source_release(camera_source);
	camera_source = NULL;
	obs_wait_for_destroy_queue();
	iterate_main_loop(100);

	camera_portal_unload();
	portal_unloaded = true;

	pthread_mutex_lock(&destroys_mutex);
	running = destroys_while_running;
	unlocked = unlocked_destroys;
	pthread_mutex_unlock(&destroys_mutex);

	fprintf(stderr, "proxies destroyed while the PipeWire thread ran: %d, without the loop lock: %d\n", running,
		unlocked);

	/* Guard: the camera's device proxy was destroyed while the loop ran. */
	assert_true(running >= 1);
	if (unlocked != 0) {
		fprintf(stderr, "the camera portal destroyed PipeWire proxies off the loop thread without its lock\n");
		fail();
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(plugged_camera_is_found),
		cmocka_unit_test(unload_destroys_device_proxies_under_loop_lock),
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
