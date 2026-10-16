#include <errno.h>
#include <inttypes.h>
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
 * and pipewire; skips (77, or cmocka skips) without them.
 *
 * obsproject/obs-studio#13836: the streams of screen captures that are not
 * showing (in a scene that is not live, or disabled) must stay paused, so the
 * compositor does not copy frames for them, and start when they are shown. */

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
	struct capture_node {
		uint32_t node, client;
		uint32_t seq;
		/* Linked once, to the monitor that was on when it appeared. */
		bool linked;
		struct pw_proxy *link;
		struct spa_hook link_listener;
	} captures[128];
	size_t n_captures;
	uint32_t captures_seen;
	struct {
		uint32_t id, node;
		bool out;
	} ports[64];
	size_t n_ports;
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
		if (strcmp(dict_str(props, PW_KEY_MEDIA_CLASS), "Stream/Input/Video") == 0 &&
		    producer.n_captures < SPA_N_ELEMENTS(producer.captures)) {
			struct capture_node *capture_node = &producer.captures[producer.n_captures++];

			memset(capture_node, 0, sizeof(*capture_node));
			capture_node->node = id;
			capture_node->client = dict_uint(props, PW_KEY_CLIENT_ID);
			capture_node->seq = ++producer.captures_seen;
		}
	} else if (strcmp(type, PW_TYPE_INTERFACE_Port) == 0 && producer.n_ports < SPA_N_ELEMENTS(producer.ports)) {
		producer.ports[producer.n_ports].id = id;
		producer.ports[producer.n_ports].node = dict_uint(props, PW_KEY_NODE_ID);
		producer.ports[producer.n_ports].out = strcmp(dict_str(props, PW_KEY_PORT_DIRECTION), "out") == 0;
		producer.n_ports++;
	}
}

static void capture_node_clear(struct capture_node *capture_node)
{
	if (capture_node->link) {
		spa_hook_remove(&capture_node->link_listener);
		pw_proxy_destroy(capture_node->link);
	}
	capture_node->link = NULL;
}

static void on_registry_global_remove(void *data, uint32_t id)
{
	UNUSED_PARAMETER(data);

	/* Node ids are reused: a new capture stream can get the id of one
	 * linked before. It gets a new entry; the old one stays, gone. */
	for (size_t i = 0; i < producer.n_captures; i++) {
		if (producer.captures[i].node == id) {
			capture_node_clear(&producer.captures[i]);
			producer.captures[i].node = SPA_ID_INVALID;
			producer.captures[i].linked = true;
		}
	}
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
	struct capture_node *capture_node = data;

	if (!(info->change_mask & PW_LINK_CHANGE_MASK_STATE) || info->state != PW_LINK_STATE_ERROR ||
	    capture_node->client == SPA_ID_INVALID)
		return;

	if (producer.error_client)
		pw_proxy_destroy(producer.error_client);
	producer.error_client = pw_registry_bind(producer.registry, capture_node->client, PW_TYPE_INTERFACE_Client,
						 PW_VERSION_CLIENT, 0);
	if (producer.error_client)
		pw_client_error((struct pw_client *)producer.error_client, capture_node->node, -EPIPE,
				info->error ? info->error : "link failed");
}

static const struct pw_link_events link_events = {
	PW_VERSION_LINK_EVENTS,
	.info = on_link_info,
};

/* The portal's streams are meant for a session manager to link; this
 * PipeWire has none, so link each new capture stream to the monitor directly,
 * once. A capture stream whose monitor went away stays unlinked, as it would
 * in a desktop session: the compositor's next screencast is a new node, which
 * only a new portal session hands out. Call with the loop locked. */
static void link_capture_locked(void)
{
	uint32_t out_port = find_port(producer.node, true);

	if (producer.node == SPA_ID_INVALID || out_port == SPA_ID_INVALID)
		return;

	for (size_t i = 0; i < producer.n_captures; i++) {
		struct capture_node *capture_node = &producer.captures[i];
		uint32_t in_port = find_port(capture_node->node, false);
		struct pw_properties *props;

		if (capture_node->linked || in_port == SPA_ID_INVALID)
			continue;

		props = pw_properties_new(NULL, NULL);
		pw_properties_setf(props, PW_KEY_LINK_OUTPUT_NODE, "%u", producer.node);
		pw_properties_setf(props, PW_KEY_LINK_OUTPUT_PORT, "%u", out_port);
		pw_properties_setf(props, PW_KEY_LINK_INPUT_NODE, "%u", capture_node->node);
		pw_properties_setf(props, PW_KEY_LINK_INPUT_PORT, "%u", in_port);
		capture_node->link = pw_core_create_object(producer.core, "link-factory", PW_TYPE_INTERFACE_Link,
							   PW_VERSION_LINK, &props->dict, 0);
		spa_zero(capture_node->link_listener);
		if (capture_node->link)
			pw_link_add_listener((struct pw_link *)capture_node->link, &capture_node->link_listener,
					     &link_events, capture_node);
		capture_node->linked = true;
		pw_properties_free(props);
	}
}

static bool producer_connect(void)
{
	pw_init(NULL, NULL);
	producer.node = SPA_ID_INVALID;

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
	for (size_t i = 0; i < producer.n_captures; i++) {
		if (producer.captures[i].link) {
			spa_hook_remove(&producer.captures[i].link_listener);
			pw_proxy_destroy(producer.captures[i].link);
			producer.captures[i].link = NULL;
		}
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
	for (size_t i = 0; i < producer.n_captures; i++)
		capture_node_clear(&producer.captures[i]);
	producer.n_captures = 0;
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
/* Frames each capture stream takes                                   */

/* Linked with -Wl,--wrap=pw_stream_dequeue_buffer: every buffer a stream
 * dequeues is a frame it got. Counted per node; called on the PipeWire
 * threads. */
struct pw_buffer *__real_pw_stream_dequeue_buffer(struct pw_stream *stream);
struct pw_buffer *__wrap_pw_stream_dequeue_buffer(struct pw_stream *stream);

static pthread_mutex_t frames_mutex = PTHREAD_MUTEX_INITIALIZER;
static struct {
	uint32_t node;
	uint64_t frames;
} frame_counts[256];
static size_t n_frame_counts;

struct pw_buffer *__wrap_pw_stream_dequeue_buffer(struct pw_stream *stream)
{
	struct pw_buffer *buffer = __real_pw_stream_dequeue_buffer(stream);
	uint32_t node;
	size_t i;

	if (!buffer)
		return NULL;

	node = pw_stream_get_node_id(stream);
	pthread_mutex_lock(&frames_mutex);
	for (i = 0; i < n_frame_counts && frame_counts[i].node != node; i++)
		;
	if (i == n_frame_counts && n_frame_counts < SPA_N_ELEMENTS(frame_counts))
		frame_counts[n_frame_counts++].node = node;
	if (i < n_frame_counts)
		frame_counts[i].frames++;
	pthread_mutex_unlock(&frames_mutex);
	return buffer;
}

static uint64_t frames_of(uint32_t node)
{
	uint64_t frames = 0;

	pthread_mutex_lock(&frames_mutex);
	for (size_t i = 0; i < n_frame_counts; i++) {
		if (frame_counts[i].node == node)
			frames = frame_counts[i].frames;
	}
	pthread_mutex_unlock(&frames_mutex);
	return frames;
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
/* Sources that are not showing: one in a scene that is not live, one
 * disabled in a live scene (obsproject/obs-studio#13836). */
static obs_scene_t *other_scene;
static obs_scene_t *live_scene;
static obs_source_t *other_capture;
static obs_source_t *disabled_capture;
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

/* A source the user set up before: it has a restore token, so the portal
 * starts it again without asking. */
static obs_source_t *create_capture(const char *name)
{
	obs_data_t *settings = obs_data_create();
	obs_source_t *source;

	obs_data_set_string(settings, "RestoreToken", RESTORE_TOKEN);
	source = obs_source_create("pipewire-screen-capture-source", name, settings, NULL);
	obs_data_release(settings);
	return source;
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

	screencast_portal_load();
	capture = create_capture("capture");
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
		obs_set_output_source(1, NULL);
		/* The scenes release their items when they are destroyed, on
		 * the destruction thread. */
		obs_scene_release(other_scene);
		obs_scene_release(live_scene);
		obs_wait_for_destroy_queue();
		obs_source_release(other_capture);
		obs_source_release(disabled_capture);
		other_scene = live_scene = NULL;
		other_capture = disabled_capture = NULL;
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

/* ------------------------------------------------------------------ */
/* Sources that are not showing (obsproject/obs-studio#13836)          */

/* With several screen captures, the ones in other scenes or disabled made
 * the compositor copy every frame for them too: their streams started active
 * and only a hide after a show paused them. */

static uint32_t captures_seen(void)
{
	uint32_t seen;

	pw_thread_loop_lock(producer.loop);
	seen = producer.captures_seen;
	pw_thread_loop_unlock(producer.loop);
	return seen;
}

/* The node of the newest capture stream that appeared after seq and is
 * linked to the monitor, or SPA_ID_INVALID. */
static uint32_t linked_capture_after(uint32_t seq)
{
	uint32_t node = SPA_ID_INVALID;
	uint32_t newest = seq;

	pw_thread_loop_lock(producer.loop);
	for (size_t i = 0; i < producer.n_captures; i++) {
		const struct capture_node *capture_node = &producer.captures[i];

		if (capture_node->seq > newest && capture_node->node != SPA_ID_INVALID && capture_node->link) {
			newest = capture_node->seq;
			node = capture_node->node;
		}
	}
	pw_thread_loop_unlock(producer.loop);
	return node;
}

static uint32_t wait_linked_capture_after(uint32_t seq, uint32_t timeout_ms)
{
	uint32_t node = SPA_ID_INVALID;

	for (uint32_t waited = 0; waited < timeout_ms && node == SPA_ID_INVALID; waited += 50) {
		pump(50);
		node = linked_capture_after(seq);
	}
	return node;
}

/* Frames the capture stream on node gets in ms (the monitor sends 30 a
 * second). */
static uint64_t frames_in(uint32_t node, uint32_t ms)
{
	uint64_t before = frames_of(node);

	pump(ms);
	return frames_of(node) - before;
}

/* Waits up to timeout_ms for the stream on node to stop getting frames:
 * none in 300 ms. */
static bool wait_no_frames(uint32_t node, uint32_t timeout_ms)
{
	for (uint32_t waited = 0; waited < timeout_ms; waited += 300) {
		if (frames_in(node, 300) == 0)
			return true;
	}
	return false;
}

/* Milliseconds until the output shows the gray level value, or UINT32_MAX. */
static uint32_t ms_until_value(uint8_t value, uint32_t timeout_ms)
{
	const uint32_t want = expected_luma(value);
	const uint64_t start = os_gettime_ns();

	while (os_gettime_ns() - start < timeout_ms * 1000000ULL) {
		uint32_t luma = get_sample_luma();

		if (luma + 3 >= want && luma <= want + 3)
			return (uint32_t)((os_gettime_ns() - start) / 1000000);
		pump(5);
	}
	return UINT32_MAX;
}

static uint32_t other_node = SPA_ID_INVALID;
static uint32_t disabled_node = SPA_ID_INVALID;

/* As a scene collection loads them: one source in a scene that is not live,
 * and one whose item is disabled in a scene that is. */
static void add_sources_not_showing(void)
{
	obs_sceneitem_t *item;
	uint32_t seq;

	if (other_capture)
		return;

	seq = captures_seen();
	other_scene = obs_scene_create_private("other scene");
	other_capture = create_capture("capture in another scene");
	obs_scene_add(other_scene, other_capture);
	other_node = wait_linked_capture_after(seq, 10000);

	seq = captures_seen();
	live_scene = obs_scene_create_private("live scene");
	disabled_capture = create_capture("disabled capture");
	item = obs_scene_add(live_scene, disabled_capture);
	obs_sceneitem_set_visible(item, false);
	obs_set_output_source(1, obs_scene_get_source(live_scene));
	disabled_node = wait_linked_capture_after(seq, 10000);

	if (other_node == SPA_ID_INVALID || disabled_node == SPA_ID_INVALID) {
		fprintf(stderr, "the streams of the sources that are not showing were not set up\n");
		fail();
	}
}

static void sources_not_showing_keep_their_streams_inactive(void **state)
{
	uint64_t main_frames, other_frames, disabled_frames;
	uint32_t main_node;

	UNUSED_PARAMETER(state);
	require_capture();
	main_node = linked_capture_after(0);

	add_sources_not_showing();
	/* Plenty of time for a stream to start, as a showing one does. */
	pump(1000);

	other_frames = frames_of(other_node);
	disabled_frames = frames_of(disabled_node);
	main_frames = frames_of(main_node);
	pump(2000);
	main_frames = frames_of(main_node) - main_frames;
	other_frames = frames_of(other_node) - other_frames;
	disabled_frames = frames_of(disabled_node) - disabled_frames;
	fprintf(stderr,
		"frames in 2 s: showing capture %" PRIu64 ", in another scene %" PRIu64 ", disabled %" PRIu64 "\n",
		main_frames, other_frames, disabled_frames);
	/* Control: the showing capture gets the monitor's frames. */
	assert_true(main_frames >= 20);
	if (other_frames || disabled_frames) {
		fprintf(stderr, "a source that is not showing has a stream that gets frames\n");
		fail();
	}
	check_no_picker();
}

/* Showing such a source starts its stream at once, every time, and hiding it
 * pauses it again. */
static void showing_a_source_starts_its_stream(void **state)
{
	const uint32_t rounds = 20;
	uint8_t value = DARK;
	uint32_t slowest = 0;
	uint64_t total = 0;

	UNUSED_PARAMETER(state);
	require_capture();
	add_sources_not_showing();

	/* Nothing showing: the output is black. */
	obs_set_output_source(0, NULL);
	pump(500);

	for (uint32_t i = 0; i < rounds; i++) {
		uint32_t ms;

		/* A new picture while the source is hidden: only frames
		 * streamed after the show have it. */
		value = i % 2 ? GRAY : DARK;
		producer_set_value(value);
		pump(100);
		obs_set_output_source(0, other_capture);
		ms = ms_until_value(value, 3000);
		if (ms == UINT32_MAX) {
			fprintf(stderr, "round %u: the shown source did not stream (luma %u, want %u)\n", i,
				get_sample_luma(), expected_luma(value));
			fail();
		}
		slowest = ms > slowest ? ms : slowest;
		total += ms;

		obs_set_output_source(0, NULL);
		if (!wait_no_frames(other_node, 3000)) {
			fprintf(stderr, "round %u: the hidden source's stream kept getting frames\n", i);
			fail();
		}
	}
	fprintf(stderr, "first new frame after show: %.1f ms on average, %u ms at most (%u rounds)\n",
		(double)total / rounds, slowest, rounds);
	/* The output and the monitor both run at 30 fps: a few frames. */
	assert_true(slowest < 1000);

	obs_set_output_source(0, capture);
	assert_true(wait_for_value(value, 5000));
	check_no_picker();
}

/* obs-rust/obs-studio#109: streams that replace lost ones while their
 * sources are not showing stay paused too, and start when shown. */
static void sources_not_showing_stay_inactive_after_reconnect(void **state)
{
	uint32_t seq;
	uint32_t running = 0;
	uint32_t paused = 0;

	UNUSED_PARAMETER(state);
	require_capture();
	add_sources_not_showing();

	seq = captures_seen();
	monitor_off();
	pump(2500);
	if (!monitor_on(WHITE)) {
		fprintf(stderr, "the monitor did not come back\n");
		fail();
	}
	if (!wait_for_value(WHITE, 15000)) {
		fprintf(stderr, "the showing capture did not come back (luma %u)\n", get_sample_luma());
		fail();
	}
	/* All three reconnect. */
	for (uint32_t waited = 0; waited < 15000; waited += 50) {
		uint32_t linked = 0;

		pw_thread_loop_lock(producer.loop);
		for (size_t i = 0; i < producer.n_captures; i++) {
			if (producer.captures[i].seq > seq && producer.captures[i].node != SPA_ID_INVALID &&
			    producer.captures[i].link)
				linked++;
		}
		pw_thread_loop_unlock(producer.loop);
		if (linked >= 3)
			break;
		pump(50);
	}
	pump(1000);

	{
		uint32_t nodes[SPA_N_ELEMENTS(producer.captures)];
		uint64_t before[SPA_N_ELEMENTS(producer.captures)];
		size_t n = 0;

		pw_thread_loop_lock(producer.loop);
		for (size_t i = 0; i < producer.n_captures; i++) {
			if (producer.captures[i].seq > seq && producer.captures[i].node != SPA_ID_INVALID &&
			    producer.captures[i].link)
				nodes[n++] = producer.captures[i].node;
		}
		pw_thread_loop_unlock(producer.loop);
		for (size_t i = 0; i < n; i++)
			before[i] = frames_of(nodes[i]);
		pump(2000);
		for (size_t i = 0; i < n; i++) {
			if (frames_of(nodes[i]) > before[i])
				running++;
			else
				paused++;
		}
	}
	fprintf(stderr, "after reconnecting: %u streaming, %u paused streams\n", running, paused);
	assert_int_equal(running, 1);
	assert_int_equal(paused, 2);

	producer_set_value(GRAY);
	pump(100);
	obs_set_output_source(0, disabled_capture);
	if (ms_until_value(GRAY, 3000) == UINT32_MAX) {
		fprintf(stderr, "the reconnected source did not stream when shown (luma %u)\n", get_sample_luma());
		fail();
	}
	obs_set_output_source(0, capture);
	assert_true(wait_for_value(GRAY, 5000));
	check_no_picker();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(no_frames_is_not_lost),
		cmocka_unit_test(hidden_source_is_not_lost),
		cmocka_unit_test(frames_resume_after_monitor_returns),
		cmocka_unit_test(frames_resume_after_failed_reconnect_stream),
		cmocka_unit_test(sources_not_showing_keep_their_streams_inactive),
		cmocka_unit_test(showing_a_source_starts_its_stream),
		cmocka_unit_test(sources_not_showing_stay_inactive_after_reconnect),
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
