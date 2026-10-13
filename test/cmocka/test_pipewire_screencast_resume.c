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
#include <util/platform.h>
#include <util/threading.h>

#include "screencast-portal.h"

/* obsproject/obs-studio#12063: a Wayland screen capture stayed frozen after its
 * monitor was switched off or unplugged, or the computer slept. The compositor
 * ends the screencast when its output goes away: the stream's PipeWire node is
 * removed and OBS's stream only pauses, with no error, so nothing reconnected.
 *
 * The screencast portal source runs against a fake ScreenCast portal on a
 * private D-Bus bus and a producer stream (the "monitor") on a private
 * PipeWire daemon that test_pipewire_cursor_hdr.sh starts. Turning the monitor
 * off destroys the producer stream; while it is off, the portal fails to start
 * a screencast. Needs an X display (xvfb-run) for obs_startup(), dbus-daemon
 * and pipewire; skips (77, or cmocka skips) without them. */

#define SKIP_EXIT 77
#define FRAME_SIZE 64
#define RESTORE_TOKEN "screencast-resume-test-token"

/* Gray levels the producer fills its frames with, and the output luma they
 * give (BT.709, partial range): 16 + v * 219 / 255. */
#define WHITE 0xff
#define GRAY 0x80
#define DARK 0x40

const char *obs_module_text(const char *val)
{
	return val;
}

static uint32_t expected_luma(uint8_t value)
{
	return 16 + value * 219 / 255;
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

/* What the portal saw, and the node it hands out (SPA_ID_INVALID while the
 * monitor is off). */
static pthread_mutex_t portal_mutex = PTHREAD_MUTEX_INITIALIZER;
static uint32_t portal_node = SPA_ID_INVALID;
static uint32_t portal_sessions;
static uint32_t portal_selects_without_token;

static uint32_t get_portal_sessions(void)
{
	uint32_t sessions;

	pthread_mutex_lock(&portal_mutex);
	sessions = portal_sessions;
	pthread_mutex_unlock(&portal_mutex);
	return sessions;
}

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
	uint32_t response = 0;

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
	pthread_mutex_lock(&portal_mutex);
	if (strcmp(method_name, "CreateSession") == 0) {
		const char *session_token = NULL;
		g_autofree char *session_path = NULL;

		portal_sessions++;
		g_variant_lookup(options, "session_handle_token", "&s", &session_token);
		session_path =
			g_strdup_printf("/org/freedesktop/portal/desktop/session/%s/%s", sender_part, session_token);
		g_variant_builder_add(&results, "{sv}", "session_handle", g_variant_new_string(session_path));
	} else if (strcmp(method_name, "SelectSources") == 0) {
		const char *token = NULL;

		/* Without a matching token a real portal shows its picker. */
		if (!g_variant_lookup(options, "restore_token", "&s", &token) || strcmp(token, RESTORE_TOKEN) != 0)
			portal_selects_without_token++;
	} else if (strcmp(method_name, "Start") == 0 && portal_node == SPA_ID_INVALID) {
		/* The monitor is off: nothing to share. */
		response = 2;
	} else if (strcmp(method_name, "Start") == 0) {
		GVariantBuilder streams;
		GVariantBuilder props;

		g_variant_builder_init(&props, G_VARIANT_TYPE_VARDICT);
		g_variant_builder_add(&props, "{sv}", "size", g_variant_new("(ii)", FRAME_SIZE, FRAME_SIZE));
		g_variant_builder_add(&props, "{sv}", "source_type", g_variant_new_uint32(1));
		g_variant_builder_init(&streams, G_VARIANT_TYPE("a(ua{sv})"));
		g_variant_builder_add(&streams, "(u@a{sv})", portal_node, g_variant_builder_end(&props));
		g_variant_builder_add(&results, "{sv}", "streams", g_variant_builder_end(&streams));
		g_variant_builder_add(&results, "{sv}", "restore_token", g_variant_new_string(RESTORE_TOKEN));
	}
	pthread_mutex_unlock(&portal_mutex);
	g_dbus_connection_emit_signal(connection, sender, request_path, "org.freedesktop.portal.Request", "Response",
				      g_variant_new("(u@a{sv})", response, g_variant_builder_end(&results)), NULL);
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
		return g_variant_new_uint32(1 | 2); /* hidden, embedded */
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
/* The "compositor": one producer stream per monitor that is on        */

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
	uint32_t node;
	uint8_t value;
	uint32_t capture_node;
	uint32_t capture_client;
	struct {
		uint32_t id, node;
		bool out;
	} ports[32];
	size_t n_ports;
	struct pw_proxy *link;
	struct spa_hook link_listener;
	uint32_t link_in_node;
	uint32_t link_in_client;
	struct pw_proxy *error_client;
};

static struct producer producer;

static void on_producer_process(void *data)
{
	struct pw_buffer *b;
	struct spa_data *d;
	const uint32_t stride = FRAME_SIZE * 4;

	UNUSED_PARAMETER(data);

	b = pw_stream_dequeue_buffer(producer.stream);
	if (!b)
		return;

	d = &b->buffer->datas[0];
	if (d->data && d->maxsize >= stride * FRAME_SIZE) {
		memset(d->data, producer.value, stride * FRAME_SIZE); /* gray BGRx */
		d->chunk->offset = 0;
		d->chunk->stride = stride;
		d->chunk->size = stride * FRAME_SIZE;
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
					       SPA_POD_Int(FRAME_SIZE * FRAME_SIZE * 4), SPA_PARAM_BUFFERS_stride,
					       SPA_POD_Int(FRAME_SIZE * 4));
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

	if (producer.stream)
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
		if (strcmp(dict_str(props, PW_KEY_MEDIA_CLASS), "Stream/Input/Video") == 0) {
			producer.capture_node = id;
			producer.capture_client = dict_uint(props, PW_KEY_CLIENT_ID);
		}
	} else if (strcmp(type, PW_TYPE_INTERFACE_Port) == 0 && producer.n_ports < SPA_N_ELEMENTS(producer.ports)) {
		producer.ports[producer.n_ports].id = id;
		producer.ports[producer.n_ports].node = dict_uint(props, PW_KEY_NODE_ID);
		producer.ports[producer.n_ports].out = strcmp(dict_str(props, PW_KEY_PORT_DIRECTION), "out") == 0;
		producer.n_ports++;
	}
}

static void on_registry_global_remove(void *data, uint32_t id)
{
	UNUSED_PARAMETER(data);

	if (id == producer.capture_node)
		producer.capture_node = SPA_ID_INVALID;
	/* Node ids are reused: a new capture stream can get the id of the one
	 * linked last. */
	if (id == producer.link_in_node)
		producer.link_in_node = SPA_ID_INVALID;
	for (size_t i = 0; i < producer.n_ports; i++) {
		if (producer.ports[i].id == id) {
			producer.ports[i] = producer.ports[--producer.n_ports];
			break;
		}
	}
}

static const struct pw_registry_events registry_events = {
	PW_VERSION_REGISTRY_EVENTS,
	.global = on_registry_global,
	.global_remove = on_registry_global_remove,
};

static uint32_t find_port(uint32_t node, bool out)
{
	for (size_t i = 0; i < producer.n_ports; i++) {
		if (producer.ports[i].node == node && producer.ports[i].out == out)
			return producer.ports[i].id;
	}
	return SPA_ID_INVALID;
}

/* A link that fails (e.g. no format both ends accept) is reported to the
 * capture stream's client, as WirePlumber does (linking/link-target.lua): the
 * stream goes to the error state with the link's message. */
static void on_link_info(void *data, const struct pw_link_info *info)
{
	UNUSED_PARAMETER(data);

	if (!(info->change_mask & PW_LINK_CHANGE_MASK_STATE) || info->state != PW_LINK_STATE_ERROR ||
	    producer.link_in_client == SPA_ID_INVALID)
		return;

	if (producer.error_client)
		pw_proxy_destroy(producer.error_client);
	producer.error_client = pw_registry_bind(producer.registry, producer.link_in_client, PW_TYPE_INTERFACE_Client,
						 PW_VERSION_CLIENT, 0);
	if (producer.error_client)
		pw_client_error((struct pw_client *)producer.error_client, producer.link_in_node, -EPIPE,
				info->error ? info->error : "link failed");
}

static const struct pw_link_events link_events = {
	PW_VERSION_LINK_EVENTS,
	.info = on_link_info,
};

/* The portal's stream is meant for a session manager to link; this
 * PipeWire has none, so link each new capture stream to the monitor directly,
 * once. A capture stream whose monitor went away stays unlinked, as it would
 * in a desktop session: the compositor's next screencast is a new node, which
 * only a new portal session hands out. Call with the loop locked. */
static void link_capture_locked(void)
{
	uint32_t out_port = find_port(producer.node, true);
	uint32_t in_port = find_port(producer.capture_node, false);
	struct pw_properties *props;

	if (producer.capture_node == producer.link_in_node)
		return;
	if (producer.node == SPA_ID_INVALID || producer.capture_node == SPA_ID_INVALID || out_port == SPA_ID_INVALID ||
	    in_port == SPA_ID_INVALID)
		return;

	if (producer.link)
		pw_proxy_destroy(producer.link);
	props = pw_properties_new(NULL, NULL);
	pw_properties_setf(props, PW_KEY_LINK_OUTPUT_NODE, "%u", producer.node);
	pw_properties_setf(props, PW_KEY_LINK_OUTPUT_PORT, "%u", out_port);
	pw_properties_setf(props, PW_KEY_LINK_INPUT_NODE, "%u", producer.capture_node);
	pw_properties_setf(props, PW_KEY_LINK_INPUT_PORT, "%u", in_port);
	producer.link = pw_core_create_object(producer.core, "link-factory", PW_TYPE_INTERFACE_Link, PW_VERSION_LINK,
					      &props->dict, 0);
	spa_zero(producer.link_listener);
	pw_link_add_listener((struct pw_link *)producer.link, &producer.link_listener, &link_events, NULL);
	producer.link_in_node = producer.capture_node;
	producer.link_in_client = producer.capture_client;
	pw_properties_free(props);
}

static bool producer_connect(void)
{
	pw_init(NULL, NULL);
	producer.node = SPA_ID_INVALID;
	producer.capture_node = SPA_ID_INVALID;
	producer.capture_client = SPA_ID_INVALID;
	producer.link_in_node = SPA_ID_INVALID;
	producer.link_in_client = SPA_ID_INVALID;

	producer.loop = pw_thread_loop_new("screencast-resume-producer", NULL);
	producer.context = pw_context_new(pw_thread_loop_get_loop(producer.loop), NULL, 0);
	if (pw_thread_loop_start(producer.loop) < 0)
		return false;

	pw_thread_loop_lock(producer.loop);
	producer.core = pw_context_connect(producer.context, NULL, 0);
	if (!producer.core) {
		pw_thread_loop_unlock(producer.loop);
		return false;
	}
	producer.registry = pw_core_get_registry(producer.core, PW_VERSION_REGISTRY, 0);
	pw_registry_add_listener(producer.registry, &producer.registry_listener, &registry_events, NULL);
	pw_thread_loop_unlock(producer.loop);
	return true;
}

/* Frames at 30 fps, or none at all (like a monitor in DPMS sleep whose
 * compositor stops rendering, but keeps the screencast). */
static void producer_set_frames(bool on)
{
	struct timespec interval = {0, 1000000000 / 30};
	struct timespec first = {0, 1};
	struct pw_loop *loop = pw_thread_loop_get_loop(producer.loop);

	pw_thread_loop_lock(producer.loop);
	if (!producer.timer)
		producer.timer = pw_loop_add_timer(loop, on_timeout, NULL);
	if (on)
		pw_loop_update_timer(loop, producer.timer, &first, &interval, false);
	else
		pw_loop_update_timer(loop, producer.timer, NULL, NULL, false);
	pw_thread_loop_unlock(producer.loop);
}

static void set_portal_node(uint32_t node)
{
	pthread_mutex_lock(&portal_mutex);
	portal_node = node;
	pthread_mutex_unlock(&portal_mutex);
}

/* The one format the monitor offers: BGRx of the given size. */
static const struct spa_pod *build_monitor_format(struct spa_pod_builder *b, uint32_t width, uint32_t height)
{
	return spa_pod_builder_add_object(b, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat, SPA_FORMAT_mediaType,
					  SPA_POD_Id(SPA_MEDIA_TYPE_video), SPA_FORMAT_mediaSubtype,
					  SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw), SPA_FORMAT_VIDEO_format,
					  SPA_POD_Id(SPA_VIDEO_FORMAT_BGRx), SPA_FORMAT_VIDEO_size,
					  SPA_POD_Rectangle(&SPA_RECTANGLE(width, height)), SPA_FORMAT_VIDEO_framerate,
					  SPA_POD_Fraction(&SPA_FRACTION(30, 1)));
}

static bool monitor_on_sized(uint8_t value, uint32_t width, uint32_t height)
{
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];
	uint32_t node = SPA_ID_INVALID;

	pw_thread_loop_lock(producer.loop);
	producer.value = value;
	producer.stream =
		pw_stream_new(producer.core, "screencast-resume-monitor",
			      pw_properties_new(PW_KEY_MEDIA_CLASS, "Video/Source", PW_KEY_MEDIA_TYPE, "Video", NULL));
	pw_stream_add_listener(producer.stream, &producer.stream_listener, &producer_events, NULL);
	params[0] = build_monitor_format(&b, width, height);
	pw_stream_connect(producer.stream, PW_DIRECTION_OUTPUT, PW_ID_ANY,
			  PW_STREAM_FLAG_DRIVER | PW_STREAM_FLAG_MAP_BUFFERS, params, 1);
	pw_thread_loop_unlock(producer.loop);
	producer_set_frames(true);

	for (int i = 0; i < 200 && node == SPA_ID_INVALID; i++) {
		pw_thread_loop_lock(producer.loop);
		node = producer.node = pw_stream_get_node_id(producer.stream);
		pw_thread_loop_unlock(producer.loop);
		if (node == SPA_ID_INVALID)
			os_sleep_ms(10);
	}
	set_portal_node(node);
	return node != SPA_ID_INVALID;
}

static bool monitor_on(uint8_t value)
{
	return monitor_on_sized(value, FRAME_SIZE, FRAME_SIZE);
}

/* The compositor ends the screencast of an output that went away: its
 * stream, and so its node, is destroyed. */
static void monitor_off(void)
{
	set_portal_node(SPA_ID_INVALID);
	pw_thread_loop_lock(producer.loop);
	if (producer.link) {
		pw_proxy_destroy(producer.link);
		producer.link = NULL;
	}
	if (producer.stream) {
		pw_stream_destroy(producer.stream);
		producer.stream = NULL;
	}
	producer.node = SPA_ID_INVALID;
	pw_thread_loop_unlock(producer.loop);
}

/* Changes the size the monitor offers, keeping its stream and node. */
static void monitor_set_size(uint32_t width, uint32_t height)
{
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];

	pw_thread_loop_lock(producer.loop);
	params[0] = build_monitor_format(&b, width, height);
	if (producer.stream)
		pw_stream_update_params(producer.stream, params, 1);
	pw_thread_loop_unlock(producer.loop);
}

static void producer_set_value(uint8_t value)
{
	pw_thread_loop_lock(producer.loop);
	producer.value = value;
	pw_thread_loop_unlock(producer.loop);
}

static void producer_disconnect(void)
{
	if (!producer.loop)
		return;
	monitor_off();
	pw_thread_loop_lock(producer.loop);
	if (producer.timer)
		pw_loop_destroy_source(pw_thread_loop_get_loop(producer.loop), producer.timer);
	if (producer.error_client)
		pw_proxy_destroy(producer.error_client);
	if (producer.registry)
		pw_proxy_destroy((struct pw_proxy *)producer.registry);
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

static pthread_mutex_t samples_mutex = PTHREAD_MUTEX_INITIALIZER;
static uint32_t sample_luma;

static void on_raw_video(void *param, struct video_data *frame)
{
	UNUSED_PARAMETER(param);

	pthread_mutex_lock(&samples_mutex);
	sample_luma = frame->data[0][(size_t)(FRAME_SIZE / 2) * frame->linesize[0] + FRAME_SIZE / 2];
	pthread_mutex_unlock(&samples_mutex);
}

static uint32_t get_sample_luma(void)
{
	uint32_t luma;

	pthread_mutex_lock(&samples_mutex);
	luma = sample_luma;
	pthread_mutex_unlock(&samples_mutex);
	return luma;
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

	if (level <= LOG_WARNING || getenv("SCREENCAST_RESUME_TEST_VERBOSE")) {
		fprintf(stderr, "[%.3f] ", os_gettime_ns() / 1e9);
		vfprintf(stderr, format, args);
		fputc('\n', stderr);
	}
}

/* Runs the main loop (where the portal replies and reconnection timers are
 * handled, like the frontend's Qt loop) for ms, keeping the graph linked. */
static void pump(uint32_t ms)
{
	uint64_t end = os_gettime_ns() + ms * 1000000ULL;

	while (os_gettime_ns() < end) {
		while (g_main_context_iteration(NULL, FALSE))
			;
		pw_thread_loop_lock(producer.loop);
		link_capture_locked();
		pw_thread_loop_unlock(producer.loop);
		os_sleep_ms(5);
	}
}

/* Waits up to timeout_ms for the output to show the gray level value. */
static bool wait_for_value(uint8_t value, uint32_t timeout_ms)
{
	const uint32_t want = expected_luma(value);

	for (uint32_t waited = 0; waited < timeout_ms; waited += 50) {
		uint32_t luma;

		pump(50);
		luma = get_sample_luma();
		if (luma + 3 >= want && luma <= want + 3)
			return true;
	}
	return false;
}

static int setup(void **state)
{
	g_autofree char *display_name = g_strdup(getenv("DISPLAY"));
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

	if (!producer_connect() || !monitor_on(WHITE)) {
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
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: cases skip\n");
		obs_shutdown();
		return 0;
	}
	obs_up = true;

	/* A source the user set up before: it has a restore token, so the
	 * portal starts it again without asking. */
	screencast_portal_load();
	settings = obs_data_create();
	obs_data_set_string(settings, "RestoreToken", RESTORE_TOKEN);
	capture = obs_source_create("pipewire-screen-capture-source", "capture", settings, NULL);
	obs_data_release(settings);
	if (capture)
		obs_set_output_source(0, capture);
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
		pump(100);
		obs_shutdown();
		screencast_portal_unload();
	}
	producer_disconnect();
	/* The bus is not torn down: the portal code keeps its connection,
	 * which g_test_dbus_down() would wait for. GTestDBus stops the bus
	 * when the process exits. */
	return 0;
}

static void require_capture(void)
{
	const uint8_t values[] = {WHITE, GRAY, DARK};

	if (!obs_up || !capture)
		skip();
	for (uint32_t waited = 0; waited < 10000; waited += 50) {
		uint32_t luma = get_sample_luma();

		for (size_t i = 0; i < SPA_N_ELEMENTS(values); i++) {
			if (luma + 3 >= expected_luma(values[i]) && luma <= expected_luma(values[i]) + 3)
				return;
		}
		pump(50);
	}
	fprintf(stderr, "the capture never showed the monitor (luma %u)\n", get_sample_luma());
	fail();
}

static void check_no_picker(void)
{
	pthread_mutex_lock(&portal_mutex);
	uint32_t without_token = portal_selects_without_token;
	pthread_mutex_unlock(&portal_mutex);

	if (without_token) {
		fprintf(stderr, "%u source selections without the restore token (a picker for the user)\n",
			without_token);
		fail();
	}
}

/* A monitor in DPMS sleep (or an idle screen) sends no frames but keeps its
 * screencast: that is not a lost connection, and frames resume by themselves. */
static void no_frames_is_not_lost(void **state)
{
	uint32_t sessions;

	UNUSED_PARAMETER(state);
	require_capture();
	sessions = get_portal_sessions();

	producer_set_frames(false);
	producer_set_value(DARK);
	pump(3000);
	producer_set_frames(true);

	if (!wait_for_value(DARK, 5000)) {
		fprintf(stderr, "frames did not resume (luma %u, want %u)\n", get_sample_luma(), expected_luma(DARK));
		fail();
	}
	assert_int_equal(get_portal_sessions(), sessions);
	check_no_picker();
}

/* Hiding the source pauses its stream on purpose: not a lost connection. */
static void hidden_source_is_not_lost(void **state)
{
	uint32_t sessions;

	UNUSED_PARAMETER(state);
	require_capture();
	sessions = get_portal_sessions();

	obs_set_output_source(0, NULL);
	pump(1000);
	producer_set_value(WHITE);
	obs_set_output_source(0, capture);

	if (!wait_for_value(WHITE, 5000)) {
		fprintf(stderr, "frames did not resume (luma %u, want %u)\n", get_sample_luma(), expected_luma(WHITE));
		fail();
	}
	assert_int_equal(get_portal_sessions(), sessions);
	check_no_picker();
}

/* The monitor is switched off (or unplugged) and comes back: the compositor
 * removes the node, refuses to start a screencast while the monitor is off,
 * then shares the monitor again under the same restore token. */
static void frames_resume_after_monitor_returns(void **state)
{
	uint32_t sessions;

	UNUSED_PARAMETER(state);
	require_capture();
	sessions = get_portal_sessions();

	monitor_off();
	pump(2500);
	if (!monitor_on(GRAY)) {
		fprintf(stderr, "the monitor did not come back\n");
		fail();
	}

	if (!wait_for_value(GRAY, 15000)) {
		fprintf(stderr,
			"the capture stayed frozen after the monitor came back (luma %u, want %u; %u new "
			"portal sessions)\n",
			get_sample_luma(), expected_luma(GRAY), get_portal_sessions() - sessions);
		fail();
	}
	check_no_picker();
}

/* obs-rust/obs-studio#257: while a captured window animates, it can be 0
 * pixels wide for a moment, and the compositor's screencast follows it. The
 * capture stream is lost and reconnects at once, while the size is still 0,
 * so the new stream fails during negotiation ("no more input formats"). That
 * attempt must be retried once the window is back to a normal size, without
 * asking the user.
 *
 * Here the compositor replaces its stream with a 0-sized one (ending the old
 * screencast), then grows it back in place, so only a new attempt picks the
 * normal size up. */
static void frames_resume_after_failed_reconnect_stream(void **state)
{
	uint32_t sessions;
	uint32_t new_sessions;

	UNUSED_PARAMETER(state);
	require_capture();
	sessions = get_portal_sessions();

	/* The window shrinks to 0 pixels wide; the first reconnection attempt
	 * starts and fails while it is. */
	monitor_off();
	if (!monitor_on_sized(DARK, FRAME_SIZE, 0)) {
		fprintf(stderr, "the 0-sized screencast did not start\n");
		fail();
	}
	for (uint32_t waited = 0; waited < 5000 && get_portal_sessions() == sessions; waited += 50)
		pump(50);
	if (get_portal_sessions() == sessions) {
		fprintf(stderr, "the lost screencast did not start a reconnection\n");
		fail();
	}
	pump(1000);

	/* The animation ends. */
	monitor_set_size(FRAME_SIZE, FRAME_SIZE);

	if (!wait_for_value(DARK, 15000)) {
		new_sessions = get_portal_sessions() - sessions;
		fprintf(stderr,
			"the capture stayed frozen after the window came back (luma %u, want %u; %u new portal "
			"sessions after the first)\n",
			get_sample_luma(), expected_luma(DARK), new_sessions ? new_sessions - 1 : 0);
		fail();
	}
	check_no_picker();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(no_frames_is_not_lost),
		cmocka_unit_test(hidden_source_is_not_lost),
		cmocka_unit_test(frames_resume_after_monitor_returns),
		cmocka_unit_test(frames_resume_after_failed_reconnect_stream),
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
