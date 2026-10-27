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
#include <spa/buffer/meta.h>
#include <spa/param/video/format-utils.h>
#include <spa/param/buffers.h>

#include <obs.h>
#include <obs-nix-platform.h>
#include <util/platform.h>
#include <util/threading.h>

#include "screencast-portal.h"

/* obsproject/obs-studio#13350 (reported by ozangoktan, also #14019): with the
 * output colour space set to Rec. 2100 (PQ or HLG), PipeWire screen capture
 * draws a black square around the cursor.
 *
 * End to end: the real screencast portal source talks to a fake
 * org.freedesktop.portal.ScreenCast on a private D-Bus bus (GTestDBus) and
 * receives its stream from a private PipeWire daemon that
 * test_pipewire_cursor_hdr.py starts for it (own runtime dir, no session
 * manager, no devices). A producer stream in this test plays the compositor:
 * it sends a solid white frame with SPA_META_Cursor metadata carrying a
 * premultiplied cursor bitmap that is opaque black in the middle and fully
 * transparent around it. The capture is the only source on the output; the
 * test reads the luma plane of the rendered output frames back through a raw
 * video callback.
 *
 * The cursor's transparent border must show the white frame under it, in SDR
 * (NV12 and P010, Rec. 709) and in HDR (P010, Rec. 2100 PQ and HLG). In HDR,
 * libobs draws an sRGB source into an intermediate texture with blending
 * disabled, so a capture that layers its cursor over the frame has to turn
 * blending on itself.
 *
 * Needs dbus-daemon and an X display for obs_startup() (xvfb-run is enough);
 * the wrapper skips (77) without PipeWire, and the cases skip when
 * obs_startup() fails. */

#define SKIP_EXIT 77

#define FRAME_SIZE 128
#define CURSOR_SIZE 32
#define CURSOR_INSET 10 /* transparent rows/columns around the opaque middle */
#define CURSOR_X 48
#define CURSOR_Y 48

/* Output pixels sampled: plain frame, the cursor's transparent border and
 * its opaque middle. */
#define BG_X 16
#define BG_Y 16
#define BORDER_X (CURSOR_X + 4)
#define BORDER_Y (CURSOR_Y + 4)
#define MIDDLE_X (CURSOR_X + CURSOR_SIZE / 2)
#define MIDDLE_Y (CURSOR_Y + CURSOR_SIZE / 2)

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
/* The "compositor": a PipeWire producer stream with cursor metadata   */

#define CURSOR_META_SIZE(w, h) \
	(sizeof(struct spa_meta_cursor) + sizeof(struct spa_meta_bitmap) + (w) * (h) * 4)

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
};

static struct producer producer;

static void fill_cursor(struct spa_buffer *buf)
{
	struct spa_meta_cursor *cursor;
	struct spa_meta_bitmap *bitmap;
	uint8_t *pixels;

	cursor = spa_buffer_find_meta_data(buf, SPA_META_Cursor, CURSOR_META_SIZE(CURSOR_SIZE, CURSOR_SIZE));
	if (!cursor)
		return;

	cursor->id = 1;
	cursor->flags = 0;
	cursor->position.x = CURSOR_X;
	cursor->position.y = CURSOR_Y;
	cursor->hotspot.x = 0;
	cursor->hotspot.y = 0;
	cursor->bitmap_offset = sizeof(struct spa_meta_cursor);

	bitmap = SPA_PTROFF(cursor, cursor->bitmap_offset, struct spa_meta_bitmap);
	bitmap->format = SPA_VIDEO_FORMAT_RGBA;
	bitmap->size.width = CURSOR_SIZE;
	bitmap->size.height = CURSOR_SIZE;
	bitmap->stride = CURSOR_SIZE * 4;
	bitmap->offset = sizeof(struct spa_meta_bitmap);

	/* Premultiplied: opaque black in the middle, (0,0,0,0) around it. */
	pixels = SPA_PTROFF(bitmap, bitmap->offset, uint8_t);
	for (int y = 0; y < CURSOR_SIZE; y++) {
		for (int x = 0; x < CURSOR_SIZE; x++) {
			bool opaque = x >= CURSOR_INSET && x < CURSOR_SIZE - CURSOR_INSET && y >= CURSOR_INSET &&
				      y < CURSOR_SIZE - CURSOR_INSET;
			uint8_t *p = pixels + y * bitmap->stride + x * 4;

			p[0] = p[1] = p[2] = 0;
			p[3] = opaque ? 255 : 0;
		}
	}
}

static void on_producer_process(void *data)
{
	struct pw_buffer *b;
	struct spa_buffer *buf;
	struct spa_data *d;
	const uint32_t stride = FRAME_SIZE * 4;

	UNUSED_PARAMETER(data);

	b = pw_stream_dequeue_buffer(producer.stream);
	if (!b)
		return;

	buf = b->buffer;
	d = &buf->datas[0];
	if (d->data && d->maxsize >= stride * FRAME_SIZE) {
		memset(d->data, 0xff, stride * FRAME_SIZE); /* white BGRx */
		d->chunk->offset = 0;
		d->chunk->stride = stride;
		d->chunk->size = stride * FRAME_SIZE;
		d->chunk->flags = 0;
	}
	fill_cursor(buf);

	pw_stream_queue_buffer(producer.stream, b);
}

static void on_producer_param_changed(void *data, uint32_t id, const struct spa_pod *param)
{
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[2];

	UNUSED_PARAMETER(data);

	if (!param || id != SPA_PARAM_Format)
		return;

	params[0] = spa_pod_builder_add_object(&b, SPA_TYPE_OBJECT_ParamBuffers, SPA_PARAM_Buffers,
					       SPA_PARAM_BUFFERS_buffers, SPA_POD_CHOICE_RANGE_Int(4, 2, 8),
					       SPA_PARAM_BUFFERS_blocks, SPA_POD_Int(1), SPA_PARAM_BUFFERS_size,
					       SPA_POD_Int(FRAME_SIZE * FRAME_SIZE * 4), SPA_PARAM_BUFFERS_stride,
					       SPA_POD_Int(FRAME_SIZE * 4));
	params[1] = spa_pod_builder_add_object(&b, SPA_TYPE_OBJECT_ParamMeta, SPA_PARAM_Meta, SPA_PARAM_META_type,
					       SPA_POD_Id(SPA_META_Cursor), SPA_PARAM_META_size,
					       SPA_POD_Int(CURSOR_META_SIZE(CURSOR_SIZE, CURSOR_SIZE)));
	pw_stream_update_params(producer.stream, params, 2);
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

	if (getenv("CURSOR_HDR_TEST_VERBOSE"))
		fprintf(stderr, "registry: %u %s class '%s' node %s dir %s\n", id, type,
			dict_str(props, PW_KEY_MEDIA_CLASS), dict_str(props, PW_KEY_NODE_ID),
			dict_str(props, PW_KEY_PORT_DIRECTION));

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

static bool producer_start(void)
{
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];
	struct timespec interval = {0, 1000000000 / 30};
	struct timespec first = {0, 1};

	pw_init(NULL, NULL);
	producer.capture_node = SPA_ID_INVALID;

	producer.loop = pw_thread_loop_new("cursor-hdr-producer", NULL);
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
		pw_stream_new(producer.core, "cursor-hdr-producer",
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

struct samples {
	uint64_t frames;
	uint32_t bg, border, middle;
};

static pthread_mutex_t samples_mutex = PTHREAD_MUTEX_INITIALIZER;
static struct samples samples;

static uint32_t luma_at(const struct video_data *frame, enum video_format format, int x, int y)
{
	const uint8_t *row = frame->data[0] + (size_t)y * frame->linesize[0];

	if (format == VIDEO_FORMAT_P010)
		return ((const uint16_t *)row)[x] >> 6;
	return row[x];
}

static enum video_format output_format;

static void on_raw_video(void *param, struct video_data *frame)
{
	UNUSED_PARAMETER(param);

	pthread_mutex_lock(&samples_mutex);
	samples.bg = luma_at(frame, output_format, BG_X, BG_Y);
	samples.border = luma_at(frame, output_format, BORDER_X, BORDER_Y);
	samples.middle = luma_at(frame, output_format, MIDDLE_X, MIDDLE_Y);
	samples.frames++;
	pthread_mutex_unlock(&samples_mutex);
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

	if (level <= LOG_WARNING || getenv("CURSOR_HDR_TEST_VERBOSE")) {
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

static bool reset_video(enum video_format format, enum video_colorspace colorspace)
{
	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = FRAME_SIZE,
		.base_height = FRAME_SIZE,
		.output_width = FRAME_SIZE,
		.output_height = FRAME_SIZE,
		.output_format = format,
		.colorspace = colorspace,
		.range = VIDEO_RANGE_PARTIAL,
		.adapter = 0,
		.gpu_conversion = true,
		.scale_type = OBS_SCALE_POINT,
	};

	output_format = format;
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS)
		return false;
	/* The frontend's defaults (Settings -> Advanced -> Video). */
	obs_set_video_levels(300.0f, 1000.0f);
	return true;
}

/* Waits until the output shows the cursor (its opaque middle is darker
 * than the frame) and returns the luma samples of a later frame. */
static bool read_cursor_frame(struct samples *out)
{
	uint64_t end = os_gettime_ns() + 10000000000ULL;
	uint64_t seen = 0;

	memset(out, 0, sizeof(*out));
	pthread_mutex_lock(&samples_mutex);
	memset(&samples, 0, sizeof(samples));
	pthread_mutex_unlock(&samples_mutex);

	obs_add_raw_video_callback(NULL, on_raw_video, NULL);
	while (os_gettime_ns() < end) {
		iterate_main_loop(20);

		pthread_mutex_lock(&samples_mutex);
		*out = samples;
		pthread_mutex_unlock(&samples_mutex);

		if (out->frames == 0 || out->middle + 100 > out->bg)
			continue;
		/* Settle a few frames after the cursor first shows. */
		if (!seen)
			seen = out->frames;
		else if (out->frames >= seen + 5)
			break;
	}
	obs_remove_raw_video_callback(on_raw_video, NULL);
	return seen != 0;
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
	if (!reset_video(VIDEO_FORMAT_NV12, VIDEO_CS_709)) {
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

	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);

	if (obs_up) {
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

static void check_cursor_border(enum video_format format, enum video_colorspace colorspace, const char *name)
{
	struct samples s = {0};
	uint32_t tolerance = format == VIDEO_FORMAT_P010 ? 8 : 2;

	if (!obs_up)
		skip();

	assert_non_null(capture);
	assert_true(producer_link_ready());
	assert_true(reset_video(format, colorspace));

	if (!read_cursor_frame(&s)) {
		fprintf(stderr, "%s: the cursor never showed in the output (frames %" PRIu64 ", bg %u, middle %u)\n",
			name, s.frames, s.bg, s.middle);
		fail();
	}

	fprintf(stderr, "%s: luma frame %u, cursor border %u, cursor middle %u\n", name, s.bg, s.border, s.middle);

	/* The transparent border shows the frame under it. */
	if (s.border + tolerance < s.bg || s.border > s.bg + tolerance) {
		fprintf(stderr, "%s: the cursor's transparent border is not the frame under it\n", name);
		fail();
	}
}

static void cursor_border_sdr(void **state)
{
	UNUSED_PARAMETER(state);
	check_cursor_border(VIDEO_FORMAT_NV12, VIDEO_CS_709, "NV12 Rec. 709");
}

static void cursor_border_pq(void **state)
{
	UNUSED_PARAMETER(state);
	check_cursor_border(VIDEO_FORMAT_P010, VIDEO_CS_2100_PQ, "P010 Rec. 2100 PQ");
}

static void cursor_border_p010_709(void **state)
{
	UNUSED_PARAMETER(state);
	check_cursor_border(VIDEO_FORMAT_P010, VIDEO_CS_709, "P010 Rec. 709");
}

static void cursor_border_hlg(void **state)
{
	UNUSED_PARAMETER(state);
	check_cursor_border(VIDEO_FORMAT_P010, VIDEO_CS_2100_HLG, "P010 Rec. 2100 HLG");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(cursor_border_sdr),
		cmocka_unit_test(cursor_border_p010_709),
		cmocka_unit_test(cursor_border_pq),
		cmocka_unit_test(cursor_border_hlg),
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
