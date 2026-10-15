#include <errno.h>
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

/* Picking another resolution for a streaming PipeWire camera in its
 * Properties window renegotiated the running stream in place
 * (obs_pipewire_stream_set_resolution()). A camera behind libcamera cannot
 * be reconfigured while it delivers: with PipeWire's libcamera plugin
 * before 1.4.9 the stream dies ("Broken pipe", then "unconnected"; with
 * PipeWire 1.0.5 the daemon itself crashes). Seen on a Surface Pro 5 (IPU3)
 * in obs-rust/obs-studio#390, upstream obsproject/obs-studio#10399. The
 * same happened when the format list dropped a framerate kept from before:
 * obs_pipewire_stream_set_framerate(NULL) on the running stream.
 *
 * End to end: the real camera portal code against a fake
 * org.freedesktop.portal.Camera on a private D-Bus bus (GTestDBus), with its
 * PipeWire connection on a private PipeWire daemon that
 * test_pipewire_cursor_hdr.sh starts (own runtime dir, no session manager, no
 * devices). A stream node in this test plays a libcamera camera: YUY2 at
 * 64x48 or 32x24, one EnumFormat per size and no framerates (like PipeWire's
 * libcamera plugin), and like libcamera it refuses a new format while it
 * runs with one (the stream errors out). There is no session manager, so the test
 * links the camera to the newest capture stream itself. The source's log
 * tells what happens: "Created stream", "Renegotiating stream" and the
 * negotiated size.
 *
 * Needs dbus-daemon and an X display for obs_startup() (xvfb-run is enough);
 * the wrapper skips (77) without PipeWire, and the cases skip when
 * obs_startup() or obs_reset_video() fails. */

#define SKIP_EXIT 77

#define CAMERA_NAME "obs-test-camera"
#define WIDTH 64
#define HEIGHT 48
#define SMALL_WIDTH 32
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
/* The camera: a driving video source node on the private daemon       */

static struct {
	struct pw_thread_loop *loop;
	struct pw_context *context;
	struct pw_core *core;
	struct pw_registry *registry;
	struct spa_hook registry_listener;
	struct pw_stream *stream;
	struct spa_hook stream_listener;
	struct spa_source *timer;
	uint32_t node;

	/* Written on the camera's PipeWire thread, read under its lock. */
	uint32_t capture_node;
	struct {
		uint32_t id, node;
		bool out;
	} ports[64];
	size_t n_ports;
	struct pw_proxy *link;
	uint32_t linked_out, linked_in;

	bool streaming;
	bool has_format;
	uint32_t width, height;
	int refused; /* new formats asked for while streaming */
	bool failed;
} camera;

static void on_camera_state_changed(void *data, enum pw_stream_state old, enum pw_stream_state state, const char *error)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(old);
	UNUSED_PARAMETER(error);

	camera.streaming = state == PW_STREAM_STATE_STREAMING;
}

static void on_camera_param_changed(void *data, uint32_t id, const struct spa_pod *param)
{
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	struct spa_video_info_raw info = {0};
	const struct spa_pod *params[1];

	UNUSED_PARAMETER(data);

	if (id != SPA_PARAM_Format)
		return;
	if (!param) {
		camera.has_format = false;
		return;
	}

	/* libcamera: "Camera in Running state trying configure()". PipeWire's
	 * libcamera plugin before 1.4.9 keeps the camera running until its
	 * format is cleared, so a second format without a clear in between is
	 * refused. A new link starts with a clear. */
	if (camera.has_format) {
		camera.refused++;
		camera.failed = true;
		pw_stream_set_error(camera.stream, -EINVAL, "camera in Running state trying configure()");
		return;
	}

	spa_format_video_raw_parse(param, &info);
	camera.has_format = true;
	camera.width = info.size.width;
	camera.height = info.size.height;

	params[0] = spa_pod_builder_add_object(&b, SPA_TYPE_OBJECT_ParamBuffers, SPA_PARAM_Buffers,
					       SPA_PARAM_BUFFERS_buffers, SPA_POD_CHOICE_RANGE_Int(4, 2, 8),
					       SPA_PARAM_BUFFERS_blocks, SPA_POD_Int(1), SPA_PARAM_BUFFERS_size,
					       SPA_POD_Int(WIDTH * HEIGHT * 2), SPA_PARAM_BUFFERS_stride,
					       SPA_POD_Int(info.size.width * 2), SPA_PARAM_BUFFERS_dataType,
					       SPA_POD_CHOICE_FLAGS_Int(1 << SPA_DATA_MemPtr));
	pw_stream_update_params(camera.stream, params, 1);
}

static void on_camera_process(void *data)
{
	struct pw_buffer *b;
	struct spa_data *d;
	uint32_t size = camera.width * camera.height * 2;

	UNUSED_PARAMETER(data);

	b = pw_stream_dequeue_buffer(camera.stream);
	if (!b)
		return;

	d = &b->buffer->datas[0];
	if (d->data && size && d->maxsize >= size) {
		memset(d->data, 0x80, size);
		*d->chunk = (struct spa_chunk){.offset = 0, .size = size, .stride = (int32_t)camera.width * 2};
	}
	pw_stream_queue_buffer(camera.stream, b);
}

static const struct pw_stream_events camera_events = {
	PW_VERSION_STREAM_EVENTS,
	.state_changed = on_camera_state_changed,
	.param_changed = on_camera_param_changed,
	.process = on_camera_process,
};

static void on_timeout(void *data, uint64_t expirations)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(expirations);

	if (camera.stream)
		pw_stream_trigger_process(camera.stream);
}

static const char *dict_str(const struct spa_dict *props, const char *key)
{
	const char *str = props ? spa_dict_lookup(props, key) : NULL;

	return str ? str : "";
}

static uint32_t dict_uint(const struct spa_dict *props, const char *key)
{
	const char *str = props ? spa_dict_lookup(props, key) : NULL;

	return str ? (uint32_t)strtoul(str, NULL, 10) : SPA_ID_INVALID;
}

static void on_registry_global(void *data, uint32_t id, uint32_t permissions, const char *type, uint32_t version,
			       const struct spa_dict *props)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(permissions);
	UNUSED_PARAMETER(version);

	if (strcmp(type, PW_TYPE_INTERFACE_Node) == 0) {
		/* The camera source's capture stream; the latest one wins. */
		if (strcmp(dict_str(props, PW_KEY_MEDIA_CLASS), "Stream/Input/Video") == 0)
			camera.capture_node = id;
	} else if (strcmp(type, PW_TYPE_INTERFACE_Port) == 0 && camera.n_ports < SPA_N_ELEMENTS(camera.ports)) {
		camera.ports[camera.n_ports].id = id;
		camera.ports[camera.n_ports].node = dict_uint(props, PW_KEY_NODE_ID);
		camera.ports[camera.n_ports].out = strcmp(dict_str(props, PW_KEY_PORT_DIRECTION), "out") == 0;
		camera.n_ports++;
	}
}

/* PipeWire reuses ids: forget removed ports and nodes. */
static void on_registry_global_remove(void *data, uint32_t id)
{
	UNUSED_PARAMETER(data);

	if (camera.capture_node == id)
		camera.capture_node = SPA_ID_INVALID;
	for (size_t i = 0; i < camera.n_ports;) {
		if (camera.ports[i].id == id || camera.ports[i].node == id)
			camera.ports[i] = camera.ports[--camera.n_ports];
		else
			i++;
	}
	if (camera.linked_out == id || camera.linked_in == id)
		camera.linked_out = camera.linked_in = SPA_ID_INVALID;
}

static const struct pw_registry_events registry_events = {
	PW_VERSION_REGISTRY_EVENTS,
	.global = on_registry_global,
	.global_remove = on_registry_global_remove,
};

static uint32_t find_port(uint32_t node, bool out)
{
	for (size_t i = camera.n_ports; i > 0; i--) {
		if (camera.ports[i - 1].node == node && camera.ports[i - 1].out == out)
			return camera.ports[i - 1].id;
	}
	return SPA_ID_INVALID;
}

static bool camera_start(void)
{
	struct timespec interval = {0, 1000000000 / 30};
	struct timespec first = {0, 1};

	pw_init(NULL, NULL);

	camera.capture_node = SPA_ID_INVALID;
	camera.linked_out = camera.linked_in = SPA_ID_INVALID;
	camera.loop = pw_thread_loop_new("camera-resolution-test", NULL);
	camera.context = pw_context_new(pw_thread_loop_get_loop(camera.loop), NULL, 0);
	if (pw_thread_loop_start(camera.loop) < 0)
		return false;

	pw_thread_loop_lock(camera.loop);
	camera.core = pw_context_connect(camera.context, NULL, 0);
	if (camera.core) {
		camera.registry = pw_core_get_registry(camera.core, PW_VERSION_REGISTRY, 0);
		pw_registry_add_listener(camera.registry, &camera.registry_listener, &registry_events, NULL);
		camera.timer = pw_loop_add_timer(pw_thread_loop_get_loop(camera.loop), on_timeout, NULL);
		pw_loop_update_timer(pw_thread_loop_get_loop(camera.loop), camera.timer, &first, &interval, false);
	}
	pw_thread_loop_unlock(camera.loop);
	return camera.core != NULL;
}

static const struct spa_pod *enum_format(struct spa_pod_builder *b, uint32_t width, uint32_t height)
{
	return spa_pod_builder_add_object(b, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat, SPA_FORMAT_mediaType,
					  SPA_POD_Id(SPA_MEDIA_TYPE_video), SPA_FORMAT_mediaSubtype,
					  SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw), SPA_FORMAT_VIDEO_format,
					  SPA_POD_Id(SPA_VIDEO_FORMAT_YUY2), SPA_FORMAT_VIDEO_size,
					  SPA_POD_Rectangle(&SPA_RECTANGLE(width, height)));
}

/* YUY2 64x48 (the default) or 32x24, no framerates. */
static bool camera_plug(void)
{
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[2];
	uint32_t node_id = SPA_ID_INVALID;

	params[0] = enum_format(&b, WIDTH, HEIGHT);
	params[1] = enum_format(&b, SMALL_WIDTH, SMALL_HEIGHT);

	pw_thread_loop_lock(camera.loop);
	camera.streaming = camera.has_format = camera.failed = false;
	camera.stream =
		pw_stream_new(camera.core, CAMERA_NAME,
			      pw_properties_new(PW_KEY_NODE_NAME, CAMERA_NAME, PW_KEY_NODE_DESCRIPTION, "Test Camera",
						PW_KEY_MEDIA_CLASS, "Video/Source", PW_KEY_MEDIA_ROLE, "Camera", NULL));
	pw_stream_add_listener(camera.stream, &camera.stream_listener, &camera_events, NULL);
	pw_stream_connect(camera.stream, PW_DIRECTION_OUTPUT, PW_ID_ANY,
			  PW_STREAM_FLAG_DRIVER | PW_STREAM_FLAG_MAP_BUFFERS, params, 2);
	pw_thread_loop_unlock(camera.loop);

	for (int i = 0; i < 200 && node_id == SPA_ID_INVALID; i++) {
		os_sleep_ms(10);
		pw_thread_loop_lock(camera.loop);
		node_id = pw_stream_get_node_id(camera.stream);
		pw_thread_loop_unlock(camera.loop);
	}
	camera.node = node_id;
	return node_id != SPA_ID_INVALID;
}

static void camera_unplug(void)
{
	pw_thread_loop_lock(camera.loop);
	if (camera.link) {
		pw_proxy_destroy(camera.link);
		camera.link = NULL;
	}
	g_clear_pointer(&camera.stream, pw_stream_destroy);
	camera.node = SPA_ID_INVALID;
	camera.linked_out = camera.linked_in = SPA_ID_INVALID;
	pw_thread_loop_unlock(camera.loop);
}

/* No session manager: link the camera to the newest capture stream. */
static void camera_link(void)
{
	pw_thread_loop_lock(camera.loop);
	uint32_t out_port = find_port(camera.node, true);
	uint32_t in_port = find_port(camera.capture_node, false);

	if (out_port != SPA_ID_INVALID && in_port != SPA_ID_INVALID &&
	    (out_port != camera.linked_out || in_port != camera.linked_in)) {
		struct pw_properties *props = pw_properties_new(NULL, NULL);

		if (camera.link)
			pw_proxy_destroy(camera.link);
		pw_properties_setf(props, PW_KEY_LINK_OUTPUT_NODE, "%u", camera.node);
		pw_properties_setf(props, PW_KEY_LINK_OUTPUT_PORT, "%u", out_port);
		pw_properties_setf(props, PW_KEY_LINK_INPUT_NODE, "%u", camera.capture_node);
		pw_properties_setf(props, PW_KEY_LINK_INPUT_PORT, "%u", in_port);
		camera.link = pw_core_create_object(camera.core, "link-factory", PW_TYPE_INTERFACE_Link,
						    PW_VERSION_LINK, &props->dict, 0);
		pw_properties_free(props);
		camera.linked_out = out_port;
		camera.linked_in = in_port;
	}
	pw_thread_loop_unlock(camera.loop);
}

static void camera_stop(void)
{
	if (!camera.loop)
		return;
	pw_thread_loop_lock(camera.loop);
	if (camera.timer)
		pw_loop_destroy_source(pw_thread_loop_get_loop(camera.loop), camera.timer);
	if (camera.link)
		pw_proxy_destroy(camera.link);
	if (camera.registry)
		pw_proxy_destroy((struct pw_proxy *)camera.registry);
	g_clear_pointer(&camera.stream, pw_stream_destroy);
	g_clear_pointer(&camera.core, pw_core_disconnect);
	pw_thread_loop_unlock(camera.loop);
	pw_thread_loop_stop(camera.loop);
	pw_context_destroy(camera.context);
	pw_thread_loop_destroy(camera.loop);
	memset(&camera, 0, sizeof(camera));
}

/* ------------------------------------------------------------------ */
/* Log: what the camera source's stream does                           */

static pthread_mutex_t log_mutex = PTHREAD_MUTEX_INITIALIZER;
static struct {
	bool found;
	int created;      /* "Created stream" lines: a new stream */
	int negotiated;   /* "Negotiated format:" lines */
	int renegotiated; /* "Renegotiating stream" lines */
	uint32_t width;   /* the last negotiated size */
	uint32_t height;
} seen;

static void log_handler(int level, const char *format, va_list args, void *param)
{
	char line[256];
	unsigned int width, height;

	UNUSED_PARAMETER(param);

	vsnprintf(line, sizeof(line), format, args);

	pthread_mutex_lock(&log_mutex);
	if (strcmp(line, "[camera-portal] Found device " CAMERA_NAME) == 0)
		seen.found = true;
	if (strncmp(line, "[pipewire] Created stream", 25) == 0)
		seen.created++;
	if (strcmp(line, "[pipewire] Negotiated format:") == 0)
		seen.negotiated++;
	if (strcmp(line, "[pipewire] Renegotiating stream") == 0)
		seen.renegotiated++;
	if (sscanf(line, "[pipewire]     Size: %ux%u", &width, &height) == 2) {
		seen.width = width;
		seen.height = height;
	}
	pthread_mutex_unlock(&log_mutex);

	if (level <= LOG_WARNING || getenv("CAMERA_RESOLUTION_TEST_VERBOSE"))
		fprintf(stderr, "%s\n", line);
}

static void iterate_main_loop(uint32_t ms)
{
	uint64_t end = os_gettime_ns() + ms * 1000000ULL;

	while (os_gettime_ns() < end) {
		while (g_main_context_iteration(NULL, FALSE))
			;
		camera_link();
		os_sleep_ms(1);
	}
}

static int seen_count(const int *counter)
{
	int n;

	pthread_mutex_lock(&log_mutex);
	n = *counter;
	pthread_mutex_unlock(&log_mutex);
	return n;
}

static bool wait_for_found(void)
{
	for (int i = 0; i < 250; i++) {
		bool found;

		pthread_mutex_lock(&log_mutex);
		found = seen.found;
		pthread_mutex_unlock(&log_mutex);
		if (found)
			return true;
		iterate_main_loop(20);
	}
	return false;
}

/* Waits until the stream has negotiated more than `count` times. */
static bool wait_for_negotiated(int count)
{
	for (int i = 0; i < 250; i++) {
		if (seen_count(&seen.negotiated) > count)
			return true;
		iterate_main_loop(20);
	}
	return false;
}

static bool negotiated_size_is(uint32_t width, uint32_t height)
{
	bool is;

	pthread_mutex_lock(&log_mutex);
	is = seen.width == width && seen.height == height;
	if (!is)
		fprintf(stderr, "negotiated size %ux%u, expected %ux%u\n", seen.width, seen.height, width, height);
	pthread_mutex_unlock(&log_mutex);
	return is;
}

static int camera_refused(void)
{
	int n;

	pw_thread_loop_lock(camera.loop);
	n = camera.refused;
	pw_thread_loop_unlock(camera.loop);
	return n;
}

/* The camera streams at width x height to the source (after a refusal it
 * had errored out, like a libcamera node). */
static bool camera_streams_at(uint32_t width, uint32_t height)
{
	for (int i = 0; i < 100; i++) {
		bool ok;

		pw_thread_loop_lock(camera.loop);
		ok = !camera.failed && camera.streaming && camera.has_format && camera.width == width &&
		     camera.height == height;
		pw_thread_loop_unlock(camera.loop);
		if (ok)
			return true;
		iterate_main_loop(20);
	}
	return false;
}

/* ------------------------------------------------------------------ */
/* The camera source                                                  */

static char *format_json(uint32_t width, uint32_t height)
{
	obs_data_t *data = obs_data_create();
	char *json;

	obs_data_set_bool(data, "encoded", false);
	obs_data_set_int(data, "video_format", SPA_VIDEO_FORMAT_YUY2);
	obs_data_set_int(data, "width", width);
	obs_data_set_int(data, "height", height);
	json = bstrdup(obs_data_get_json(data));
	obs_data_release(data);
	return json;
}

/* A camera source with the camera and its format selected, and a
 * framerate kept from before (from a camera or PipeWire version that listed
 * framerates): this camera lists none. */
static obs_source_t *create_camera_source(void)
{
	struct media_frames_per_second fps = {.numerator = 15, .denominator = 1};
	obs_data_t *settings = obs_data_create();
	obs_data_t *framerate = obs_data_create();
	char *format = format_json(WIDTH, HEIGHT);
	obs_source_t *source;

	obs_data_set_frames_per_second(framerate, "framerate", fps, NULL);
	obs_data_set_string(settings, "device_id", CAMERA_NAME);
	obs_data_set_string(settings, "format", format);
	obs_data_set_string(settings, "framerate", obs_data_get_json(framerate));
	source = obs_source_create("pipewire-camera-source", "camera", settings, NULL);
	obs_data_release(framerate);
	obs_data_release(settings);
	bfree(format);
	return source;
}

/* What the Properties window does when it opens or reloads. */
static void open_properties(obs_source_t *source)
{
	obs_properties_t *props = obs_source_properties(source);

	obs_properties_destroy(props);
}

/* The user picks another entry in the Properties window's format list. */
static void pick_format(obs_source_t *source, uint32_t width, uint32_t height)
{
	obs_properties_t *props = obs_source_properties(source);
	obs_data_t *settings = obs_source_get_settings(source);
	char *json = format_json(width, height);

	obs_data_set_string(settings, "format", json);
	obs_property_modified(obs_properties_get(props, "format"), settings);
	obs_source_update(source, settings);

	bfree(json);
	obs_data_release(settings);
	obs_properties_destroy(props);
}

/* A camera that refused a format errored out: plug it in again, which the
 * source follows with a new stream. */
static bool revive_camera(void)
{
	bool failed;
	int before = seen_count(&seen.negotiated);

	pw_thread_loop_lock(camera.loop);
	failed = camera.failed;
	pw_thread_loop_unlock(camera.loop);
	if (!failed)
		return true;

	camera_unplug();
	iterate_main_loop(300);
	if (!camera_plug())
		return false;
	return wait_for_negotiated(before);
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

	/* One source for all cases, which run in order: the camera portal
	 * follows the camera (replugs) only for the source that opened its
	 * connection. */
	camera_source = create_camera_source();
	if (camera_source)
		wait_for_negotiated(0);

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

/* Guard: the camera is listed and the source streams at the camera's
 * default size. */
static void camera_streams(void **state)
{
	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();

	assert_non_null(camera_source);
	assert_true(wait_for_found());
	if (!wait_for_negotiated(0)) {
		fprintf(stderr, "the camera source's stream never negotiated a format\n");
		fail();
	}
	assert_true(negotiated_size_is(WIDTH, HEIGHT));
	assert_true(camera_streams_at(WIDTH, HEIGHT));
}

/* The camera lists no framerates, but the source kept one from before.
 * Opening the Properties window drops it: that must not ask the streaming
 * camera for a new format in place. */
static void dropped_framerate_does_not_reconfigure(void **state)
{
	int refused, changes;

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	assert_non_null(camera_source);
	assert_true(camera_streams_at(WIDTH, HEIGHT));

	refused = camera_refused();
	open_properties(camera_source);
	iterate_main_loop(500);

	if (camera_refused() != refused) {
		fprintf(stderr, "opening Properties asked the streaming camera for a new format %d time(s) in place\n",
			camera_refused() - refused);
		fail();
	}
	if (!camera_streams_at(WIDTH, HEIGHT)) {
		fprintf(stderr, "the camera no longer streams to the source\n");
		fail();
	}

	/* Only once: the next Properties pass leaves the stream alone. */
	changes = seen_count(&seen.created) + seen_count(&seen.renegotiated);
	open_properties(camera_source);
	iterate_main_loop(500);
	assert_int_equal(seen_count(&seen.created) + seen_count(&seen.renegotiated), changes);
}

/* The user picks 32x24 in the format list while the camera streams at
 * 64x48. The source must start a new stream at 32x24 instead of asking the
 * running camera for the new size in place. */
static void resolution_change_restarts_the_stream(void **state)
{
	int created, negotiated, refused, renegotiated;

	UNUSED_PARAMETER(state);

	if (!obs_up)
		skip();
	assert_non_null(camera_source);
	assert_true(revive_camera());
	assert_true(camera_streams_at(WIDTH, HEIGHT));

	created = seen_count(&seen.created);
	negotiated = seen_count(&seen.negotiated);
	renegotiated = seen_count(&seen.renegotiated);
	refused = camera_refused();

	pick_format(camera_source, SMALL_WIDTH, SMALL_HEIGHT);
	wait_for_negotiated(negotiated);
	iterate_main_loop(300);

	if (camera_refused() != refused) {
		fprintf(stderr, "the streaming camera was asked for 32x24 in place %d time(s)\n",
			camera_refused() - refused);
		fail();
	}
	if (seen_count(&seen.renegotiated) != renegotiated) {
		fprintf(stderr, "the running stream was renegotiated %d time(s)\n",
			seen_count(&seen.renegotiated) - renegotiated);
		fail();
	}
	if (seen_count(&seen.created) != created + 1) {
		fprintf(stderr, "%d new stream(s), expected 1\n", seen_count(&seen.created) - created);
		fail();
	}
	assert_true(negotiated_size_is(SMALL_WIDTH, SMALL_HEIGHT));
	if (!camera_streams_at(SMALL_WIDTH, SMALL_HEIGHT)) {
		fprintf(stderr, "the camera does not stream at 32x24\n");
		fail();
	}

	/* And back. */
	created = seen_count(&seen.created);
	negotiated = seen_count(&seen.negotiated);
	pick_format(camera_source, WIDTH, HEIGHT);
	wait_for_negotiated(negotiated);
	iterate_main_loop(300);
	assert_int_equal(camera_refused(), refused);
	assert_int_equal(seen_count(&seen.created), created + 1);
	assert_true(negotiated_size_is(WIDTH, HEIGHT));
	assert_true(camera_streams_at(WIDTH, HEIGHT));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(camera_streams),
		cmocka_unit_test(dropped_framerate_does_not_reconfigure),
		cmocka_unit_test(resolution_change_restarts_the_stream),
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
