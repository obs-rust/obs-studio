/* screencast-portal.c
 *
 * Copyright 2022 Georges Basile Stavracas Neto <georges.stavracas@gmail.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#include "pipewire.h"
#include "portal.h"

#include <gio/gunixfdlist.h>
#include <pthread.h>
#include <util/platform.h>

/* How long one reconnection attempt may take before it is abandoned and
 * retried, and the cap on the delay between attempts. */
#define RECONNECT_ATTEMPT_TIMEOUT_MS 3000
#define RECONNECT_MAX_DELAY_MS 10000
#define RECONNECT_MAX_ATTEMPTS 10

enum portal_capture_type {
	PORTAL_CAPTURE_TYPE_MONITOR = 1 << 0,
	PORTAL_CAPTURE_TYPE_WINDOW = 1 << 1,
	PORTAL_CAPTURE_TYPE_VIRTUAL = 1 << 2,
};

enum portal_cursor_mode {
	PORTAL_CURSOR_MODE_HIDDEN = 1 << 0,
	PORTAL_CURSOR_MODE_EMBEDDED = 1 << 1,
	PORTAL_CURSOR_MODE_METADATA = 1 << 2,
};

enum obs_portal_capture_type {
	OBS_PORTAL_CAPTURE_TYPE_MONITOR = PORTAL_CAPTURE_TYPE_MONITOR,
	OBS_PORTAL_CAPTURE_TYPE_WINDOW = PORTAL_CAPTURE_TYPE_WINDOW,
	OBS_PORTAL_CAPTURE_TYPE_UNIFIED = PORTAL_CAPTURE_TYPE_MONITOR | PORTAL_CAPTURE_TYPE_WINDOW,
};

struct screencast_portal_capture {
	enum obs_portal_capture_type capture_type;

	GCancellable *cancellable;

	char *session_handle;
	char *restore_token;

	obs_source_t *source;
	obs_weak_source_t *weak_source;
	obs_data_t *settings;

	uint32_t pipewire_node;
	bool cursor_visible;

	obs_pipewire *obs_pw;
	obs_pipewire_stream *obs_pw_stream;

	/* Held while reading or swapping the streams outside the graphics
	 * thread. Taken last: never take another lock while holding it. */
	pthread_mutex_t streams_mutex;

	/* Reconnection after the PipeWire daemon goes away. The dead stream is
	 * kept so its last frame stays on screen until the new stream delivers
	 * one. */
	struct {
		bool active;
		uint32_t attempts;
		uint64_t started_ns;
		guint attempt_timeout_id;
		guint retry_id;
		bool held_release_queued;
		obs_pipewire *held_obs_pw;
		obs_pipewire_stream *held_obs_pw_stream;
	} reconnect;
};

static void on_pipewire_disconnected(void *user_data);
static void reconnect_attempt_failed(struct screencast_portal_capture *capture, bool cancelled_by_user);
static void clear_reconnect_timer(guint *id);

/* ------------------------------------------------- */

static GDBusProxy *screencast_proxy = NULL;

static void ensure_screencast_portal_proxy(void)
{
	g_autoptr(GError) error = NULL;
	if (!screencast_proxy) {
		screencast_proxy = g_dbus_proxy_new_sync(portal_get_dbus_connection(), G_DBUS_PROXY_FLAGS_NONE, NULL,
							 "org.freedesktop.portal.Desktop",
							 "/org/freedesktop/portal/desktop",
							 "org.freedesktop.portal.ScreenCast", NULL, &error);

		if (error) {
			blog(LOG_WARNING, "[portals] Error retrieving D-Bus proxy: %s", error->message);
			return;
		}
	}
}

static GDBusProxy *get_screencast_portal_proxy(void)
{
	ensure_screencast_portal_proxy();
	return screencast_proxy;
}

static uint32_t get_available_capture_types(void)
{
	g_autoptr(GVariant) cached_source_types = NULL;
	uint32_t available_source_types;

	ensure_screencast_portal_proxy();

	if (!screencast_proxy)
		return 0;

	cached_source_types = g_dbus_proxy_get_cached_property(screencast_proxy, "AvailableSourceTypes");
	available_source_types = cached_source_types ? g_variant_get_uint32(cached_source_types) : 0;

	return available_source_types;
}

static uint32_t get_available_cursor_modes(void)
{
	g_autoptr(GVariant) cached_cursor_modes = NULL;
	uint32_t available_cursor_modes;

	ensure_screencast_portal_proxy();

	if (!screencast_proxy)
		return 0;

	cached_cursor_modes = g_dbus_proxy_get_cached_property(screencast_proxy, "AvailableCursorModes");
	available_cursor_modes = cached_cursor_modes ? g_variant_get_uint32(cached_cursor_modes) : 0;

	return available_cursor_modes;
}

static uint32_t get_screencast_version(void)
{
	g_autoptr(GVariant) cached_version = NULL;
	uint32_t version;

	ensure_screencast_portal_proxy();

	if (!screencast_proxy)
		return 0;

	cached_version = g_dbus_proxy_get_cached_property(screencast_proxy, "version");
	version = cached_version ? g_variant_get_uint32(cached_version) : 0;

	return version;
}

/* ------------------------------------------------- */

static const char *capture_type_to_string(enum obs_portal_capture_type capture_type)
{
	switch (capture_type) {
	case OBS_PORTAL_CAPTURE_TYPE_MONITOR:
		return "monitor";
	case OBS_PORTAL_CAPTURE_TYPE_WINDOW:
		return "window";
	case OBS_PORTAL_CAPTURE_TYPE_UNIFIED:
		return "monitor and window";
	default:
		return "unknown";
	}
}

/* ------------------------------------------------- */

static void on_pipewire_remote_opened_cb(GObject *source, GAsyncResult *res, void *user_data)
{
	struct obs_pipewire_connect_stream_info connect_info;
	struct screencast_portal_capture *capture;
	g_autoptr(GUnixFDList) fd_list = NULL;
	g_autoptr(GVariant) result = NULL;
	g_autoptr(GError) error = NULL;
	int pipewire_fd;
	int fd_index;

	capture = user_data;
	result = g_dbus_proxy_call_with_unix_fd_list_finish(G_DBUS_PROXY(source), &fd_list, res, &error);
	if (error) {
		if (!g_error_matches(error, G_IO_ERROR, G_IO_ERROR_CANCELLED)) {
			blog(LOG_ERROR, "[pipewire] Error retrieving pipewire fd: %s", error->message);
			reconnect_attempt_failed(capture, false);
		}
		return;
	}

	g_variant_get(result, "(h)", &fd_index, &error);

	pipewire_fd = g_unix_fd_list_get(fd_list, fd_index, &error);
	if (error) {
		if (!g_error_matches(error, G_IO_ERROR, G_IO_ERROR_CANCELLED))
			blog(LOG_ERROR, "[pipewire] Error retrieving pipewire fd: %s", error->message);
		return;
	}

	capture->obs_pw = obs_pipewire_connect_fd(pipewire_fd, NULL, NULL);

	if (!capture->obs_pw)
		return;

	obs_pipewire_set_disconnected_callback(capture->obs_pw, on_pipewire_disconnected, capture->weak_source);
	if (obs_pipewire_is_disconnected(capture->obs_pw)) {
		on_pipewire_disconnected(capture->weak_source);
		return;
	}

	connect_info = (struct obs_pipewire_connect_stream_info){
		.stream_name = "OBS Studio",
		.stream_properties = pw_properties_new(PW_KEY_MEDIA_TYPE, "Video", PW_KEY_MEDIA_CATEGORY, "Capture",
						       PW_KEY_MEDIA_ROLE, "Screen", NULL),
		.screencast =
			{
				.cursor_visible = capture->cursor_visible,
			},
	};

	capture->obs_pw_stream =
		obs_pipewire_connect_stream(capture->obs_pw, capture->source, capture->pipewire_node, &connect_info);

	/* show/hide calls made while reconnecting had no stream to act on. */
	if (capture->reconnect.active && capture->obs_pw_stream && !obs_source_showing(capture->source))
		obs_pipewire_stream_hide(capture->obs_pw_stream);
}

static void open_pipewire_remote(struct screencast_portal_capture *capture)
{
	GVariantBuilder builder;

	g_variant_builder_init(&builder, G_VARIANT_TYPE_VARDICT);

	g_dbus_proxy_call_with_unix_fd_list(get_screencast_portal_proxy(), "OpenPipeWireRemote",
					    g_variant_new("(oa{sv})", capture->session_handle, &builder),
					    G_DBUS_CALL_FLAGS_NONE, -1, NULL, capture->cancellable,
					    on_pipewire_remote_opened_cb, capture);
}

/* ------------------------------------------------- */

static void on_start_response_received_cb(GVariant *parameters, void *user_data)
{
	struct screencast_portal_capture *capture = user_data;
	g_autoptr(GVariant) stream_properties = NULL;
	g_autoptr(GVariant) streams = NULL;
	g_autoptr(GVariant) result = NULL;
	GVariantIter iter;
	uint32_t response;
	size_t n_streams;

	g_variant_get(parameters, "(u@a{sv})", &response, &result);

	if (response != 0) {
		blog(LOG_WARNING, "[pipewire] Failed to start screencast, denied or cancelled by user");
		reconnect_attempt_failed(capture, response == 1);
		return;
	}

	streams = g_variant_lookup_value(result, "streams", G_VARIANT_TYPE_ARRAY);

	g_variant_iter_init(&iter, streams);

	n_streams = g_variant_iter_n_children(&iter);
	if (n_streams != 1) {
		blog(LOG_WARNING, "[pipewire] Received more than one stream when only one was expected. "
				  "This is probably a bug in the desktop portal implementation you are "
				  "using.");

		// The KDE Desktop portal implementation sometimes sends an invalid
		// response where more than one stream is attached, and only the
		// last one is the one we're looking for. This is the only known
		// buggy implementation, so let's at least try to make it work here.
		for (size_t i = 0; i < n_streams - 1; i++) {
			g_autoptr(GVariant) throwaway_properties = NULL;
			uint32_t throwaway_pipewire_node;

			g_variant_iter_loop(&iter, "(u@a{sv})", &throwaway_pipewire_node, &throwaway_properties);
		}
	}

	g_variant_iter_loop(&iter, "(u@a{sv})", &capture->pipewire_node, &stream_properties);

	if (get_screencast_version() >= 4) {
		g_autoptr(GVariant) restore_token = NULL;

		g_clear_pointer(&capture->restore_token, bfree);

		restore_token = g_variant_lookup_value(result, "restore_token", G_VARIANT_TYPE_STRING);
		if (restore_token)
			capture->restore_token = bstrdup(g_variant_get_string(restore_token, NULL));

		obs_source_save(capture->source);
	}

	blog(LOG_INFO, "[pipewire] source selected, setting up screencast");

	open_pipewire_remote(capture);
}

static void on_started_cb(GObject *source, GAsyncResult *res, void *user_data)
{
	g_autoptr(GVariant) result = NULL;
	g_autoptr(GError) error = NULL;

	result = g_dbus_proxy_call_finish(G_DBUS_PROXY(source), res, &error);
	if (error) {
		/* Cancelled means the capture may be gone: don't touch it. */
		if (!g_error_matches(error, G_IO_ERROR, G_IO_ERROR_CANCELLED)) {
			blog(LOG_ERROR, "[pipewire] Error selecting screencast source: %s", error->message);
			reconnect_attempt_failed(user_data, false);
		}
		return;
	}
}

static void start(struct screencast_portal_capture *capture)
{
	GVariantBuilder builder;
	char *request_token;
	char *request_path;

	portal_create_request_path(&request_path, &request_token);

	blog(LOG_INFO, "[pipewire] Asking for %s", capture_type_to_string(capture->capture_type));

	portal_signal_subscribe(request_path, capture->cancellable, on_start_response_received_cb, capture);

	g_variant_builder_init(&builder, G_VARIANT_TYPE_VARDICT);
	g_variant_builder_add(&builder, "{sv}", "handle_token", g_variant_new_string(request_token));

	g_dbus_proxy_call(get_screencast_portal_proxy(), "Start",
			  g_variant_new("(osa{sv})", capture->session_handle, "", &builder), G_DBUS_CALL_FLAGS_NONE, -1,
			  capture->cancellable, on_started_cb, capture);

	bfree(request_token);
	bfree(request_path);
}

/* ------------------------------------------------- */

static void on_select_source_response_received_cb(GVariant *parameters, void *user_data)
{
	struct screencast_portal_capture *capture = user_data;
	g_autoptr(GVariant) ret = NULL;
	uint32_t response;

	blog(LOG_DEBUG, "[pipewire] Response to select source received");

	g_variant_get(parameters, "(u@a{sv})", &response, &ret);

	if (response != 0) {
		blog(LOG_WARNING, "[pipewire] Failed to select source, denied or cancelled by user");
		reconnect_attempt_failed(capture, response == 1);
		return;
	}

	start(capture);
}

static void on_source_selected_cb(GObject *source, GAsyncResult *res, void *user_data)
{
	g_autoptr(GVariant) result = NULL;
	g_autoptr(GError) error = NULL;

	result = g_dbus_proxy_call_finish(G_DBUS_PROXY(source), res, &error);
	if (error) {
		/* Cancelled means the capture may be gone: don't touch it. */
		if (!g_error_matches(error, G_IO_ERROR, G_IO_ERROR_CANCELLED)) {
			blog(LOG_ERROR, "[pipewire] Error selecting screencast source: %s", error->message);
			reconnect_attempt_failed(user_data, false);
		}
		return;
	}
}

static void select_source(struct screencast_portal_capture *capture)
{
	GVariantBuilder builder;
	uint32_t available_cursor_modes;
	char *request_token;
	char *request_path;

	portal_create_request_path(&request_path, &request_token);

	/* From here on the portal may show a picker (e.g. the restore token no
	 * longer matches); never time that out from under the user. */
	clear_reconnect_timer(&capture->reconnect.attempt_timeout_id);

	portal_signal_subscribe(request_path, capture->cancellable, on_select_source_response_received_cb, capture);

	g_variant_builder_init(&builder, G_VARIANT_TYPE_VARDICT);
	g_variant_builder_add(&builder, "{sv}", "types", g_variant_new_uint32(capture->capture_type));
	g_variant_builder_add(&builder, "{sv}", "multiple", g_variant_new_boolean(FALSE));
	g_variant_builder_add(&builder, "{sv}", "handle_token", g_variant_new_string(request_token));

	available_cursor_modes = get_available_cursor_modes();

	if (available_cursor_modes & PORTAL_CURSOR_MODE_METADATA)
		g_variant_builder_add(&builder, "{sv}", "cursor_mode",
				      g_variant_new_uint32(PORTAL_CURSOR_MODE_METADATA));
	else if ((available_cursor_modes & PORTAL_CURSOR_MODE_EMBEDDED) && capture->cursor_visible)
		g_variant_builder_add(&builder, "{sv}", "cursor_mode",
				      g_variant_new_uint32(PORTAL_CURSOR_MODE_EMBEDDED));
	else
		g_variant_builder_add(&builder, "{sv}", "cursor_mode", g_variant_new_uint32(PORTAL_CURSOR_MODE_HIDDEN));

	if (get_screencast_version() >= 4) {
		g_variant_builder_add(&builder, "{sv}", "persist_mode", g_variant_new_uint32(2));
		if (capture->restore_token && *capture->restore_token) {
			g_variant_builder_add(&builder, "{sv}", "restore_token",
					      g_variant_new_string(capture->restore_token));
		}
	}

	g_dbus_proxy_call(get_screencast_portal_proxy(), "SelectSources",
			  g_variant_new("(oa{sv})", capture->session_handle, &builder), G_DBUS_CALL_FLAGS_NONE, -1,
			  capture->cancellable, on_source_selected_cb, capture);

	bfree(request_token);
	bfree(request_path);
}

/* ------------------------------------------------- */

static void on_create_session_response_received_cb(GVariant *parameters, void *user_data)
{
	struct screencast_portal_capture *capture = user_data;
	g_autoptr(GVariant) session_handle_variant = NULL;
	g_autoptr(GVariant) result = NULL;
	uint32_t response;

	g_variant_get(parameters, "(u@a{sv})", &response, &result);

	if (response != 0) {
		blog(LOG_WARNING, "[pipewire] Failed to create session, denied or cancelled by user");
		reconnect_attempt_failed(capture, response == 1);
		return;
	}

	blog(LOG_INFO, "[pipewire] Screencast session created");

	session_handle_variant = g_variant_lookup_value(result, "session_handle", NULL);
	capture->session_handle = g_variant_dup_string(session_handle_variant, NULL);

	select_source(capture);
}

static void on_session_created_cb(GObject *source, GAsyncResult *res, void *user_data)
{
	g_autoptr(GVariant) result = NULL;
	g_autoptr(GError) error = NULL;

	result = g_dbus_proxy_call_finish(G_DBUS_PROXY(source), res, &error);
	if (error) {
		/* Cancelled means the capture may be gone: don't touch it. */
		if (!g_error_matches(error, G_IO_ERROR, G_IO_ERROR_CANCELLED)) {
			blog(LOG_ERROR, "[pipewire] Error creating screencast session: %s", error->message);
			reconnect_attempt_failed(user_data, false);
		}
		return;
	}
}

static void create_session(struct screencast_portal_capture *capture)
{
	GVariantBuilder builder;
	char *session_token;
	char *request_token;
	char *request_path;

	portal_create_request_path(&request_path, &request_token);
	portal_create_session_path(NULL, &session_token);

	portal_signal_subscribe(request_path, capture->cancellable, on_create_session_response_received_cb, capture);

	g_variant_builder_init(&builder, G_VARIANT_TYPE_VARDICT);
	g_variant_builder_add(&builder, "{sv}", "handle_token", g_variant_new_string(request_token));
	g_variant_builder_add(&builder, "{sv}", "session_handle_token", g_variant_new_string(session_token));

	g_dbus_proxy_call(get_screencast_portal_proxy(), "CreateSession", g_variant_new("(a{sv})", &builder),
			  G_DBUS_CALL_FLAGS_NONE, -1, capture->cancellable, on_session_created_cb, capture);

	bfree(session_token);
	bfree(request_token);
	bfree(request_path);
}

/* ------------------------------------------------- */

static gboolean init_screencast_capture(struct screencast_portal_capture *capture)
{
	GDBusConnection *connection;
	GDBusProxy *proxy;

	capture->cancellable = g_cancellable_new();
	connection = portal_get_dbus_connection();
	if (!connection)
		return FALSE;
	proxy = get_screencast_portal_proxy();
	if (!proxy)
		return FALSE;

	blog(LOG_INFO, "PipeWire initialized");

	create_session(capture);

	return TRUE;
}

/* ------------------------------------------------- */

/* Reconnection. The PipeWire thread reports a lost connection; everything
 * else runs on the main loop, like the portal calls. Callbacks hold a weak
 * reference to the source, so they become no-ops once it is destroyed. */

static struct screencast_portal_capture *capture_from_weak(obs_weak_source_t *weak_source, obs_source_t **out_source)
{
	obs_source_t *source = obs_weak_source_get_source(weak_source);

	*out_source = source;
	return source ? obs_obj_get_data(source) : NULL;
}

static void release_weak_source(void *weak_source)
{
	obs_weak_source_release(weak_source);
}

static void add_weak_idle(obs_weak_source_t *weak_source, GSourceFunc callback)
{
	obs_weak_source_addref(weak_source);
	g_idle_add_full(G_PRIORITY_HIGH_IDLE, callback, weak_source, release_weak_source);
}

static guint schedule_reconnect_timer(struct screencast_portal_capture *capture, guint delay_ms, GSourceFunc callback)
{
	obs_weak_source_addref(capture->weak_source);
	return g_timeout_add_full(G_PRIORITY_DEFAULT, delay_ms, callback, capture->weak_source, release_weak_source);
}

/* The timer may have fired already (its callback cannot clear the id once the
 * source is being destroyed), so look it up instead of g_source_remove(). */
static void clear_reconnect_timer(guint *id)
{
	GSource *timer = *id ? g_main_context_find_source_by_id(NULL, *id) : NULL;

	if (timer)
		g_source_destroy(timer);
	*id = 0;
}

static void close_session(struct screencast_portal_capture *capture)
{
	if (!capture->session_handle)
		return;

	g_dbus_connection_call(portal_get_dbus_connection(), "org.freedesktop.portal.Desktop", capture->session_handle,
			       "org.freedesktop.portal.Session", "Close", NULL, NULL, G_DBUS_CALL_FLAGS_NONE, -1, NULL,
			       NULL, NULL);

	g_clear_pointer(&capture->session_handle, g_free);
}

/* Streams are rendered under the graphics lock and their size is queried
 * under streams_mutex, so swap them out under both and destroy them after. */
static void lock_streams(struct screencast_portal_capture *capture)
{
	obs_enter_graphics();
	pthread_mutex_lock(&capture->streams_mutex);
}

static void unlock_streams(struct screencast_portal_capture *capture)
{
	pthread_mutex_unlock(&capture->streams_mutex);
	obs_leave_graphics();
}

static void destroy_live_stream(struct screencast_portal_capture *capture)
{
	obs_pipewire_stream *obs_pw_stream;
	obs_pipewire *obs_pw;

	lock_streams(capture);
	obs_pw_stream = g_steal_pointer(&capture->obs_pw_stream);
	obs_pw = g_steal_pointer(&capture->obs_pw);
	unlock_streams(capture);

	obs_pipewire_stream_destroy(obs_pw_stream);
	obs_pipewire_destroy(obs_pw);
}

static void release_held_stream(struct screencast_portal_capture *capture)
{
	obs_pipewire_stream *held_obs_pw_stream;
	obs_pipewire *held_obs_pw;

	lock_streams(capture);
	held_obs_pw_stream = g_steal_pointer(&capture->reconnect.held_obs_pw_stream);
	held_obs_pw = g_steal_pointer(&capture->reconnect.held_obs_pw);
	capture->reconnect.held_release_queued = false;
	unlock_streams(capture);

	obs_pipewire_stream_destroy(held_obs_pw_stream);
	obs_pipewire_destroy(held_obs_pw);
}

static void stop_reconnect(struct screencast_portal_capture *capture)
{
	clear_reconnect_timer(&capture->reconnect.attempt_timeout_id);
	clear_reconnect_timer(&capture->reconnect.retry_id);
	capture->reconnect.active = false;
	capture->reconnect.attempts = 0;
	release_held_stream(capture);
}

static gboolean on_reconnect_attempt_timeout(void *user_data);

static void start_reconnect_attempt(struct screencast_portal_capture *capture)
{
	/* Abandon whatever the previous attempt left behind. Cancelling also
	 * drops its pending portal responses, so a late reply cannot act on a
	 * session that no longer exists. */
	g_cancellable_cancel(capture->cancellable);
	g_clear_object(&capture->cancellable);
	capture->cancellable = g_cancellable_new();

	close_session(capture);
	destroy_live_stream(capture);

	blog(LOG_INFO, "[pipewire] Reconnecting screencast (attempt %u)", capture->reconnect.attempts + 1);

	/* Covers creating the session; select_source() disarms it because the
	 * portal may wait for the user from there on. */
	clear_reconnect_timer(&capture->reconnect.attempt_timeout_id);
	capture->reconnect.attempt_timeout_id =
		schedule_reconnect_timer(capture, RECONNECT_ATTEMPT_TIMEOUT_MS, on_reconnect_attempt_timeout);

	create_session(capture);
}

static gboolean on_reconnect_retry(void *user_data)
{
	struct screencast_portal_capture *capture;
	obs_source_t *source;

	capture = capture_from_weak(user_data, &source);
	if (capture) {
		capture->reconnect.retry_id = 0;
		start_reconnect_attempt(capture);
	}
	obs_source_release(source);

	return G_SOURCE_REMOVE;
}

static void schedule_reconnect_retry(struct screencast_portal_capture *capture)
{
	guint delay_ms;

	clear_reconnect_timer(&capture->reconnect.attempt_timeout_id);
	if (capture->reconnect.retry_id)
		return;

	capture->reconnect.attempts++;
	if (capture->reconnect.attempts >= RECONNECT_MAX_ATTEMPTS) {
		blog(LOG_WARNING,
		     "[pipewire] Giving up on reconnecting the screencast after %u attempts; "
		     "select the source again to resume capture",
		     capture->reconnect.attempts);
		stop_reconnect(capture);
		return;
	}

	delay_ms = MIN(250u << MIN(capture->reconnect.attempts, 6u), RECONNECT_MAX_DELAY_MS);
	blog(LOG_WARNING, "[pipewire] Screencast reconnection attempt %u failed, retrying in %u ms",
	     capture->reconnect.attempts, delay_ms);

	capture->reconnect.retry_id = schedule_reconnect_timer(capture, delay_ms, on_reconnect_retry);
}

static void reconnect_attempt_failed(struct screencast_portal_capture *capture, bool cancelled_by_user)
{
	if (!capture->reconnect.active)
		return;

	if (cancelled_by_user) {
		blog(LOG_INFO, "[pipewire] Screencast reconnection cancelled");
		stop_reconnect(capture);
		return;
	}

	schedule_reconnect_retry(capture);
}

static gboolean on_reconnect_attempt_timeout(void *user_data)
{
	struct screencast_portal_capture *capture;
	obs_source_t *source;

	capture = capture_from_weak(user_data, &source);
	if (capture) {
		capture->reconnect.attempt_timeout_id = 0;
		schedule_reconnect_retry(capture);
	}
	obs_source_release(source);

	return G_SOURCE_REMOVE;
}

static gboolean on_pipewire_disconnected_idle(void *user_data)
{
	struct screencast_portal_capture *capture;
	obs_source_t *source;

	capture = capture_from_weak(user_data, &source);
	if (!capture || !obs_pipewire_is_disconnected(capture->obs_pw))
		goto out;

	/* Lost again mid-reconnect: keep the frame we are already holding. */
	if (capture->reconnect.active) {
		schedule_reconnect_retry(capture);
		goto out;
	}

	if (!capture->restore_token || !*capture->restore_token) {
		blog(LOG_WARNING, "[pipewire] Lost the PipeWire connection and have no restore token; "
				  "select the source again to resume capture");
		goto out;
	}

	blog(LOG_INFO, "[pipewire] Lost the PipeWire connection, keeping the last frame and reconnecting");

	/* Keep the dead stream around so the last frame keeps rendering. */
	release_held_stream(capture);
	lock_streams(capture);
	capture->reconnect.held_obs_pw_stream = g_steal_pointer(&capture->obs_pw_stream);
	capture->reconnect.held_obs_pw = g_steal_pointer(&capture->obs_pw);
	unlock_streams(capture);

	capture->reconnect.active = true;
	capture->reconnect.attempts = 0;
	capture->reconnect.started_ns = os_gettime_ns();
	start_reconnect_attempt(capture);

out:
	obs_source_release(source);
	return G_SOURCE_REMOVE;
}

static void on_pipewire_disconnected(void *user_data)
{
	obs_weak_source_t *weak_source = user_data;

	/* PipeWire thread: hop over to the main loop. */
	add_weak_idle(weak_source, on_pipewire_disconnected_idle);
}

static gboolean on_reconnected_idle(void *user_data)
{
	struct screencast_portal_capture *capture;
	obs_source_t *source;

	capture = capture_from_weak(user_data, &source);
	/* The new stream may have been lost again since this was queued. */
	if (capture && capture->reconnect.active && obs_pipewire_stream_has_frame(capture->obs_pw_stream)) {
		blog(LOG_INFO, "[pipewire] Screencast reconnected, first frame %.0f ms after the connection was lost",
		     (os_gettime_ns() - capture->reconnect.started_ns) / 1000000.0);
		stop_reconnect(capture);
	} else if (capture) {
		obs_enter_graphics();
		capture->reconnect.held_release_queued = false;
		obs_leave_graphics();
	}
	obs_source_release(source);

	return G_SOURCE_REMOVE;
}

/* Pick the stream to draw: the live one once it has a frame, otherwise the
 * one held from before the connection was lost. Call with the graphics lock
 * or streams_mutex held. */
static obs_pipewire_stream *get_render_stream(struct screencast_portal_capture *capture)
{
	if (capture->reconnect.held_obs_pw_stream && !obs_pipewire_stream_has_frame(capture->obs_pw_stream))
		return capture->reconnect.held_obs_pw_stream;

	return capture->obs_pw_stream;
}

static bool reload_session_cb(obs_properties_t *properties, obs_property_t *property, void *data)
{
	UNUSED_PARAMETER(properties);
	UNUSED_PARAMETER(property);

	struct screencast_portal_capture *capture = data;

	stop_reconnect(capture);
	g_cancellable_cancel(capture->cancellable);
	g_clear_object(&capture->cancellable);

	g_clear_pointer(&capture->restore_token, bfree);
	destroy_live_stream(capture);

	if (capture->session_handle)
		blog(LOG_DEBUG, "[pipewire] Cleaning previous session %s", capture->session_handle);
	close_session(capture);

	init_screencast_capture(capture);

	return false;
}

/* obs_source_info methods */

static const char *screencast_portal_desktop_capture_get_name(void *data)
{
	UNUSED_PARAMETER(data);
	return obs_module_text("PipeWireDesktopCapture");
}

static const char *screencast_portal_window_capture_get_name(void *data)
{
	UNUSED_PARAMETER(data);
	return obs_module_text("PipeWireWindowCapture");
}

static void *screencast_portal_desktop_capture_create(obs_data_t *settings, obs_source_t *source)
{
	struct screencast_portal_capture *capture;

	capture = bzalloc(sizeof(struct screencast_portal_capture));
	capture->capture_type = OBS_PORTAL_CAPTURE_TYPE_MONITOR;
	capture->cursor_visible = obs_data_get_bool(settings, "ShowCursor");
	capture->restore_token = bstrdup(obs_data_get_string(settings, "RestoreToken"));
	capture->source = source;
	capture->weak_source = obs_source_get_weak_source(source);
	pthread_mutex_init(&capture->streams_mutex, NULL);

	init_screencast_capture(capture);

	return capture;
}
static void *screencast_portal_window_capture_create(obs_data_t *settings, obs_source_t *source)
{
	struct screencast_portal_capture *capture;

	capture = bzalloc(sizeof(struct screencast_portal_capture));
	capture->capture_type = OBS_PORTAL_CAPTURE_TYPE_WINDOW;
	capture->cursor_visible = obs_data_get_bool(settings, "ShowCursor");
	capture->restore_token = bstrdup(obs_data_get_string(settings, "RestoreToken"));
	capture->source = source;
	capture->weak_source = obs_source_get_weak_source(source);
	pthread_mutex_init(&capture->streams_mutex, NULL);

	init_screencast_capture(capture);

	return capture;
}

static void *screencast_portal_capture_create(obs_data_t *settings, obs_source_t *source)
{
	struct screencast_portal_capture *capture;

	capture = bzalloc(sizeof(struct screencast_portal_capture));
	capture->capture_type = OBS_PORTAL_CAPTURE_TYPE_UNIFIED;
	capture->cursor_visible = obs_data_get_bool(settings, "ShowCursor");
	capture->restore_token = bstrdup(obs_data_get_string(settings, "RestoreToken"));
	capture->source = source;
	capture->weak_source = obs_source_get_weak_source(source);
	pthread_mutex_init(&capture->streams_mutex, NULL);

	init_screencast_capture(capture);

	return capture;
}

static void screencast_portal_capture_destroy(void *data)
{
	struct screencast_portal_capture *capture = data;

	if (!capture)
		return;

	stop_reconnect(capture);
	close_session(capture);

	g_clear_pointer(&capture->restore_token, bfree);

	g_clear_pointer(&capture->obs_pw_stream, obs_pipewire_stream_destroy);
	obs_pipewire_destroy(capture->obs_pw);
	g_cancellable_cancel(capture->cancellable);
	g_clear_object(&capture->cancellable);
	obs_weak_source_release(capture->weak_source);
	pthread_mutex_destroy(&capture->streams_mutex);
	bfree(capture);
}

static void screencast_portal_capture_save(void *data, obs_data_t *settings)
{
	struct screencast_portal_capture *capture = data;

	obs_data_set_string(settings, "RestoreToken", capture->restore_token);
}

static void screencast_portal_capture_get_defaults(obs_data_t *settings)
{
	obs_data_set_default_bool(settings, "ShowCursor", true);
	obs_data_set_default_string(settings, "RestoreToken", NULL);
}

static obs_properties_t *screencast_portal_capture_get_properties(void *data)
{
	struct screencast_portal_capture *capture = data;
	const char *reload_string_id;
	obs_properties_t *properties;

	switch (capture->capture_type) {
	case OBS_PORTAL_CAPTURE_TYPE_MONITOR:
		reload_string_id = "PipeWireSelectMonitor";
		break;
	case OBS_PORTAL_CAPTURE_TYPE_WINDOW:
		reload_string_id = "PipeWireSelectWindow";
		break;
	case OBS_PORTAL_CAPTURE_TYPE_UNIFIED:
		reload_string_id = "PipeWireSelectScreenCast";
		break;
	default:
		return NULL;
	}

	properties = obs_properties_create();
	obs_properties_add_button2(properties, "Reload", obs_module_text(reload_string_id), reload_session_cb, capture);
	obs_properties_add_bool(properties, "ShowCursor", obs_module_text("ShowCursor"));

	return properties;
}

static void screencast_portal_capture_update(void *data, obs_data_t *settings)
{
	struct screencast_portal_capture *capture = data;

	capture->cursor_visible = obs_data_get_bool(settings, "ShowCursor");

	if (capture->obs_pw_stream)
		obs_pipewire_stream_set_cursor_visible(capture->obs_pw_stream, capture->cursor_visible);
}

static void screencast_portal_capture_show(void *data)
{
	struct screencast_portal_capture *capture = data;

	if (capture->obs_pw_stream)
		obs_pipewire_stream_show(capture->obs_pw_stream);
}

static void screencast_portal_capture_hide(void *data)
{
	struct screencast_portal_capture *capture = data;

	if (capture->obs_pw_stream)
		obs_pipewire_stream_hide(capture->obs_pw_stream);
}

static uint32_t screencast_portal_capture_get_width(void *data)
{
	struct screencast_portal_capture *capture = data;

	obs_pipewire_stream *obs_pw_stream;
	uint32_t value = 0;

	/* Not the graphics lock: callers may hold scene locks that the graphics
	 * thread takes after it. */
	pthread_mutex_lock(&capture->streams_mutex);
	obs_pw_stream = get_render_stream(capture);
	if (obs_pw_stream)
		value = obs_pipewire_stream_get_width(obs_pw_stream);
	pthread_mutex_unlock(&capture->streams_mutex);

	return value;
}

static uint32_t screencast_portal_capture_get_height(void *data)
{
	struct screencast_portal_capture *capture = data;

	obs_pipewire_stream *obs_pw_stream;
	uint32_t value = 0;

	/* Not the graphics lock: callers may hold scene locks that the graphics
	 * thread takes after it. */
	pthread_mutex_lock(&capture->streams_mutex);
	obs_pw_stream = get_render_stream(capture);
	if (obs_pw_stream)
		value = obs_pipewire_stream_get_height(obs_pw_stream);
	pthread_mutex_unlock(&capture->streams_mutex);

	return value;
}

static void screencast_portal_capture_video_render(void *data, gs_effect_t *effect)
{
	struct screencast_portal_capture *capture = data;

	obs_pipewire_stream *obs_pw_stream = get_render_stream(capture);

	/* The new stream has its first frame: drop the held one. */
	if (capture->reconnect.held_obs_pw_stream && obs_pw_stream == capture->obs_pw_stream &&
	    !capture->reconnect.held_release_queued) {
		capture->reconnect.held_release_queued = true;
		add_weak_idle(capture->weak_source, on_reconnected_idle);
	}

	if (obs_pw_stream)
		obs_pipewire_stream_video_render(obs_pw_stream, effect);
}

void screencast_portal_load(void)
{
	uint32_t available_capture_types = get_available_capture_types();
	bool desktop_capture_available = (available_capture_types & PORTAL_CAPTURE_TYPE_MONITOR) != 0;
	bool window_capture_available = (available_capture_types & PORTAL_CAPTURE_TYPE_WINDOW) != 0;

	if (available_capture_types == 0) {
		blog(LOG_INFO, "[pipewire] No capture sources available");
		return;
	}

	blog(LOG_INFO, "[pipewire] Available capture sources:");
	if (desktop_capture_available)
		blog(LOG_INFO, "[pipewire]     - Monitor source");
	if (window_capture_available)
		blog(LOG_INFO, "[pipewire]     - Window source");

	// Desktop capture
	const struct obs_source_info screencast_portal_desktop_capture_info = {
		.id = "pipewire-desktop-capture-source",
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_DO_NOT_DUPLICATE | OBS_SOURCE_CAP_OBSOLETE,
		.get_name = screencast_portal_desktop_capture_get_name,
		.create = screencast_portal_desktop_capture_create,
		.destroy = screencast_portal_capture_destroy,
		.save = screencast_portal_capture_save,
		.get_defaults = screencast_portal_capture_get_defaults,
		.get_properties = screencast_portal_capture_get_properties,
		.update = screencast_portal_capture_update,
		.show = screencast_portal_capture_show,
		.hide = screencast_portal_capture_hide,
		.get_width = screencast_portal_capture_get_width,
		.get_height = screencast_portal_capture_get_height,
		.video_render = screencast_portal_capture_video_render,
		.icon_type = OBS_ICON_TYPE_DESKTOP_CAPTURE,
	};
	if (desktop_capture_available)
		obs_register_source(&screencast_portal_desktop_capture_info);

	// Window capture
	const struct obs_source_info screencast_portal_window_capture_info = {
		.id = "pipewire-window-capture-source",
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_DO_NOT_DUPLICATE | OBS_SOURCE_CAP_OBSOLETE,
		.get_name = screencast_portal_window_capture_get_name,
		.create = screencast_portal_window_capture_create,
		.destroy = screencast_portal_capture_destroy,
		.save = screencast_portal_capture_save,
		.get_defaults = screencast_portal_capture_get_defaults,
		.get_properties = screencast_portal_capture_get_properties,
		.update = screencast_portal_capture_update,
		.show = screencast_portal_capture_show,
		.hide = screencast_portal_capture_hide,
		.get_width = screencast_portal_capture_get_width,
		.get_height = screencast_portal_capture_get_height,
		.video_render = screencast_portal_capture_video_render,
		.icon_type = OBS_ICON_TYPE_WINDOW_CAPTURE,
	};
	if (window_capture_available)
		obs_register_source(&screencast_portal_window_capture_info);

	// Screen capture (monitor and window)
	const struct obs_source_info screencast_portal_capture_info = {
		.id = "pipewire-screen-capture-source",
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_DO_NOT_DUPLICATE,
		.get_name = screencast_portal_desktop_capture_get_name,
		.create = screencast_portal_capture_create,
		.destroy = screencast_portal_capture_destroy,
		.save = screencast_portal_capture_save,
		.get_defaults = screencast_portal_capture_get_defaults,
		.get_properties = screencast_portal_capture_get_properties,
		.update = screencast_portal_capture_update,
		.show = screencast_portal_capture_show,
		.hide = screencast_portal_capture_hide,
		.get_width = screencast_portal_capture_get_width,
		.get_height = screencast_portal_capture_get_height,
		.video_render = screencast_portal_capture_video_render,
		.icon_type = OBS_ICON_TYPE_DESKTOP_CAPTURE,
	};
	obs_register_source(&screencast_portal_capture_info);
}

void screencast_portal_unload(void)
{
	g_clear_object(&screencast_proxy);
}
