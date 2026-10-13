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

/* obsproject/obs-studio#13971: OBS could crash at startup when the scene
 * collection has a "Video Capture Device (PipeWire)" source.
 *
 * on_pipewire_remote_opened_cb() stored the camera portal's PipeWire
 * connection in connection->obs_pw only after obs_pipewire_connect_fd() had
 * returned. By then that function had already added the registry listener and
 * let the PipeWire thread run, so registry globals could reach
 * on_registry_global_cb() first. It reads connection->obs_pw (NULL until the
 * assignment) to bind the node and, for a source set to that camera, to
 * connect its stream: a NULL dereference on the PipeWire thread.
 *
 * The window is short, so the test makes it deterministic: it links with
 * --wrap and holds the camera portal right after obs_pipewire_connect_fd()
 * returns, before the assignment, until a roundtrip on the new connection
 * shows that PipeWire has delivered everything the registry had to say. Every
 * call the camera portal makes with a NULL connection is counted instead of
 * crashing: obs_pipewire_get_registry() gets the real connection's registry
 * and obs_pipewire_connect_stream() returns no stream.
 *
 * End to end like test_pipewire_camera_unplug: the real camera portal code
 * talks to a fake org.freedesktop.portal.Camera on a private D-Bus bus
 * (GTestDBus) and gets its PipeWire connection from a private PipeWire daemon
 * that test_pipewire_cursor_hdr.sh starts for it (own runtime dir, no session
 * manager, no devices). A stream node in this test plays the camera; the
 * source is set to it before the camera portal connects, as when a saved
 * scene collection is loaded at startup.
 *
 * Needs dbus-daemon and an X display for obs_startup() (xvfb-run is enough);
 * the wrapper skips (77) without PipeWire, and the cases skip when
 * obs_startup() fails. */

#define SKIP_EXIT 77

#define CAMERA_NAME "obs-test-camera"
#define FOUND_LINE "[camera-portal] Found device " CAMERA_NAME

const char *obs_module_text(const char *val)
{
	return val;
}

/* ------------------------------------------------------------------ */
/* Link-time hooks (--wrap) between the camera portal and pipewire.c  */

static pthread_mutex_t hooks_mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t held_cond = PTHREAD_COND_INITIALIZER;
static obs_pipewire *held_obs_pw;
static int null_registry_reads;
static int null_stream_connects;
static int streams_connected;

extern obs_pipewire *__real_obs_pipewire_connect_fd(int pipewire_fd);
extern struct pw_registry *__real_obs_pipewire_get_registry(obs_pipewire *obs_pw);
extern obs_pipewire_stream *
__real_obs_pipewire_connect_stream(obs_pipewire *obs_pw, obs_source_t *source, int pipewire_node,
				   const struct obs_pipewire_connect_stream_info *connect_info);

/* Visible: the link checks them against the libraries that use these names. */
EXPORT obs_pipewire *__wrap_obs_pipewire_connect_fd(int pipewire_fd);
EXPORT struct pw_registry *__wrap_obs_pipewire_get_registry(obs_pipewire *obs_pw);
EXPORT obs_pipewire_stream *
__wrap_obs_pipewire_connect_stream(obs_pipewire *obs_pw, obs_source_t *source, int pipewire_node,
				   const struct obs_pipewire_connect_stream_info *connect_info);

/* camera-portal.c is the only caller in this binary. */
obs_pipewire *__wrap_obs_pipewire_connect_fd(int pipewire_fd)
{
	obs_pipewire *obs_pw = __real_obs_pipewire_connect_fd(pipewire_fd);

	if (!obs_pw)
		return NULL;

	/* Hold the caller until PipeWire has answered everything sent so far,
	 * the registry's globals included. */
	pthread_mutex_lock(&hooks_mutex);
	held_obs_pw = obs_pw;
	pthread_cond_broadcast(&held_cond);
	pthread_mutex_unlock(&hooks_mutex);
	obs_pipewire_roundtrip(obs_pw);

	return obs_pw;
}

struct pw_registry *__wrap_obs_pipewire_get_registry(obs_pipewire *obs_pw)
{
	if (!obs_pw) {
		/* Called from the PipeWire thread before the camera portal has
		 * its connection: count it, and carry on with the connection
		 * obs_pipewire_connect_fd() is about to return (this thread may
		 * get here before the hook above has seen it). */
		pthread_mutex_lock(&hooks_mutex);
		null_registry_reads++;
		while (!held_obs_pw)
			pthread_cond_wait(&held_cond, &hooks_mutex);
		obs_pw = held_obs_pw;
		pthread_mutex_unlock(&hooks_mutex);
	}
	return __real_obs_pipewire_get_registry(obs_pw);
}

obs_pipewire_stream *__wrap_obs_pipewire_connect_stream(obs_pipewire *obs_pw, obs_source_t *source, int pipewire_node,
							const struct obs_pipewire_connect_stream_info *connect_info)
{
	obs_pipewire_stream *stream = NULL;

	if (obs_pw)
		stream = __real_obs_pipewire_connect_stream(obs_pw, source, pipewire_node, connect_info);
	else
		pw_properties_free(connect_info->stream_properties);

	pthread_mutex_lock(&hooks_mutex);
	if (!obs_pw)
		null_stream_connects++;
	else if (stream)
		streams_connected++;
	pthread_mutex_unlock(&hooks_mutex);

	return stream;
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

	camera.loop = pw_thread_loop_new("camera-registry-race-test", NULL);
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

	if (level <= LOG_WARNING || getenv("CAMERA_REGISTRY_RACE_TEST_VERBOSE"))
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

static int wait_for_streams_connected(void)
{
	int connected = 0;

	for (int i = 0; i < 100; i++) {
		pthread_mutex_lock(&hooks_mutex);
		connected = streams_connected;
		pthread_mutex_unlock(&hooks_mutex);
		if (connected)
			break;
		iterate_main_loop(20);
	}
	return connected;
}

/* ------------------------------------------------------------------ */
/* Fixture                                                            */

static bool obs_up;
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

	/* As loaded from a saved scene collection: the source is set to the
	 * camera before the camera portal has its PipeWire connection. */
	settings = obs_data_create();
	obs_data_set_string(settings, "device_id", CAMERA_NAME);
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
		obs_shutdown();
		camera_portal_unload();
	}
	camera_stop();
	/* The bus is not torn down: the portal code keeps its connection,
	 * which g_test_dbus_down() would wait for. GTestDBus stops the bus
	 * when the process exits. */
	return 0;
}

/* Guard: the camera shows up in the camera portal's device list. */
static void camera_is_listed(void **state)
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

static void registry_events_wait_for_the_connection(void **state)
{
	int registry_reads, stream_connects;

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();

	assert_true(wait_for_found());

	pthread_mutex_lock(&hooks_mutex);
	registry_reads = null_registry_reads;
	stream_connects = null_stream_connects;
	pthread_mutex_unlock(&hooks_mutex);

	fprintf(stderr,
		"calls with no connection yet: %d obs_pipewire_get_registry(), %d obs_pipewire_connect_stream()\n",
		registry_reads, stream_connects);
	if (registry_reads || stream_connects) {
		fprintf(stderr, "registry events reached the camera portal before it had its PipeWire connection\n");
		fail();
	}
}

static void saved_source_streams_the_camera(void **state)
{
	int connected;

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();

	assert_true(wait_for_found());

	connected = wait_for_streams_connected();
	fprintf(stderr, "streams connected for the source: %d\n", connected);
	if (!connected) {
		fprintf(stderr, "the source set to \"%s\" never connected a stream\n", CAMERA_NAME);
		fail();
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(camera_is_listed),
		cmocka_unit_test(registry_events_wait_for_the_connection),
		cmocka_unit_test(saved_source_streams_the_camera),
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
