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
#include <util/platform.h>
#include <util/threading.h>

#include "screencast-portal.h"

/* PipeWire screen capture of frames in shared memory (MemPtr/MemFd
 * buffers, what a compositor sends when it does not use DMA-BUF) ignores
 * the buffer's chunk: its stride, its offset and its size.
 * obsproject/obs-studio#4961 (and #6408, #6944): the capture is skewed or
 * distorted; obsproject/obs-studio#13318 (and #13750): OBS crashes in
 * memmove on the PipeWire thread with a producer sending buffers OBS did
 * not expect.
 *
 * End to end: the real screencast portal source talks to a fake
 * org.freedesktop.portal.ScreenCast on a private D-Bus bus (GTestDBus) and
 * receives its stream from a private PipeWire daemon that
 * test_pipewire_cursor_hdr.sh starts for it (own runtime dir, no session
 * manager, no devices). A producer stream in this test plays the
 * compositor: it sends BGRx frames, black on the left half and white on the
 * right, with the rows, offset and chunk size a case asks for; bytes that
 * are not pixels (row padding, the space before the offset) are mid grey.
 * The capture is the only source on the output, which is read back as the
 * luma plane of NV12 frames through a raw video callback.
 *
 * Needs dbus-daemon and an X display for obs_startup() (xvfb-run is enough);
 * the wrapper skips (77) without PipeWire, and the cases skip when
 * obs_startup() fails. */

#define SKIP_EXIT 77

#define FRAME_SIZE 128
#define ROW_BYTES (FRAME_SIZE * 4)
#define PADDED_STRIDE (ROW_BYTES + 64) /* 16 pixels of padding per row */
#define OFFSET 4096
/* Room for the largest layout: a padded frame after the offset. */
#define BUFFER_SIZE (OFFSET + PADDED_STRIDE * FRAME_SIZE)

#define BLACK_MAX 40 /* NV12 limited range: black 16, grey 0x80 ~126, white 235 */
#define WHITE_MIN 200

const char *obs_module_text(const char *val)
{
	return val;
}

/* ------------------------------------------------------------------ */
/* Fake org.freedesktop.portal.ScreenCast, served from its own thread  */

static const char portal_xml[] = "<node>"
				 " <interface name='org.freedesktop.portal.ScreenCast'>"
				 "  <method name='CreateSession'>"
				 "   <arg type='a{sv}' direction='in'/><arg type='o' direction='out'/>"
				 "  </method>"
				 "  <method name='SelectSources'>"
				 "   <arg type='o' direction='in'/><arg type='a{sv}' direction='in'/>"
				 "   <arg type='o' direction='out'/>"
				 "  </method>"
				 "  <method name='Start'>"
				 "   <arg type='o' direction='in'/><arg type='s' direction='in'/>"
				 "   <arg type='a{sv}' direction='in'/><arg type='o' direction='out'/>"
				 "  </method>"
				 "  <method name='OpenPipeWireRemote'>"
				 "   <arg type='o' direction='in'/><arg type='a{sv}' direction='in'/>"
				 "   <arg type='h' direction='out'/>"
				 "  </method>"
				 "  <property name='AvailableSourceTypes' type='u' access='read'/>"
				 "  <property name='AvailableCursorModes' type='u' access='read'/>"
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

static uint32_t producer_node_id = SPA_ID_INVALID;

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

	options = g_variant_get_child_value(parameters, g_variant_n_children(parameters) - 1);
	g_variant_lookup(options, "handle_token", "&s", &handle_token);
	request_path = g_strdup_printf("/org/freedesktop/portal/desktop/request/%s/%s", sender_part, handle_token);
	g_dbus_method_invocation_return_value(invocation, g_variant_new("(o)", request_path));

	g_variant_builder_init(&results, G_VARIANT_TYPE_VARDICT);
	if (strcmp(method_name, "CreateSession") == 0) {
		const char *session_token = NULL;
		g_autofree char *session_path = NULL;

		g_variant_lookup(options, "session_handle_token", "&s", &session_token);
		session_path =
			g_strdup_printf("/org/freedesktop/portal/desktop/session/%s/%s", sender_part, session_token);
		g_variant_builder_add(&results, "{sv}", "session_handle", g_variant_new_string(session_path));
	} else if (strcmp(method_name, "Start") == 0) {
		GVariantBuilder streams;
		GVariantBuilder props;

		g_variant_builder_init(&props, G_VARIANT_TYPE_VARDICT);
		g_variant_builder_add(&props, "{sv}", "size", g_variant_new("(ii)", FRAME_SIZE, FRAME_SIZE));
		g_variant_builder_add(&props, "{sv}", "source_type", g_variant_new_uint32(1));
		g_variant_builder_init(&streams, G_VARIANT_TYPE("a(ua{sv})"));
		g_variant_builder_add(&streams, "(u@a{sv})", producer_node_id, g_variant_builder_end(&props));
		g_variant_builder_add(&results, "{sv}", "streams", g_variant_builder_end(&streams));
	}
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

	if (strcmp(property_name, "AvailableSourceTypes") == 0)
		return g_variant_new_uint32(3);
	if (strcmp(property_name, "AvailableCursorModes") == 0)
		return g_variant_new_uint32(1 | 2 | 4); /* hidden, embedded, metadata */
	return g_variant_new_uint32(4);
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
/* The "compositor": a PipeWire producer sending frames in memory      */

struct layout {
	uint32_t stride; /* bytes from one row to the next */
	uint32_t offset; /* chunk offset */
	uint32_t rows;   /* rows covered by the chunk size */
	bool inverted;   /* white on the left, black on the right */
};

struct producer {
	struct pw_thread_loop *loop;
	struct pw_context *context;
	struct pw_core *core;
	struct pw_stream *stream;
	struct spa_hook stream_listener;
	struct pw_registry *registry;
	struct spa_hook registry_listener;
	struct spa_source *timer;

	/* Written on the PipeWire thread, read under the loop lock. */
	uint32_t capture_node;
	struct {
		uint32_t id, node;
		bool out;
	} ports[16];
	size_t n_ports;
	struct pw_proxy *link;

	/* Set under the loop lock. */
	struct layout layout;
};

static struct producer producer;

static void on_producer_process(void *data)
{
	const struct layout *l = &producer.layout;
	struct pw_buffer *b;
	struct spa_data *d;

	UNUSED_PARAMETER(data);

	b = pw_stream_dequeue_buffer(producer.stream);
	if (!b)
		return;

	d = &b->buffer->datas[0];
	if (d->data && d->maxsize >= l->offset + l->stride * FRAME_SIZE) {
		uint8_t *base = d->data;

		memset(base, 0x80, d->maxsize);
		for (uint32_t y = 0; y < FRAME_SIZE; y++) {
			uint8_t *row = base + l->offset + (size_t)y * l->stride;

			memset(row, l->inverted ? 0xff : 0x00, ROW_BYTES / 2);
			memset(row + ROW_BYTES / 2, l->inverted ? 0x00 : 0xff, ROW_BYTES / 2);
		}
		d->chunk->offset = l->offset;
		d->chunk->stride = (int32_t)l->stride;
		d->chunk->size = l->stride * l->rows;
		d->chunk->flags = 0;
	}

	pw_stream_queue_buffer(producer.stream, b);
}

static void on_producer_param_changed(void *data, uint32_t id, const struct spa_pod *param)
{
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];

	UNUSED_PARAMETER(data);

	if (!param || id != SPA_PARAM_Format)
		return;

	params[0] = spa_pod_builder_add_object(&b, SPA_TYPE_OBJECT_ParamBuffers, SPA_PARAM_Buffers,
					       SPA_PARAM_BUFFERS_buffers, SPA_POD_CHOICE_RANGE_Int(4, 2, 8),
					       SPA_PARAM_BUFFERS_blocks, SPA_POD_Int(1), SPA_PARAM_BUFFERS_size,
					       SPA_POD_Int(BUFFER_SIZE), SPA_PARAM_BUFFERS_stride,
					       SPA_POD_Int(PADDED_STRIDE));
	pw_stream_update_params(producer.stream, params, 1);
}

static const struct pw_stream_events producer_events = {
	PW_VERSION_STREAM_EVENTS,
	.param_changed = on_producer_param_changed,
	.process = on_producer_process,
};

static void on_timeout(void *data, uint64_t expirations)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(expirations);

	pw_stream_trigger_process(producer.stream);
}

static uint32_t dict_uint(const struct spa_dict *props, const char *key)
{
	const char *str = props ? spa_dict_lookup(props, key) : NULL;

	return str ? (uint32_t)strtoul(str, NULL, 10) : SPA_ID_INVALID;
}

static const char *dict_str(const struct spa_dict *props, const char *key)
{
	const char *str = props ? spa_dict_lookup(props, key) : NULL;

	return str ? str : "";
}

static void on_registry_global(void *data, uint32_t id, uint32_t permissions, const char *type, uint32_t version,
			       const struct spa_dict *props)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(permissions);
	UNUSED_PARAMETER(version);

	if (strcmp(type, PW_TYPE_INTERFACE_Node) == 0) {
		if (strcmp(dict_str(props, PW_KEY_MEDIA_CLASS), "Stream/Input/Video") == 0)
			producer.capture_node = id;
	} else if (strcmp(type, PW_TYPE_INTERFACE_Port) == 0 && producer.n_ports < SPA_N_ELEMENTS(producer.ports)) {
		producer.ports[producer.n_ports].id = id;
		producer.ports[producer.n_ports].node = dict_uint(props, PW_KEY_NODE_ID);
		producer.ports[producer.n_ports].out = strcmp(dict_str(props, PW_KEY_PORT_DIRECTION), "out") == 0;
		producer.n_ports++;
	}
}

static uint32_t find_port(uint32_t node, bool out)
{
	for (size_t i = 0; i < producer.n_ports; i++) {
		if (producer.ports[i].node == node && producer.ports[i].out == out)
			return producer.ports[i].id;
	}
	return SPA_ID_INVALID;
}

static const struct pw_registry_events registry_events = {
	PW_VERSION_REGISTRY_EVENTS,
	.global = on_registry_global,
};

static void producer_set_layout(struct layout layout)
{
	pw_thread_loop_lock(producer.loop);
	producer.layout = layout;
	pw_thread_loop_unlock(producer.loop);
}

static bool producer_start(void)
{
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];
	struct timespec interval = {0, 1000000000 / 30};
	struct timespec first = {0, 1};

	pw_init(NULL, NULL);
	producer.capture_node = SPA_ID_INVALID;
	producer.layout = (struct layout){ROW_BYTES, 0, FRAME_SIZE, false};

	producer.loop = pw_thread_loop_new("shm-layout-producer", NULL);
	producer.context = pw_context_new(pw_thread_loop_get_loop(producer.loop), NULL, 0);
	if (pw_thread_loop_start(producer.loop) < 0)
		return false;

	pw_thread_loop_lock(producer.loop);
	producer.core = pw_context_connect(producer.context, NULL, 0);
	if (!producer.core) {
		pw_thread_loop_unlock(producer.loop);
		return false;
	}

	producer.stream =
		pw_stream_new(producer.core, "shm-layout-producer",
			      pw_properties_new(PW_KEY_MEDIA_CLASS, "Video/Source", PW_KEY_MEDIA_TYPE, "Video", NULL));
	pw_stream_add_listener(producer.stream, &producer.stream_listener, &producer_events, NULL);

	params[0] = spa_pod_builder_add_object(&b, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat, SPA_FORMAT_mediaType,
					       SPA_POD_Id(SPA_MEDIA_TYPE_video), SPA_FORMAT_mediaSubtype,
					       SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw), SPA_FORMAT_VIDEO_format,
					       SPA_POD_Id(SPA_VIDEO_FORMAT_BGRx), SPA_FORMAT_VIDEO_size,
					       SPA_POD_Rectangle(&SPA_RECTANGLE(FRAME_SIZE, FRAME_SIZE)),
					       SPA_FORMAT_VIDEO_framerate, SPA_POD_Fraction(&SPA_FRACTION(30, 1)));

	pw_stream_connect(producer.stream, PW_DIRECTION_OUTPUT, PW_ID_ANY,
			  PW_STREAM_FLAG_DRIVER | PW_STREAM_FLAG_MAP_BUFFERS, params, 1);

	producer.registry = pw_core_get_registry(producer.core, PW_VERSION_REGISTRY, 0);
	pw_registry_add_listener(producer.registry, &producer.registry_listener, &registry_events, NULL);

	producer.timer = pw_loop_add_timer(pw_thread_loop_get_loop(producer.loop), on_timeout, NULL);
	pw_loop_update_timer(pw_thread_loop_get_loop(producer.loop), producer.timer, &first, &interval, false);
	pw_thread_loop_unlock(producer.loop);

	for (int i = 0; i < 200; i++) {
		pw_thread_loop_lock(producer.loop);
		producer_node_id = pw_stream_get_node_id(producer.stream);
		pw_thread_loop_unlock(producer.loop);
		if (producer_node_id != SPA_ID_INVALID)
			return true;
		os_sleep_ms(10);
	}
	return false;
}

/* The portal's stream is meant for a session manager to link; this
 * PipeWire has none, so link the producer to the capture directly. */
static bool producer_link_ready(void)
{
	bool linked;

	pw_thread_loop_lock(producer.loop);
	uint32_t out_port = find_port(producer_node_id, true);
	uint32_t in_port = find_port(producer.capture_node, false);

	if (!producer.link && producer.capture_node != SPA_ID_INVALID && out_port != SPA_ID_INVALID &&
	    in_port != SPA_ID_INVALID) {
		struct pw_properties *props = pw_properties_new(NULL, NULL);

		pw_properties_setf(props, PW_KEY_LINK_OUTPUT_NODE, "%u", producer_node_id);
		pw_properties_setf(props, PW_KEY_LINK_OUTPUT_PORT, "%u", out_port);
		pw_properties_setf(props, PW_KEY_LINK_INPUT_NODE, "%u", producer.capture_node);
		pw_properties_setf(props, PW_KEY_LINK_INPUT_PORT, "%u", in_port);
		producer.link = pw_core_create_object(producer.core, "link-factory", PW_TYPE_INTERFACE_Link,
						      PW_VERSION_LINK, &props->dict, 0);
		pw_properties_free(props);
	}
	linked = producer.link != NULL;
	pw_thread_loop_unlock(producer.loop);
	return linked;
}

static void producer_stop(void)
{
	if (!producer.loop)
		return;
	pw_thread_loop_lock(producer.loop);
	if (producer.timer)
		pw_loop_destroy_source(pw_thread_loop_get_loop(producer.loop), producer.timer);
	if (producer.link)
		pw_proxy_destroy(producer.link);
	if (producer.registry)
		pw_proxy_destroy((struct pw_proxy *)producer.registry);
	if (producer.stream)
		pw_stream_destroy(producer.stream);
	if (producer.core)
		pw_core_disconnect(producer.core);
	pw_thread_loop_unlock(producer.loop);
	pw_thread_loop_stop(producer.loop);
	pw_context_destroy(producer.context);
	pw_thread_loop_destroy(producer.loop);
	memset(&producer, 0, sizeof(producer));
}

/* ------------------------------------------------------------------ */
/* Output readback                                                    */

static pthread_mutex_t luma_mutex = PTHREAD_MUTEX_INITIALIZER;
static uint8_t luma[FRAME_SIZE][FRAME_SIZE];
static uint64_t luma_frames;

static void on_raw_video(void *param, struct video_data *frame)
{
	UNUSED_PARAMETER(param);

	pthread_mutex_lock(&luma_mutex);
	for (int y = 0; y < FRAME_SIZE; y++)
		memcpy(luma[y], frame->data[0] + (size_t)y * frame->linesize[0], FRAME_SIZE);
	luma_frames++;
	pthread_mutex_unlock(&luma_mutex);
}

/* ------------------------------------------------------------------ */
/* Fixture                                                            */

static bool obs_up;
static obs_source_t *capture;
static GTestDBus *bus;
static struct portal portal;
static Display *display;

static void log_handler(int level, const char *format, va_list args, void *param)
{
	UNUSED_PARAMETER(param);

	if (level <= LOG_WARNING || getenv("SHM_LAYOUT_TEST_VERBOSE")) {
		vfprintf(stderr, format, args);
		fputc('\n', stderr);
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

static bool reset_video(void)
{
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = FRAME_SIZE,
		.base_height = FRAME_SIZE,
		.output_width = FRAME_SIZE,
		.output_height = FRAME_SIZE,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.adapter = 0,
		.gpu_conversion = true,
		.scale_type = OBS_SCALE_POINT,
	};

	return obs_reset_video(&ovi) == OBS_VIDEO_SUCCESS;
}

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

	if (!producer_start()) {
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
	if (!reset_video()) {
		fprintf(stderr, "obs_reset_video failed: cases skip\n");
		obs_shutdown();
		return 0;
	}
	obs_up = true;

	screencast_portal_load();
	capture = obs_source_create("pipewire-screen-capture-source", "capture", NULL, NULL);
	if (capture)
		obs_set_output_source(0, capture);

	/* CreateSession, SelectSources, Start, OpenPipeWireRemote, then the
	 * capture's stream shows up in the registry. */
	for (int i = 0; i < 250 && capture && !producer_link_ready(); i++)
		iterate_main_loop(20);

	obs_add_raw_video_callback(NULL, on_raw_video, NULL);
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);

	if (obs_up) {
		obs_remove_raw_video_callback(on_raw_video, NULL);
		obs_set_output_source(0, NULL);
		obs_source_release(capture);
		capture = NULL;
		obs_wait_for_destroy_queue();
		iterate_main_loop(100);
		obs_shutdown();
		screencast_portal_unload();
	}
	producer_stop();
	/* The bus is not torn down: the portal code keeps its connection,
	 * which g_test_dbus_down() would wait for. GTestDBus stops the bus
	 * when the process exits. */
	return 0;
}

/* Rows whose left half is not all `left` and right half not all `right`
 * (black or white), and the grey pixels seen. */
struct check {
	int bad_rows;
	int first_bad_row;
	int grey;
};

static bool is_black(uint8_t v)
{
	return v <= BLACK_MAX;
}

static bool is_white(uint8_t v)
{
	return v >= WHITE_MIN;
}

static struct check check_luma(bool inverted)
{
	struct check c = {0, -1, 0};

	pthread_mutex_lock(&luma_mutex);
	for (int y = 0; y < FRAME_SIZE; y++) {
		bool row_ok = true;

		for (int x = 0; x < FRAME_SIZE; x++) {
			uint8_t v = luma[y][x];
			bool left = x < FRAME_SIZE / 2;
			bool want_white = left == inverted;

			if (!is_black(v) && !is_white(v))
				c.grey++;
			if (want_white ? !is_white(v) : !is_black(v))
				row_ok = false;
		}
		if (!row_ok) {
			if (c.first_bad_row < 0)
				c.first_bad_row = y;
			c.bad_rows++;
		}
	}
	pthread_mutex_unlock(&luma_mutex);
	return c;
}

/* Has the producer send `layout`, waits for frames that show it and checks
 * the output: the left half black and the right half white (the other way
 * around if `expect_inverted`), on every row. */
static void expect_output(const char *name, struct layout layout, bool expect_inverted)
{
	struct check c = {0};
	uint64_t start, end = os_gettime_ns() + 5000000000ULL;

	producer_set_layout(layout);

	/* Let a few frames of the new layout reach the output. */
	pthread_mutex_lock(&luma_mutex);
	start = luma_frames;
	pthread_mutex_unlock(&luma_mutex);
	while (os_gettime_ns() < end) {
		uint64_t frames;

		iterate_main_loop(20);
		pthread_mutex_lock(&luma_mutex);
		frames = luma_frames;
		pthread_mutex_unlock(&luma_mutex);
		if (frames >= start + 15)
			break;
	}

	c = check_luma(expect_inverted);
	fprintf(stderr, "%s: %d of %d rows wrong (first %d), %d grey pixels\n", name, c.bad_rows, FRAME_SIZE,
		c.first_bad_row, c.grey);
	if (c.bad_rows != 0 || c.grey != 0) {
		pthread_mutex_lock(&luma_mutex);
		for (int y = 0; y < 4; y++)
			fprintf(stderr, "  row %d: x0 %u x%d %u x%d %u x%d %u\n", y, luma[y][0], FRAME_SIZE / 2 - 1,
				luma[y][FRAME_SIZE / 2 - 1], FRAME_SIZE / 2, luma[y][FRAME_SIZE / 2], FRAME_SIZE - 1,
				luma[y][FRAME_SIZE - 1]);
		pthread_mutex_unlock(&luma_mutex);
		fail();
	}
}

static void require_capture(void)
{
	if (!obs_up)
		skip();
	assert_non_null(capture);
	assert_true(producer_link_ready());
}

/* Guard: rows packed back to back, no offset. */
static void packed_rows(void **state)
{
	UNUSED_PARAMETER(state);
	require_capture();
	expect_output("packed rows", (struct layout){ROW_BYTES, 0, FRAME_SIZE, false}, false);
	expect_output("packed rows, inverted", (struct layout){ROW_BYTES, 0, FRAME_SIZE, true}, true);
}

/* Rows padded to a larger stride (the chunk says so): read row by row. */
static void padded_rows(void **state)
{
	UNUSED_PARAMETER(state);
	require_capture();
	expect_output("padded rows", (struct layout){PADDED_STRIDE, 0, FRAME_SIZE, false}, false);
}

/* The frame starts at the chunk's offset into the buffer. */
static void chunk_offset(void **state)
{
	UNUSED_PARAMETER(state);
	require_capture();
	expect_output("chunk offset", (struct layout){ROW_BYTES, OFFSET, FRAME_SIZE, false}, false);
}

/* Both at once. */
static void padded_rows_at_offset(void **state)
{
	UNUSED_PARAMETER(state);
	require_capture();
	expect_output("padded rows at an offset", (struct layout){PADDED_STRIDE, OFFSET, FRAME_SIZE, false}, false);
}

/* A chunk holding only half the frame can't be shown: the capture keeps
 * the last whole frame instead of reading past the data the producer
 * gave. */
static void short_chunk_dropped(void **state)
{
	UNUSED_PARAMETER(state);
	require_capture();
	expect_output("whole frame, inverted", (struct layout){ROW_BYTES, 0, FRAME_SIZE, true}, true);
	expect_output("then a chunk of half the frame", (struct layout){ROW_BYTES, 0, FRAME_SIZE / 2, false}, true);
}

/* A stride shorter than a row can't be shown either. */
static void short_stride_dropped(void **state)
{
	UNUSED_PARAMETER(state);
	require_capture();
	expect_output("whole frame, inverted", (struct layout){ROW_BYTES, 0, FRAME_SIZE, true}, true);
	expect_output("then a stride shorter than a row", (struct layout){ROW_BYTES - 64, 0, FRAME_SIZE, false}, true);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(packed_rows),         cmocka_unit_test(padded_rows),
		cmocka_unit_test(chunk_offset),        cmocka_unit_test(padded_rows_at_offset),
		cmocka_unit_test(short_chunk_dropped), cmocka_unit_test(short_stride_dropped),
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
