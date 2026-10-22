/* Global hotkeys through the xdg-desktop-portal GlobalShortcuts interface
 * (upstream obsproject/obs-studio#13169, #10701, #10538).
 *
 * A fake org.freedesktop.portal.GlobalShortcuts runs on a private D-Bus
 * session bus (GTestDBus starts its own dbus-daemon; the user's session bus
 * is never touched). The portal client binds the combinations of the real
 * libobs hotkey bindings (obs-hotkey.c with an isolated core), and the fake
 * portal's Activated/Deactivated signals must press and release the hotkeys
 * through the real libobs hotkey thread. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <inttypes.h>
#include <stdio.h>
#include <string.h>

#include <gio/gio.h>

#include <obs-internal.h>
#include <util/profiler.h>

#include "obs-nix-hotkey-state.h"
#include "obs-nix-portal-shortcuts.h"

#define PORTAL_NAME "org.freedesktop.portal.Desktop"
#define PORTAL_PATH "/org/freedesktop/portal/desktop"
#define SHORTCUTS_IFACE "org.freedesktop.portal.GlobalShortcuts"
#define REGISTRY_IFACE "org.freedesktop.host.portal.Registry"

struct obs_core *hotkey_test_obs;

static struct obs_nix_hotkey_state key_state;
static pthread_t hotkey_thread;
static profiler_name_store_t *name_store;

bool obs_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	UNUSED_PARAMETER(context);
	return obs_nix_hotkey_state_is_pressed(&key_state, key);
}

profiler_name_store_t *obs_get_profiler_name_store(void)
{
	return name_store;
}

/* ------------------------------------------------------------------------ */
/* Fake portal                                                               */

static const char fake_portal_xml[] =
	"<node>"
	"  <interface name='org.freedesktop.portal.GlobalShortcuts'>"
	"    <method name='CreateSession'>"
	"      <arg type='a{sv}' name='options' direction='in'/>"
	"      <arg type='o' name='handle' direction='out'/>"
	"    </method>"
	"    <method name='BindShortcuts'>"
	"      <arg type='o' name='session_handle' direction='in'/>"
	"      <arg type='a(sa{sv})' name='shortcuts' direction='in'/>"
	"      <arg type='s' name='parent_window' direction='in'/>"
	"      <arg type='a{sv}' name='options' direction='in'/>"
	"      <arg type='o' name='request_handle' direction='out'/>"
	"    </method>"
	"    <method name='ConfigureShortcuts'>"
	"      <arg type='o' name='session_handle' direction='in'/>"
	"      <arg type='s' name='parent_window' direction='in'/>"
	"      <arg type='a{sv}' name='options' direction='in'/>"
	"    </method>"
	"    <method name='ListShortcuts'>"
	"      <arg type='o' name='session_handle' direction='in'/>"
	"      <arg type='a{sv}' name='options' direction='in'/>"
	"      <arg type='o' name='request_handle' direction='out'/>"
	"    </method>"
	"    <signal name='Activated'>"
	"      <arg type='o' name='session_handle'/><arg type='s' name='shortcut_id'/>"
	"      <arg type='t' name='timestamp'/><arg type='a{sv}' name='options'/>"
	"    </signal>"
	"    <signal name='Deactivated'>"
	"      <arg type='o' name='session_handle'/><arg type='s' name='shortcut_id'/>"
	"      <arg type='t' name='timestamp'/><arg type='a{sv}' name='options'/>"
	"    </signal>"
	"    <signal name='ShortcutsChanged'>"
	"      <arg type='o' name='session_handle'/><arg type='a(sa{sv})' name='shortcuts'/>"
	"    </signal>"
	"    <property name='version' type='u' access='read'/>"
	"  </interface>"
	"  <interface name='org.freedesktop.host.portal.Registry'>"
	"    <method name='Register'>"
	"      <arg type='s' name='app_id' direction='in'/>"
	"      <arg type='a{sv}' name='options' direction='in'/>"
	"    </method>"
	"  </interface>"
	"  <interface name='org.freedesktop.portal.Session'>"
	"    <method name='Close'/>"
	"  </interface>"
	"</node>";

struct fake_session {
	char *path;
	char *sender;
	guint registration;
	bool bound;
	bool closed;
};

static struct fake_portal {
	GTestDBus *bus;
	GDBusConnection *conn;
	GDBusNodeInfo *info;
	guint registration;
	guint registry_registration;
	struct fake_session sessions[16];
	size_t num_sessions;
	size_t num_binds;
	/* Response code of the next BindShortcuts: 0 success, 1 the user
	 * cancelled the desktop's dialog. */
	uint32_t bind_response;
	/* org.freedesktop.host.portal.Registry.Register calls */
	size_t num_registers;
	char *registered_app_id;
	/* Register came after a GlobalShortcuts call on that connection */
	bool register_too_late;
	/* Last BindShortcuts call: a(sa{sv}) */
	GVariant *last_shortcuts;
	char *last_bind_session;
	/* Like xdg-desktop-portal-hyprland: no trigger_description, the keys
	 * come from the compositor's config. */
	bool no_triggers;
	/* CreateSession fails with this D-Bus error message. */
	const char *create_error;
	/* GlobalShortcuts version property (default 1). */
	uint32_t version;
	/* Like KDE when the user leaves a new shortcut without a key in its
	 * dialog: this id comes back with an empty trigger. */
	const char *untriggered_id;
	/* ConfigureShortcuts calls (portal version 2) and the last session. */
	size_t num_configures;
	char *configure_session;
	/* ConfigureShortcuts fails with this D-Bus error message. */
	const char *configure_error;
	/* Like a dialog cancelled for one new shortcut: a BindShortcuts with
	 * this id answers 1 (cancelled); one without it succeeds. */
	const char *decline_with_id;
} fake;

static char *request_path(const char *sender, const char *token)
{
	char *escaped = g_strdup(sender + 1);
	g_strdelimit(escaped, ".", '_');
	char *path = g_strdup_printf("/org/freedesktop/portal/desktop/request/%s/%s", escaped, token);
	g_free(escaped);
	return path;
}

static void respond(const char *sender, const char *path, uint32_t response, GVariant *results)
{
	g_dbus_connection_emit_signal(fake.conn, sender, path, "org.freedesktop.portal.Request", "Response",
				      g_variant_new("(u@a{sv})", response, results), NULL);
}

static struct fake_session *find_session(const char *path)
{
	for (size_t i = 0; i < fake.num_sessions; i++) {
		if (strcmp(fake.sessions[i].path, path) == 0)
			return &fake.sessions[i];
	}
	return NULL;
}

static void session_method(GDBusConnection *conn, const char *sender, const char *path, const char *iface,
			   const char *method, GVariant *params, GDBusMethodInvocation *invocation, gpointer data)
{
	UNUSED_PARAMETER(conn);
	UNUSED_PARAMETER(sender);
	UNUSED_PARAMETER(iface);
	UNUSED_PARAMETER(method);
	UNUSED_PARAMETER(params);
	struct fake_session *session = data;
	UNUSED_PARAMETER(path);
	session->closed = true;
	g_dbus_method_invocation_return_value(invocation, NULL);
}

static const GDBusInterfaceVTable session_vtable = {session_method, NULL, NULL, {0}};

static void portal_method(GDBusConnection *conn, const char *sender, const char *path, const char *iface,
			  const char *method, GVariant *params, GDBusMethodInvocation *invocation, gpointer data)
{
	UNUSED_PARAMETER(path);
	UNUSED_PARAMETER(iface);
	UNUSED_PARAMETER(data);

	if (fake.num_registers == 0 && fake.num_sessions == 0)
		fake.register_too_late = true;

	if (strcmp(method, "ConfigureShortcuts") == 0) {
		const char *session_path, *parent;
		g_variant_get(params, "(&o&s@a{sv})", &session_path, &parent, NULL);
		g_free(fake.configure_session);
		fake.configure_session = g_strdup(session_path);
		fake.num_configures++;
		if (fake.configure_error) {
			g_dbus_method_invocation_return_dbus_error(invocation, "org.freedesktop.portal.Error.Failed",
								   fake.configure_error);
			return;
		}
		g_dbus_method_invocation_return_value(invocation, NULL);
		return;
	}

	if (strcmp(method, "CreateSession") == 0 && fake.create_error) {
		g_dbus_method_invocation_return_dbus_error(invocation, "org.freedesktop.portal.Error.Failed",
							   fake.create_error);
		return;
	}

	if (strcmp(method, "CreateSession") == 0) {
		g_autoptr(GVariant) options = NULL;
		const char *token = "t", *session_token = "s";
		g_variant_get(params, "(@a{sv})", &options);
		g_variant_lookup(options, "handle_token", "&s", &token);
		g_variant_lookup(options, "session_handle_token", "&s", &session_token);

		char *req = request_path(sender, token);
		char *escaped = g_strdup(sender + 1);
		g_strdelimit(escaped, ".", '_');
		char *session_path =
			g_strdup_printf("/org/freedesktop/portal/desktop/session/%s/%s", escaped, session_token);
		g_free(escaped);

		struct fake_session *session = &fake.sessions[fake.num_sessions++];
		session->path = session_path;
		session->sender = g_strdup(sender);
		GDBusInterfaceInfo *session_iface =
			g_dbus_node_info_lookup_interface(fake.info, "org.freedesktop.portal.Session");
		session->registration = g_dbus_connection_register_object(conn, session_path, session_iface,
									  &session_vtable, session, NULL, NULL);

		g_dbus_method_invocation_return_value(invocation, g_variant_new("(o)", req));

		GVariantBuilder results;
		g_variant_builder_init(&results, G_VARIANT_TYPE_VARDICT);
		/* xdg-desktop-portal sends the handle as a string. */
		g_variant_builder_add(&results, "{sv}", "session_handle", g_variant_new_string(session_path));
		respond(sender, req, 0, g_variant_builder_end(&results));
		g_free(req);
		return;
	}

	if (strcmp(method, "BindShortcuts") == 0) {
		const char *session_path, *parent;
		g_autoptr(GVariant) shortcuts = NULL;
		g_autoptr(GVariant) options = NULL;
		const char *token = "t";
		g_variant_get(params, "(&o@a(sa{sv})&s@a{sv})", &session_path, &shortcuts, &parent, &options);
		g_variant_lookup(options, "handle_token", "&s", &token);

		struct fake_session *session = find_session(session_path);
		if (!session || session->closed || session->bound) {
			/* Like xdg-desktop-portal: one BindShortcuts per session. */
			g_dbus_method_invocation_return_dbus_error(invocation, "org.freedesktop.portal.Error.Failed",
								   "Invalid session state");
			return;
		}
		session->bound = true;
		fake.num_binds++;
		g_clear_pointer(&fake.last_shortcuts, g_variant_unref);
		fake.last_shortcuts = g_variant_ref(shortcuts);
		g_free(fake.last_bind_session);
		fake.last_bind_session = g_strdup(session_path);

		char *req = request_path(sender, token);
		g_dbus_method_invocation_return_value(invocation, g_variant_new("(o)", req));

		/* Echo the shortcuts back with a trigger description. */
		GVariantBuilder list;
		g_variant_builder_init(&list, G_VARIANT_TYPE("a(sa{sv})"));
		GVariantIter iter;
		const char *id;
		GVariant *props;
		g_variant_iter_init(&iter, shortcuts);
		while (g_variant_iter_next(&iter, "(&s@a{sv})", &id, &props)) {
			const char *trigger = "";
			const char *description = "";
			g_variant_lookup(props, "preferred_trigger", "&s", &trigger);
			g_variant_lookup(props, "description", "&s", &description);
			GVariantBuilder out;
			g_variant_builder_init(&out, G_VARIANT_TYPE_VARDICT);
			g_variant_builder_add(&out, "{sv}", "description", g_variant_new_string(description));
			g_variant_builder_add(
				&out, "{sv}", "trigger_description",
				g_variant_new_string(fake.no_triggers || (fake.untriggered_id &&
									  strcmp(id, fake.untriggered_id) == 0)
							     ? ""
							     : trigger));
			g_variant_builder_add(&list, "(s@a{sv})", id, g_variant_builder_end(&out));
			g_variant_unref(props);
		}
		uint32_t response = fake.bind_response;
		if (fake.decline_with_id) {
			const char *requested;
			GVariant *requested_props;
			g_variant_iter_init(&iter, shortcuts);
			while (g_variant_iter_next(&iter, "(&s@a{sv})", &requested, &requested_props)) {
				if (strcmp(requested, fake.decline_with_id) == 0)
					response = 1;
				g_variant_unref(requested_props);
			}
		}
		GVariantBuilder results;
		g_variant_builder_init(&results, G_VARIANT_TYPE_VARDICT);
		g_variant_builder_add(&results, "{sv}", "shortcuts", g_variant_builder_end(&list));
		respond(sender, req, response, g_variant_builder_end(&results));
		g_free(req);
		return;
	}

	g_dbus_method_invocation_return_dbus_error(invocation, "org.freedesktop.DBus.Error.UnknownMethod", method);
}

static void registry_method(GDBusConnection *conn, const char *sender, const char *path, const char *iface,
			    const char *method, GVariant *params, GDBusMethodInvocation *invocation, gpointer data)
{
	UNUSED_PARAMETER(conn);
	UNUSED_PARAMETER(sender);
	UNUSED_PARAMETER(path);
	UNUSED_PARAMETER(iface);
	UNUSED_PARAMETER(method);
	UNUSED_PARAMETER(data);
	const char *app_id;
	g_variant_get(params, "(&s@a{sv})", &app_id, NULL);
	g_free(fake.registered_app_id);
	fake.registered_app_id = g_strdup(app_id);
	fake.num_registers++;
	g_dbus_method_invocation_return_value(invocation, NULL);
}

static const GDBusInterfaceVTable registry_vtable = {registry_method, NULL, NULL, {0}};

static GVariant *portal_property(GDBusConnection *conn, const char *sender, const char *path, const char *iface,
				 const char *property, GError **error, gpointer data)
{
	UNUSED_PARAMETER(conn);
	UNUSED_PARAMETER(sender);
	UNUSED_PARAMETER(path);
	UNUSED_PARAMETER(iface);
	UNUSED_PARAMETER(error);
	UNUSED_PARAMETER(data);
	if (fake.num_registers == 0)
		fake.register_too_late = true;
	if (strcmp(property, "version") == 0)
		return g_variant_new_uint32(fake.version ? fake.version : 1);
	return NULL;
}

static const GDBusInterfaceVTable portal_vtable = {portal_method, portal_property, NULL, {0}};

static void pump(void)
{
	while (g_main_context_iteration(NULL, FALSE))
		;
}

/* Runs the fake portal while waiting for cond. */
static bool wait_until(bool (*cond)(void *), void *data)
{
	for (int i = 0; i < 400; i++) {
		pump();
		if (cond(data))
			return true;
		os_sleep_ms(10);
	}
	return false;
}

static void serve_for(int ms)
{
	for (int i = 0; i < ms / 10; i++) {
		pump();
		os_sleep_ms(10);
	}
}

/* Takes or drops the portal's well-known name, as a portal starting or
 * exiting does. */
static void fake_portal_own_name(bool own)
{
	g_autoptr(GError) error = NULL;
	g_autoptr(GVariant) reply = g_dbus_connection_call_sync(
		fake.conn, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
		own ? "RequestName" : "ReleaseName",
		own ? g_variant_new("(su)", PORTAL_NAME, 4 /* DO_NOT_QUEUE */) : g_variant_new("(s)", PORTAL_NAME),
		G_VARIANT_TYPE("(u)"), G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);
	assert_non_null(reply);
	g_dbus_connection_flush_sync(fake.conn, NULL, NULL);
}

static void fake_portal_start(bool own_portal)
{
	memset(&fake, 0, sizeof(fake));
	fake.bus = g_test_dbus_new(G_TEST_DBUS_NONE);
	g_test_dbus_up(fake.bus);

	g_autoptr(GError) error = NULL;
	fake.conn = g_dbus_connection_new_for_address_sync(g_test_dbus_get_bus_address(fake.bus),
							   G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT |
								   G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION,
							   NULL, NULL, &error);
	assert_non_null(fake.conn);

	if (!own_portal)
		return;

	fake.info = g_dbus_node_info_new_for_xml(fake_portal_xml, &error);
	assert_non_null(fake.info);
	fake.registration = g_dbus_connection_register_object(
		fake.conn, PORTAL_PATH, g_dbus_node_info_lookup_interface(fake.info, SHORTCUTS_IFACE), &portal_vtable,
		NULL, NULL, &error);
	assert_true(fake.registration > 0);
	fake.registry_registration = g_dbus_connection_register_object(
		fake.conn, PORTAL_PATH, g_dbus_node_info_lookup_interface(fake.info, REGISTRY_IFACE), &registry_vtable,
		NULL, NULL, &error);
	assert_true(fake.registry_registration > 0);

	fake_portal_own_name(true);
}

static void fake_portal_stop(void)
{
	pump();
	for (size_t i = 0; i < fake.num_sessions; i++) {
		if (fake.sessions[i].registration)
			g_dbus_connection_unregister_object(fake.conn, fake.sessions[i].registration);
		g_free(fake.sessions[i].path);
		g_free(fake.sessions[i].sender);
	}
	if (fake.registration)
		g_dbus_connection_unregister_object(fake.conn, fake.registration);
	if (fake.registry_registration)
		g_dbus_connection_unregister_object(fake.conn, fake.registry_registration);
	g_clear_pointer(&fake.registered_app_id, g_free);
	g_clear_pointer(&fake.last_shortcuts, g_variant_unref);
	g_clear_pointer(&fake.last_bind_session, g_free);
	g_clear_pointer(&fake.configure_session, g_free);
	g_clear_pointer(&fake.info, g_dbus_node_info_unref);
	g_dbus_connection_close_sync(fake.conn, NULL, NULL);
	g_clear_object(&fake.conn);
	g_test_dbus_down(fake.bus);
	g_clear_object(&fake.bus);
}

static void emit_shortcut(const char *session_path, const char *id, bool active)
{
	const char *sender = fake.sessions[fake.num_sessions - 1].sender;
	g_dbus_connection_emit_signal(fake.conn, sender, PORTAL_PATH, SHORTCUTS_IFACE,
				      active ? "Activated" : "Deactivated",
				      g_variant_new("(ost@a{sv})", session_path, id, (guint64)0,
						    g_variant_new_array(G_VARIANT_TYPE("{sv}"), NULL, 0)),
				      NULL);
	g_dbus_connection_flush_sync(fake.conn, NULL, NULL);
}

/* The user gave a shortcut another trigger in the desktop settings. */
static void emit_shortcuts_changed(const char *session_path, const char *id, const char *trigger)
{
	const char *sender = fake.sessions[fake.num_sessions - 1].sender;
	GVariantBuilder props;
	g_variant_builder_init(&props, G_VARIANT_TYPE_VARDICT);
	g_variant_builder_add(&props, "{sv}", "description", g_variant_new_string("changed"));
	g_variant_builder_add(&props, "{sv}", "trigger_description", g_variant_new_string(trigger));
	GVariantBuilder list;
	g_variant_builder_init(&list, G_VARIANT_TYPE("a(sa{sv})"));
	g_variant_builder_add(&list, "(s@a{sv})", id, g_variant_builder_end(&props));
	g_dbus_connection_emit_signal(fake.conn, sender, PORTAL_PATH, SHORTCUTS_IFACE, "ShortcutsChanged",
				      g_variant_new("(o@a(sa{sv}))", session_path, g_variant_builder_end(&list)), NULL);
	g_dbus_connection_flush_sync(fake.conn, NULL, NULL);
}

static void emit_session_closed(struct fake_session *session)
{
	session->closed = true;
	g_dbus_connection_emit_signal(fake.conn, session->sender, session->path, "org.freedesktop.portal.Session",
				      "Closed",
				      g_variant_new("(@a{sv})", g_variant_new_array(G_VARIANT_TYPE("{sv}"), NULL, 0)),
				      NULL);
	g_dbus_connection_flush_sync(fake.conn, NULL, NULL);
}

/* ------------------------------------------------------------------------ */
/* libobs side                                                               */

struct hotkey_log {
	volatile long pressed;
	volatile long presses;
};

static void hotkey_callback(void *data, obs_hotkey_id id, obs_hotkey_t *hotkey, bool pressed)
{
	UNUSED_PARAMETER(id);
	UNUSED_PARAMETER(hotkey);
	struct hotkey_log *log = data;
	os_atomic_set_long(&log->pressed, pressed);
	if (pressed)
		os_atomic_inc_long(&log->presses);
}

static bool test_key_name(obs_key_t key, struct dstr *name)
{
	const char *full = obs_key_to_name(key);
	if (strncmp(full, "OBS_KEY_", 8) != 0 || key == OBS_KEY_NONE)
		return false;
	dstr_copy(name, full + 8);
	if (name->len == 1)
		dstr_to_lower(name);
	return true;
}

static bool dbus_daemon_available(void)
{
	char *path = g_find_program_in_path("dbus-daemon");
	bool found = path != NULL;
	g_free(path);
	return found;
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	if (!dbus_daemon_available())
		return 0;
	memset(&key_state, 0, sizeof(key_state));
	name_store = profiler_name_store_create();
	hotkey_test_obs = bzalloc(sizeof(*hotkey_test_obs));
	assert_int_equal(pthread_mutex_init_recursive(&hotkey_test_obs->hotkeys.mutex), 0);
	assert_int_equal(os_event_init(&hotkey_test_obs->hotkeys.stop_event, OS_EVENT_TYPE_MANUAL), 0);
	hotkey_test_obs->hotkeys.strict_modifiers = true;
	hotkey_test_obs->hotkeys.signals = signal_handler_create();
	static const char *signals[] = {"void hotkey_register(ptr key)",
					"void hotkey_unregister(ptr key)",
					"void hotkey_bindings_changed(ptr key)",
					"void hotkey_background_press_changed(bool enabled)",
					"void hotkey_background_status_changed()",
					"void hotkey_background_probe(string id)",
					"void source_rename(ptr source, string new_name, string prev_name)",
					NULL};
	assert_true(signal_handler_add_array(hotkey_test_obs->hotkeys.signals, signals));
	assert_int_equal(pthread_create(&hotkey_thread, NULL, obs_hotkey_thread, NULL), 0);
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (!hotkey_test_obs)
		return 0;
	os_event_signal(hotkey_test_obs->hotkeys.stop_event);
	pthread_join(hotkey_thread, NULL);
	os_event_destroy(hotkey_test_obs->hotkeys.stop_event);
	obs_hotkeys_free();
	/* What obs_free_hotkeys() (obs.c, not built here) frees after it (#464). */
	obs_hotkeys_background_clear_shortcuts(&hotkey_test_obs->hotkeys.background);
	da_free(hotkey_test_obs->hotkeys.background.shortcuts);
	bfree(hotkey_test_obs->hotkeys.background.error);
	obs_nix_hotkey_state_free(&key_state);
	signal_handler_destroy(hotkey_test_obs->hotkeys.signals);
	pthread_mutex_destroy(&hotkey_test_obs->hotkeys.mutex);
	bfree(hotkey_test_obs);
	hotkey_test_obs = NULL;
	profiler_name_store_free(name_store);
	name_store = NULL;
	return 0;
}

#define REQUIRE_DBUS_DAEMON()                                                           \
	do {                                                                            \
		if (!dbus_daemon_available()) {                                         \
			fprintf(stderr, "dbus-daemon not found; skipping portal test\n"); \
			skip();                                                         \
		}                                                                       \
	} while (false)

static obs_hotkey_id register_bound(const char *name, struct hotkey_log *log, obs_key_combination_t combo)
{
	obs_hotkey_id id = obs_hotkey_register_frontend(name, name, hotkey_callback, log);
	assert_int_not_equal(id, OBS_INVALID_HOTKEY_ID);
	obs_hotkey_load_bindings(id, &combo, 1);
	return id;
}

static bool binds_at_least(void *data)
{
	return fake.num_binds >= (size_t)(uintptr_t)data;
}

static bool session_is_closed(void *data)
{
	return ((struct fake_session *)data)->closed;
}

static bool log_pressed(void *data)
{
	return os_atomic_load_long(&((struct hotkey_log *)data)->pressed) != 0;
}

static bool log_released(void *data)
{
	return os_atomic_load_long(&((struct hotkey_log *)data)->pressed) == 0;
}

static bool bound_shortcut(const char *want_id, const char **trigger, const char **description);

/* No further BindShortcuts arrives while the fake portal runs for ms. */
static void expect_no_bind_after(size_t count, int ms, const char *why)
{
	serve_for(ms);
	if (fake.num_binds != count) {
		fprintf(stderr, "%s: expected %zu BindShortcuts calls, got %zu\n", why, count, fake.num_binds);
		fail();
	}
}

static bool bound_has(const char *want_id)
{
	const char *trigger, *description;
	return bound_shortcut(want_id, &trigger, &description);
}

static void expect_bind(size_t count)
{
	if (!wait_until(binds_at_least, (void *)(uintptr_t)count)) {
		fprintf(stderr, "portal never received BindShortcuts #%zu (sessions %zu, binds %zu)\n", count,
			fake.num_sessions, fake.num_binds);
		fail();
	}
}

/* Shortcut id -> preferred_trigger of the last BindShortcuts call. */
static bool bound_shortcut(const char *want_id, const char **trigger, const char **description)
{
	GVariantIter iter;
	const char *id;
	GVariant *props;
	bool found = false;
	g_variant_iter_init(&iter, fake.last_shortcuts);
	while (g_variant_iter_next(&iter, "(&s@a{sv})", &id, &props)) {
		if (!found && strcmp(id, want_id) == 0) {
			found = true;
			*trigger = NULL;
			*description = NULL;
			g_variant_lookup(props, "preferred_trigger", "&s", trigger);
			g_variant_lookup(props, "description", "&s", description);
		}
		g_variant_unref(props);
	}
	return found;
}

static const obs_key_combination_t f13 = {0, OBS_KEY_F13};
static const obs_key_combination_t f14 = {0, OBS_KEY_F14};
static const obs_key_combination_t ctrl_shift_a = {INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY, OBS_KEY_A};

static void shortcut_ids_round_trip(void **state)
{
	UNUSED_PARAMETER(state);
	const obs_key_combination_t combos[] = {
		f13,
		ctrl_shift_a,
		{INTERACT_ALT_KEY | INTERACT_COMMAND_KEY, OBS_KEY_F1},
		{INTERACT_SHIFT_KEY | INTERACT_CONTROL_KEY | INTERACT_ALT_KEY | INTERACT_COMMAND_KEY, OBS_KEY_SPACE},
	};
	for (size_t i = 0; i < sizeof(combos) / sizeof(combos[0]); i++) {
		struct dstr id = {0};
		obs_key_combination_t back = {0, OBS_KEY_NONE};
		if (!obs_nix_portal_shortcut_id(combos[i], &id) || !obs_nix_portal_shortcut_combo(id.array, &back)) {
			fprintf(stderr, "combination %zu has no shortcut id round trip (id '%s')\n", i,
				id.array ? id.array : "");
			dstr_free(&id);
			fail();
		}
		assert_int_equal(back.key, combos[i].key);
		assert_int_equal(back.modifiers, combos[i].modifiers);
		dstr_free(&id);
	}

	struct dstr id = {0};
	assert_true(obs_nix_portal_shortcut_id(ctrl_shift_a, &id));
	assert_string_equal(id.array, "ctrl+shift+OBS_KEY_A");
	dstr_free(&id);

	obs_key_combination_t combo;
	assert_false(obs_nix_portal_shortcut_combo("", &combo));
	assert_false(obs_nix_portal_shortcut_combo("ctrl+", &combo));
	assert_false(obs_nix_portal_shortcut_combo("hyper+OBS_KEY_A", &combo));
	assert_false(obs_nix_portal_shortcut_combo("OBS_KEY_NOT_A_KEY", &combo));
	assert_false(obs_nix_portal_shortcut_combo("some-other-app-shortcut", &combo));

	struct dstr trigger = {0};
	assert_true(obs_nix_portal_shortcut_trigger(ctrl_shift_a, test_key_name, &trigger));
	assert_string_equal(trigger.array, "CTRL+SHIFT+a");
	assert_true(obs_nix_portal_shortcut_trigger((obs_key_combination_t){INTERACT_ALT_KEY | INTERACT_COMMAND_KEY,
									    OBS_KEY_F13},
						    test_key_name, &trigger));
	assert_string_equal(trigger.array, "ALT+LOGO+F13");
	dstr_free(&trigger);
}

/* One portal shortcut per distinct combination, with the OBS binding as the
 * preferred trigger; then Activated/Deactivated press and release every
 * hotkey bound to it through the libobs hotkey thread. */
static void portal_drives_push_to_talk(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0}, mute = {0}, record = {0};
	register_bound("push-to-talk", &talk, f13);
	register_bound("push-to-mute", &mute, f13);
	register_bound("record", &record, ctrl_shift_a);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);

	/* Host app id, registered before any GlobalShortcuts call. */
	if (fake.num_registers != 1 || fake.register_too_late || !fake.registered_app_id) {
		fprintf(stderr, "Registry.Register: %zu calls, %s, app id '%s'\n", fake.num_registers,
			fake.register_too_late ? "after a GlobalShortcuts call" : "in time",
			fake.registered_app_id ? fake.registered_app_id : "");
		fail();
	}
	assert_string_equal(fake.registered_app_id, "com.obsproject.Studio");

	assert_int_equal(g_variant_n_children(fake.last_shortcuts), 2);
	const char *trigger, *description;
	assert_true(bound_shortcut("OBS_KEY_F13", &trigger, &description));
	assert_non_null(trigger);
	assert_string_equal(trigger, "F13");
	assert_non_null(description);
	assert_true(bound_shortcut("ctrl+shift+OBS_KEY_A", &trigger, &description));
	assert_non_null(trigger);
	assert_string_equal(trigger, "CTRL+SHIFT+a");

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	if (!wait_until(log_pressed, &talk) || !wait_until(log_pressed, &mute)) {
		fprintf(stderr, "Activated did not press the hotkeys (talk %ld, mute %ld)\n", talk.pressed,
			mute.pressed);
		fail();
	}
	serve_for(150);
	assert_true(os_atomic_load_long(&talk.pressed));
	assert_int_equal(os_atomic_load_long(&record.presses), 0);

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", false);
	if (!wait_until(log_released, &talk) || !wait_until(log_released, &mute)) {
		fprintf(stderr, "Deactivated did not release the hotkeys\n");
		fail();
	}

	emit_shortcut(fake.last_bind_session, "ctrl+shift+OBS_KEY_A", true);
	assert_true(wait_until(log_pressed, &record));
	emit_shortcut(fake.last_bind_session, "ctrl+shift+OBS_KEY_A", false);
	assert_true(wait_until(log_released, &record));

	/* The user moves the shortcut to another trigger: the id still names
	 * the OBS combination, so nothing is rebound and it still drives the
	 * same hotkeys. */
	emit_shortcuts_changed(fake.last_bind_session, "OBS_KEY_F13", "Pause");
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	assert_true(wait_until(log_pressed, &talk));
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", false);
	assert_true(wait_until(log_released, &talk));

	/* An id this client never bound presses nothing. */
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F14", true);
	serve_for(150);
	assert_false(obs_nix_hotkey_state_is_pressed(&key_state, OBS_KEY_F14));

	/* Nothing changed, so nothing is bound again. */
	expect_no_bind_after(1, 1000, "unchanged bindings");

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* Changing the bindings rebinds in a new session (the portal accepts one
 * BindShortcuts per session), and the old session is closed. */
static void rebinds_when_bindings_change(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0};
	obs_hotkey_id id = register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	char *first_session = g_strdup(fake.last_bind_session);

	obs_hotkey_load_bindings(id, (obs_key_combination_t *)&f14, 1);
	expect_bind(2);

	assert_string_not_equal(first_session, fake.last_bind_session);
	struct fake_session *old = find_session(first_session);
	assert_non_null(old);
	assert_true(old->closed);
	const char *trigger, *description;
	assert_int_equal(g_variant_n_children(fake.last_shortcuts), 1);
	assert_true(bound_shortcut("OBS_KEY_F14", &trigger, &description));

	/* A late signal from the closed session must not press anything. */
	emit_shortcut(first_session, "OBS_KEY_F14", true);
	serve_for(150);
	assert_int_equal(os_atomic_load_long(&talk.presses), 0);

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F14", true);
	assert_true(wait_until(log_pressed, &talk));
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F14", false);
	assert_true(wait_until(log_released, &talk));

	/* The last hotkey goes away: the session is closed, nothing bound. */
	struct fake_session *second = find_session(fake.last_bind_session);
	assert_non_null(second);
	obs_hotkey_unregister(id);
	if (!wait_until(session_is_closed, second)) {
		fprintf(stderr, "session not closed after the last hotkey was unregistered\n");
		fail();
	}
	expect_no_bind_after(2, 300, "no hotkeys left");

	g_free(first_session);
	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* Many binding changes in a row (loading a profile or a scene collection)
 * are bound once, with the final set. */
static void burst_of_changes_binds_once(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0}, other = {0};
	obs_hotkey_id id = register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);

	const obs_key_combination_t steps[] = {f14, ctrl_shift_a, f13, f14};
	for (size_t i = 0; i < sizeof(steps) / sizeof(steps[0]); i++) {
		obs_hotkey_load_bindings(id, (obs_key_combination_t *)&steps[i], 1);
		serve_for(50);
	}
	register_bound("other", &other, ctrl_shift_a);

	expect_bind(2);
	expect_no_bind_after(2, 1000, "one burst of binding changes");
	assert_int_equal(g_variant_n_children(fake.last_shortcuts), 2);
	assert_true(bound_has("OBS_KEY_F14"));
	assert_true(bound_has("ctrl+shift+OBS_KEY_A"));

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* The desktop closes the session (the user revoked it): held hotkeys are
 * released and the same shortcuts are not bound again behind the user's
 * back; assigning a new combination in OBS binds again. */
static void closed_session_is_not_rebound(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0}, record = {0};
	register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	assert_true(wait_until(log_pressed, &talk));

	struct fake_session *session = find_session(fake.last_bind_session);
	assert_non_null(session);
	emit_session_closed(session);
	if (!wait_until(log_released, &talk)) {
		fprintf(stderr, "push-to-talk still held after the desktop closed the session\n");
		fail();
	}
	expect_no_bind_after(1, 1500, "session closed by the desktop");

	register_bound("record", &record, ctrl_shift_a);
	expect_bind(2);
	assert_string_not_equal(session->path, fake.last_bind_session);
	assert_true(bound_has("OBS_KEY_F13"));
	assert_true(bound_has("ctrl+shift+OBS_KEY_A"));

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	assert_true(wait_until(log_pressed, &talk));
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", false);
	assert_true(wait_until(log_released, &talk));

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* The portal restarts (its bus name goes away and comes back): held hotkeys
 * are released, and the client registers its app id with the new instance
 * and binds again. */
static void rebinds_after_portal_restart(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0};
	register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	char *first_session = g_strdup(fake.last_bind_session);

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	assert_true(wait_until(log_pressed, &talk));

	fake_portal_own_name(false);
	if (!wait_until(log_released, &talk)) {
		fprintf(stderr, "push-to-talk still held after the portal left the bus\n");
		fail();
	}
	expect_no_bind_after(1, 300, "portal gone");

	fake_portal_own_name(true);
	expect_bind(2);
	assert_string_not_equal(first_session, fake.last_bind_session);
	assert_int_equal(fake.num_registers, 2);

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	assert_true(wait_until(log_pressed, &talk));
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", false);
	assert_true(wait_until(log_released, &talk));

	g_free(first_session);
	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* No portal on the bus: the client gives up quietly, nothing is pressed, and
 * stopping it does not hang. */
static void no_portal_falls_back(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(false);

	struct hotkey_log talk = {0};
	register_bound("push-to-talk", &talk, f13);

	uint64_t start = os_gettime_ns();
	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	serve_for(300);
	obs_nix_portal_shortcuts_stop(portal);
	uint64_t elapsed_ms = (os_gettime_ns() - start) / 1000000;

	assert_int_equal(os_atomic_load_long(&talk.presses), 0);
	for (obs_key_t key = 0; key < OBS_KEY_LAST_VALUE; key++)
		assert_false(obs_nix_hotkey_state_is_pressed(&key_state, key));
	if (elapsed_ms > 3000) {
		fprintf(stderr, "stopping the portal client took %" PRIu64 " ms\n", elapsed_ms);
		fail();
	}
	fake_portal_stop();
}

/* Settings -> Advanced -> "Disable hotkeys when main window is not in
 * focus" (and the settings dialog itself, which disables presses while it
 * is open) call obs_hotkey_enable_background_press(false). The hotkey
 * thread then drops every press, so the desktop must not keep holding the
 * keys for OBS: the session is closed while presses are disabled, a held
 * shortcut is released, and nothing is bound until presses are enabled
 * again, when the bindings of that moment are bound. */
static void disabled_background_press_closes_session(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0}, record = {0};
	register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	struct fake_session *first = find_session(fake.last_bind_session);
	assert_non_null(first);

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	assert_true(wait_until(log_pressed, &talk));

	obs_hotkey_enable_background_press(false);
	if (!wait_until(session_is_closed, first)) {
		fprintf(stderr, "session still open while background presses are disabled\n");
		fail();
	}
	if (!wait_until(log_released, &talk)) {
		fprintf(stderr, "push-to-talk still held after background presses were disabled\n");
		fail();
	}

	/* A change while disabled is not bound yet, so no desktop prompt. */
	register_bound("record", &record, ctrl_shift_a);
	expect_no_bind_after(1, 1000, "background presses disabled");

	obs_hotkey_enable_background_press(true);
	expect_bind(2);
	assert_string_not_equal(first->path, fake.last_bind_session);
	assert_int_equal(g_variant_n_children(fake.last_shortcuts), 2);
	assert_true(bound_has("OBS_KEY_F13"));
	assert_true(bound_has("ctrl+shift+OBS_KEY_A"));

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	assert_true(wait_until(log_pressed, &talk));
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", false);
	assert_true(wait_until(log_released, &talk));

	/* Enabling what is already enabled changes nothing. */
	obs_hotkey_enable_background_press(true);
	expect_no_bind_after(2, 300, "already enabled");

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* OBS started unfocused with the setting on: presses are disabled before
 * the client starts, so nothing is bound until they are enabled. */
static void disabled_at_start_binds_once_enabled(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0};
	register_bound("push-to-talk", &talk, f13);
	obs_hotkey_enable_background_press(false);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_no_bind_after(0, 1000, "disabled before start");
	assert_int_equal(fake.num_sessions, 0);

	obs_hotkey_enable_background_press(true);
	expect_bind(1);
	assert_true(bound_has("OBS_KEY_F13"));

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	assert_true(wait_until(log_pressed, &talk));
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", false);
	assert_true(wait_until(log_released, &talk));

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* A set the user declined in the desktop's dialog is not asked for again
 * because presses were disabled and enabled (every focus change, with the
 * setting on); a new combination still binds. */
static void declined_set_stays_declined_across_disable(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);
	fake.bind_response = 1;

	struct hotkey_log talk = {0}, record = {0};
	register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	serve_for(300);

	obs_hotkey_enable_background_press(false);
	serve_for(300);
	obs_hotkey_enable_background_press(true);
	expect_no_bind_after(1, 1000, "declined set after a focus change");

	fake.bind_response = 0;
	register_bound("record", &record, ctrl_shift_a);
	expect_bind(2);
	assert_true(bound_has("OBS_KEY_F13"));
	assert_true(bound_has("ctrl+shift+OBS_KEY_A"));

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* ------------------------------------------------------------------------ */
/* Status for Settings -> Hotkeys (obs_hotkeys_get_background_state)        */

static enum obs_hotkeys_background_state want_state;

static bool state_is(void *data)
{
	UNUSED_PARAMETER(data);
	return obs_hotkeys_get_background_state(NULL) == want_state;
}

static const char *state_name(enum obs_hotkeys_background_state state)
{
	switch (state) {
	case OBS_HOTKEYS_BACKGROUND_NATIVE:
		return "native";
	case OBS_HOTKEYS_BACKGROUND_STARTING:
		return "starting";
	case OBS_HOTKEYS_BACKGROUND_UNAVAILABLE:
		return "unavailable";
	case OBS_HOTKEYS_BACKGROUND_IDLE:
		return "idle";
	case OBS_HOTKEYS_BACKGROUND_WAITING:
		return "waiting";
	case OBS_HOTKEYS_BACKGROUND_ACTIVE:
		return "active";
	case OBS_HOTKEYS_BACKGROUND_DECLINED:
		return "declined";
	case OBS_HOTKEYS_BACKGROUND_CLOSED:
		return "closed";
	case OBS_HOTKEYS_BACKGROUND_FAILED:
		return "failed";
	}
	return "?";
}

static void expect_state(enum obs_hotkeys_background_state state, const char *why)
{
	want_state = state;
	if (!wait_until(state_is, NULL)) {
		fprintf(stderr, "%s: background hotkeys are '%s', expected '%s'\n", why,
			state_name(obs_hotkeys_get_background_state(NULL)), state_name(state));
		fail();
	}
}

static uint32_t background_flags(void)
{
	uint32_t flags = 0;
	obs_hotkeys_get_background_state(&flags);
	return flags;
}

struct shortcut_info {
	obs_key_combination_t combo;
	bool found;
	bool bound;
	char trigger[64];
	size_t count;
};

static bool find_shortcut(void *data, obs_key_combination_t combo, const char *trigger, bool bound)
{
	struct shortcut_info *info = data;
	info->count++;
	if (combo.key == info->combo.key && combo.modifiers == info->combo.modifiers) {
		info->found = true;
		info->bound = bound;
		snprintf(info->trigger, sizeof(info->trigger), "%s", trigger ? trigger : "(none)");
	}
	return true;
}

static struct shortcut_info shortcut_info(obs_key_combination_t combo)
{
	struct shortcut_info info = {.combo = combo};
	obs_hotkeys_enum_background_shortcuts(find_shortcut, &info);
	return info;
}

static bool trigger_is(void *data)
{
	struct shortcut_info info = shortcut_info(f13);
	return info.found && strcmp(info.trigger, (const char *)data) == 0;
}

static volatile long status_signals;

static void count_status(void *data, calldata_t *params)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(params);
	os_atomic_inc_long(&status_signals);
}

static char probed_id[64];
static volatile long probes;

static void record_probe(void *data, calldata_t *params)
{
	UNUSED_PARAMETER(data);
	snprintf(probed_id, sizeof(probed_id), "%s", calldata_string(params, "id"));
	os_atomic_inc_long(&probes);
}

static bool probed(void *data)
{
	return os_atomic_load_long(&probes) >= (long)(uintptr_t)data;
}

/* Without a platform client (X11, Windows, macOS) OBS sees background
 * presses itself: every combination works in the background. */
static void native_without_portal_client(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();

	uint32_t flags = 1234;
	assert_int_equal(obs_hotkeys_get_background_state(&flags), OBS_HOTKEYS_BACKGROUND_NATIVE);
	assert_int_equal(flags, 0);
	assert_null(obs_hotkeys_get_background_app_id());
	assert_null(obs_hotkeys_get_background_error());
	assert_true(obs_hotkey_combination_is_background(f13));
	assert_true(obs_hotkey_combination_is_background((obs_key_combination_t){0, OBS_KEY_MOUSE4}));
	assert_null(obs_hotkey_combination_get_background_id(f13));
	assert_null(obs_hotkey_combination_get_background_trigger(f13));
}

/* KDE/GNOME-like desktop: it reports the trigger it assigned, so Settings
 * can show it per row; a trigger changed in the desktop shows up too. */
static void status_reports_bound_shortcuts(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);
	signal_handler_connect(hotkey_test_obs->hotkeys.signals, "hotkey_background_status_changed", count_status,
			       NULL);
	os_atomic_set_long(&status_signals, 0);

	struct hotkey_log talk = {0}, record = {0}, mouse = {0};
	register_bound("push-to-talk", &talk, f13);
	register_bound("record", &record, ctrl_shift_a);
	register_bound("mouse", &mouse, (obs_key_combination_t){0, OBS_KEY_MOUSE4});

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "after BindShortcuts");
	assert_true(os_atomic_load_long(&status_signals) > 0);

	uint32_t flags = background_flags();
	assert_true(flags & OBS_HOTKEYS_BACKGROUND_DESKTOP_TRIGGERS);
	assert_false(flags & OBS_HOTKEYS_BACKGROUND_PAUSED);
	assert_false(flags & OBS_HOTKEYS_BACKGROUND_CONFIGURABLE);
	assert_string_equal(obs_hotkeys_get_background_app_id(), "com.obsproject.Studio");

	struct shortcut_info info = shortcut_info(f13);
	assert_true(info.found);
	assert_true(info.bound);
	assert_string_equal(info.trigger, "F13");
	assert_int_equal(info.count, 2);
	info = shortcut_info(ctrl_shift_a);
	assert_true(info.bound);
	assert_string_equal(info.trigger, "CTRL+SHIFT+a");
	assert_false(shortcut_info((obs_key_combination_t){0, OBS_KEY_MOUSE4}).found);

	/* Per-row markers: mouse buttons and modifier-only stay focus-only. */
	assert_true(obs_hotkey_combination_is_background(f13));
	assert_false(obs_hotkey_combination_is_background((obs_key_combination_t){0, OBS_KEY_MOUSE4}));
	assert_false(obs_hotkey_combination_is_background((obs_key_combination_t){INTERACT_CONTROL_KEY, OBS_KEY_NONE}));
	char *id = obs_hotkey_combination_get_background_id(ctrl_shift_a);
	assert_non_null(id);
	assert_string_equal(id, "ctrl+shift+OBS_KEY_A");
	bfree(id);
	assert_null(obs_hotkey_combination_get_background_id((obs_key_combination_t){0, OBS_KEY_MOUSE4}));
	char *trigger = obs_hotkey_combination_get_background_trigger(ctrl_shift_a);
	assert_non_null(trigger);
	assert_string_equal(trigger, "CTRL+SHIFT+a");
	bfree(trigger);

	emit_shortcuts_changed(fake.last_bind_session, "OBS_KEY_F13", "Pause");
	if (!wait_until(trigger_is, "Pause")) {
		fprintf(stderr, "trigger changed in the desktop not reported: '%s'\n", shortcut_info(f13).trigger);
		fail();
	}

	obs_nix_portal_shortcuts_stop(portal);
	signal_handler_disconnect(hotkey_test_obs->hotkeys.signals, "hotkey_background_status_changed", count_status,
				  NULL);
	fake_portal_stop();
}

/* Hyprland-like desktop (xdg-desktop-portal-hyprland: version 1, empty
 * trigger_description): shortcuts are bound but no trigger comes back; the
 * keys must be bound in the compositor's config. */
static void status_without_desktop_triggers(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);
	fake.no_triggers = true;

	struct hotkey_log talk = {0};
	register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "after BindShortcuts");

	uint32_t flags = background_flags();
	assert_false(flags & OBS_HOTKEYS_BACKGROUND_DESKTOP_TRIGGERS);
	assert_false(flags & OBS_HOTKEYS_BACKGROUND_CONFIGURABLE);
	struct shortcut_info info = shortcut_info(f13);
	assert_true(info.bound);
	assert_string_equal(info.trigger, "(none)");

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* The user dismissed the desktop's dialog: Settings shows it, and "Ask
 * again" binds the same set again without a new combination. */
static void declined_then_asked_again(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);
	fake.bind_response = 1;

	struct hotkey_log talk = {0};
	register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_state(OBS_HOTKEYS_BACKGROUND_DECLINED, "cancelled dialog");
	struct shortcut_info info = shortcut_info(f13);
	assert_true(info.found);
	assert_false(info.bound);
	expect_no_bind_after(1, 700, "declined set");

	fake.bind_response = 0;
	obs_hotkeys_retry_background();
	expect_bind(2);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "asked again");
	assert_true(shortcut_info(f13).bound);

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	assert_true(wait_until(log_pressed, &talk));
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", false);
	assert_true(wait_until(log_released, &talk));

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* A new key is added later and the user cancels the desktop's dialog for
 * it (GNOME lists every shortcut again in that dialog): the keys approved
 * before keep working, only the new one is not bound, and it is not asked
 * for again until Ask again. */
static void declined_new_key_keeps_approved_keys(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0}, record = {0};
	register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "approved");

	fake.decline_with_id = "ctrl+shift+OBS_KEY_A";
	register_bound("record", &record, ctrl_shift_a);
	expect_bind(2);
	/* The approved key is bound again, without the declined one. */
	if (!wait_until(binds_at_least, (void *)(uintptr_t)3)) {
		fprintf(stderr,
			"after the dialog for a new key was cancelled, the approved keys were "
			"not bound again (state %s)\n",
			state_name(obs_hotkeys_get_background_state(NULL)));
		fail();
	}
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "approved keys bound again");
	uint32_t flags = background_flags();
	assert_true(flags & OBS_HOTKEYS_BACKGROUND_PARTLY_DECLINED);
	assert_true(shortcut_info(f13).bound);
	assert_true(shortcut_info(ctrl_shift_a).found);
	assert_false(shortcut_info(ctrl_shift_a).bound);
	expect_no_bind_after(3, 700, "declined new key");

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	assert_true(wait_until(log_pressed, &talk));
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", false);
	assert_true(wait_until(log_released, &talk));

	/* Settings opens and closes (the keys are released meanwhile): the
	 * approved key is bound again, the declined one is not asked for. */
	obs_hotkey_enable_background_press(false);
	serve_for(300);
	obs_hotkey_enable_background_press(true);
	expect_bind(4);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "after Settings closed");
	if (bound_has("ctrl+shift+OBS_KEY_A")) {
		fprintf(stderr, "closing Settings asked again for the declined key\n");
		fail();
	}
	assert_true(bound_has("OBS_KEY_F13"));
	assert_true(background_flags() & OBS_HOTKEYS_BACKGROUND_PARTLY_DECLINED);
	assert_false(shortcut_info(ctrl_shift_a).bound);
	expect_no_bind_after(4, 700, "declined new key after Settings closed");

	fake.decline_with_id = NULL;
	obs_hotkeys_retry_background();
	expect_bind(5);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "asked again");
	assert_false(background_flags() & OBS_HOTKEYS_BACKGROUND_PARTLY_DECLINED);
	assert_true(shortcut_info(ctrl_shift_a).bound);

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* The desktop revoked the shortcuts: Settings shows it; asking again binds. */
static void closed_then_asked_again(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0};
	register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "bound");

	emit_session_closed(find_session(fake.last_bind_session));
	expect_state(OBS_HOTKEYS_BACKGROUND_CLOSED, "session closed by the desktop");
	assert_false(shortcut_info(f13).bound);

	obs_hotkeys_retry_background();
	expect_bind(2);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "asked again");

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* Settings disables background presses while it is open, which releases
 * the keys; the status keeps the last binding with the paused flag. "Test a
 * key" (the probe) binds them meanwhile and reports a press without running
 * any hotkey; ending the probe releases them again. */
static void paused_status_and_probe(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);
	signal_handler_connect(hotkey_test_obs->hotkeys.signals, "hotkey_background_probe", record_probe, NULL);
	os_atomic_set_long(&probes, 0);
	probed_id[0] = 0;

	struct hotkey_log talk = {0};
	register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "bound");
	struct fake_session *first = find_session(fake.last_bind_session);

	obs_hotkey_enable_background_press(false);
	assert_true(wait_until(session_is_closed, first));
	serve_for(100);
	assert_int_equal(obs_hotkeys_get_background_state(NULL), OBS_HOTKEYS_BACKGROUND_ACTIVE);
	assert_true(background_flags() & OBS_HOTKEYS_BACKGROUND_PAUSED);
	assert_true(shortcut_info(f13).found);

	obs_hotkeys_set_background_probe(true);
	expect_bind(2);
	serve_for(100);
	assert_false(background_flags() & OBS_HOTKEYS_BACKGROUND_PAUSED);
	struct fake_session *probe_session = find_session(fake.last_bind_session);

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	if (!wait_until(probed, (void *)(uintptr_t)1)) {
		fprintf(stderr, "a probed key press sent no hotkey_background_probe\n");
		fail();
	}
	assert_string_equal(probed_id, "OBS_KEY_F13");
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", false);
	serve_for(150);
	assert_int_equal(os_atomic_load_long(&talk.presses), 0);
	assert_false(obs_nix_hotkey_state_is_pressed(&key_state, OBS_KEY_F13));

	obs_hotkeys_set_background_probe(false);
	if (!wait_until(session_is_closed, probe_session)) {
		fprintf(stderr, "probe ended but the desktop still holds the keys\n");
		fail();
	}
	serve_for(100);
	assert_true(background_flags() & OBS_HOTKEYS_BACKGROUND_PAUSED);

	obs_hotkey_enable_background_press(true);
	expect_bind(3);
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	assert_true(wait_until(log_pressed, &talk));
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", false);
	assert_true(wait_until(log_released, &talk));
	assert_int_equal(os_atomic_load_long(&probes), 1);

	obs_nix_portal_shortcuts_stop(portal);
	signal_handler_disconnect(hotkey_test_obs->hotkeys.signals, "hotkey_background_probe", record_probe, NULL);
	fake_portal_stop();
}

/* KDE: the user accepted the dialog but left one shortcut without a key;
 * that one is not bound, so Settings must not call it working. */
static void untriggered_shortcut_is_not_bound(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);
	fake.untriggered_id = "ctrl+shift+OBS_KEY_A";

	struct hotkey_log talk = {0}, record = {0};
	register_bound("push-to-talk", &talk, f13);
	register_bound("record", &record, ctrl_shift_a);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "partly approved");
	assert_true(background_flags() & OBS_HOTKEYS_BACKGROUND_DESKTOP_TRIGGERS);
	assert_true(shortcut_info(f13).bound);
	if (shortcut_info(ctrl_shift_a).bound) {
		fprintf(stderr, "a shortcut the desktop gave no key is reported as bound\n");
		fail();
	}

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* KDE after its dialog was cancelled once: the shortcuts stay registered
 * without keys, so every later BindShortcuts returns at once, no dialog, all
 * triggers empty. A desktop with its own shortcut editor (portal version 2)
 * assigns the keys itself, so none of them is bound. */
static void configurable_portal_without_triggers_binds_nothing(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);
	fake.no_triggers = true;
	fake.version = 2;

	struct hotkey_log talk = {0}, record = {0};
	register_bound("push-to-talk", &talk, f13);
	register_bound("record", &record, ctrl_shift_a);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "after BindShortcuts");
	uint32_t flags = background_flags();
	assert_true(flags & OBS_HOTKEYS_BACKGROUND_CONFIGURABLE);
	if (!(flags & OBS_HOTKEYS_BACKGROUND_DESKTOP_TRIGGERS) || shortcut_info(f13).bound ||
	    shortcut_info(ctrl_shift_a).bound) {
		fprintf(stderr,
			"version 2 portal returned no keys, yet OBS counts the shortcuts as bound "
			"(desktop triggers flag %s)\n",
			flags & OBS_HOTKEYS_BACKGROUND_DESKTOP_TRIGGERS ? "set" : "not set");
		fail();
	}

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* The portal restarts while Settings is open (background presses off):
 * Settings shows that it went away, then that the keys are not bound yet;
 * closing Settings binds them. */
static void status_follows_portal_restart(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0};
	register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "bound");
	obs_hotkey_enable_background_press(false);

	fake_portal_own_name(false);
	expect_state(OBS_HOTKEYS_BACKGROUND_FAILED, "portal gone");
	assert_true(background_flags() & OBS_HOTKEYS_BACKGROUND_RESTARTING);
	assert_false(shortcut_info(f13).bound);

	fake_portal_own_name(true);
	expect_state(OBS_HOTKEYS_BACKGROUND_IDLE, "portal back, presses off");
	assert_false(background_flags() & OBS_HOTKEYS_BACKGROUND_RESTARTING);
	expect_no_bind_after(1, 500, "presses off after the restart");

	obs_hotkey_enable_background_press(true);
	expect_bind(2);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "presses on again");
	assert_true(shortcut_info(f13).bound);

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

static void expect_description(const char *id, const char *want)
{
	const char *trigger = NULL, *description = NULL;
	if (!bound_shortcut(id, &trigger, &description) || !description || strcmp(description, want) != 0) {
		fprintf(stderr, "%s described as '%s', expected '%s'\n", id, description ? description : "(none)",
			want);
		fail();
	}
}

/* The desktop's shortcut list names what each key does, not just the key;
 * an action moving to or from a key updates it (the set of keys is the
 * same, so only the description changed). */
static void descriptions_name_actions(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0}, mute = {0}, record = {0}, a = {0}, b = {0};
	register_bound("Push-to-talk", &talk, f13);
	register_bound("Push-to-mute", &mute, f13);
	register_bound("Start Recording", &record, ctrl_shift_a);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_description("OBS_KEY_F13", "OBS: Push-to-talk, Push-to-mute");
	expect_description("ctrl+shift+OBS_KEY_A", "OBS: Start Recording");

	obs_hotkey_id scene = register_bound("Switch to Scene 2", &a, f13);
	expect_bind(2);
	expect_description("OBS_KEY_F13", "OBS: Push-to-talk, Push-to-mute, Switch to Scene 2");

	/* At most three actions by name. */
	register_bound("Screenshot", &b, f13);
	expect_bind(3);
	expect_description("OBS_KEY_F13", "OBS: Push-to-talk, Push-to-mute, Switch to Scene 2 and 1 more");

	obs_hotkey_unregister(scene);
	expect_bind(4);
	expect_description("OBS_KEY_F13", "OBS: Push-to-talk, Push-to-mute, Screenshot");
	assert_int_equal(g_variant_n_children(fake.last_shortcuts), 2);

	/* Nothing changed, nothing bound again. */
	expect_no_bind_after(4, 800, "unchanged descriptions");

	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", true);
	assert_true(wait_until(log_pressed, &talk));
	emit_shortcut(fake.last_bind_session, "OBS_KEY_F13", false);
	assert_true(wait_until(log_released, &talk));

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* A set the user declined is not asked for again only because an action
 * moved onto one of its keys. */
static void declined_not_reasked_for_description(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);
	fake.bind_response = 1;

	struct hotkey_log talk = {0}, mute = {0};
	register_bound("Push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_state(OBS_HOTKEYS_BACKGROUND_DECLINED, "cancelled dialog");

	register_bound("Push-to-mute", &mute, f13);
	expect_no_bind_after(1, 1000, "declined set with a new description");

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

static bool configured(void *data)
{
	return fake.num_configures >= (size_t)(uintptr_t)data;
}

/* "Change it in desktop settings": with portal version 2, Settings (where
 * presses are off) probes to bind the shortcuts and asks the desktop to open
 * its editor for OBS's session. */
static void configure_opens_desktop_editor(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);
	fake.version = 2;

	struct hotkey_log talk = {0};
	register_bound("Push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	assert_true(background_flags() & OBS_HOTKEYS_BACKGROUND_CONFIGURABLE);

	obs_hotkey_enable_background_press(false);
	assert_true(wait_until(session_is_closed, find_session(fake.last_bind_session)));

	obs_hotkeys_set_background_probe(true);
	obs_hotkeys_configure_background();
	if (!wait_until(configured, (void *)(uintptr_t)1)) {
		fprintf(stderr, "ConfigureShortcuts was not called\n");
		fail();
	}
	assert_int_equal(fake.num_binds, 2);
	assert_string_equal(fake.configure_session, fake.last_bind_session);
	assert_false(find_session(fake.last_bind_session)->closed);

	/* The user picks another key in the desktop's editor. */
	emit_shortcuts_changed(fake.last_bind_session, "OBS_KEY_F13", "Pause");
	if (!wait_until(trigger_is, "Pause")) {
		fprintf(stderr, "key changed in the desktop editor not reported\n");
		fail();
	}

	obs_hotkeys_set_background_probe(false);
	obs_hotkey_enable_background_press(true);
	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

static bool configure_failed(void *data)
{
	UNUSED_PARAMETER(data);
	return (background_flags() & OBS_HOTKEYS_BACKGROUND_CONFIGURE_FAILED) != 0;
}

/* The desktop refuses to open its editor: Settings is told, so it does not
 * keep waiting for Done; the next request starts clean. */
static void configure_failure_is_reported(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);
	fake.version = 2;
	fake.configure_error = "Not supported by this desktop";

	struct hotkey_log talk = {0};
	register_bound("Push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	obs_hotkeys_configure_background();
	if (!wait_until(configure_failed, NULL)) {
		fprintf(stderr, "a failed ConfigureShortcuts was not reported\n");
		fail();
	}
	assert_int_equal(fake.num_configures, 1);

	fake.configure_error = NULL;
	obs_hotkeys_configure_background();
	assert_false(background_flags() & OBS_HOTKEYS_BACKGROUND_CONFIGURE_FAILED);
	assert_true(wait_until(configured, (void *)(uintptr_t)2));
	serve_for(200);
	assert_false(background_flags() & OBS_HOTKEYS_BACKGROUND_CONFIGURE_FAILED);

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* Portal version 1 has no ConfigureShortcuts: nothing is called. */
static void configure_needs_version_2(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0};
	register_bound("Push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	assert_false(background_flags() & OBS_HOTKEYS_BACKGROUND_CONFIGURABLE);
	obs_hotkeys_configure_background();
	serve_for(500);
	assert_int_equal(fake.num_configures, 0);
	/* Settings says so instead of waiting for an editor. */
	assert_true(background_flags() & OBS_HOTKEYS_BACKGROUND_CONFIGURE_FAILED);

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* No GlobalShortcuts portal: Settings says hotkeys only work while focused,
 * and every combination is focus-only. */
static void status_without_portal(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(false);

	struct hotkey_log talk = {0};
	register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_state(OBS_HOTKEYS_BACKGROUND_UNAVAILABLE, "no portal");
	char *error = obs_hotkeys_get_background_error();
	assert_non_null(error);
	bfree(error);

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* The portal refuses the session (no app id: OBS run from a build directory
 * without its desktop file): Settings shows the portal's reason. */
static void status_reports_portal_error(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);
	fake.create_error = "An app id is required";

	struct hotkey_log talk = {0};
	register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_state(OBS_HOTKEYS_BACKGROUND_FAILED, "CreateSession refused");
	char *error = obs_hotkeys_get_background_error();
	assert_non_null(error);
	if (!strstr(error, "An app id is required")) {
		fprintf(stderr, "error '%s' does not carry the portal's reason\n", error);
		fail();
	}
	bfree(error);

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* Only a mouse button is bound: nothing to hand to the desktop. */
static void status_idle_with_mouse_only(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log mouse = {0};
	register_bound("mouse", &mouse, (obs_key_combination_t){0, OBS_KEY_MOUSE4});

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_state(OBS_HOTKEYS_BACKGROUND_IDLE, "mouse binding only");
	expect_no_bind_after(0, 700, "mouse binding only");
	assert_int_equal(shortcut_info(f13).count, 0);

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* ------------------------------------------------------------------------ */
/* When a sync binds again (the combination set logic)                       */

/* Shortcut id of entry i of the last BindShortcuts call. */
static char *bound_id_at(size_t i)
{
	if (i >= g_variant_n_children(fake.last_shortcuts))
		return g_strdup("(none)");
	char *id = NULL;
	g_variant_get_child(fake.last_shortcuts, i, "(s@a{sv})", &id, NULL);
	return id;
}

static void expect_bound_id_at(size_t i, const char *want)
{
	char *id = bound_id_at(i);
	if (strcmp(id, want) != 0) {
		fprintf(stderr, "shortcut %zu of the last bind is '%s', expected '%s'\n", i, id, want);
		g_free(id);
		fail();
	}
	g_free(id);
}

/* The same keys with the same descriptions in another order are the same
 * set: nothing is bound again. */
static void reordered_set_is_not_bound_again(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0}, record = {0}, shot = {0};
	obs_hotkey_id talk_id = register_bound("Push-to-talk", &talk, f13);
	register_bound("Start Recording", &record, ctrl_shift_a);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	assert_int_equal(g_variant_n_children(fake.last_shortcuts), 2);
	expect_bound_id_at(0, "OBS_KEY_F13");
	expect_bound_id_at(1, "ctrl+shift+OBS_KEY_A");

	/* Push-to-talk again: its binding now comes after Start Recording. */
	obs_hotkey_unregister(talk_id);
	register_bound("Push-to-talk", &talk, f13);
	expect_no_bind_after(1, 1000, "the same set in another order");

	/* A new key binds the set in its new order. */
	register_bound("Screenshot", &shot, f14);
	expect_bind(2);
	assert_int_equal(g_variant_n_children(fake.last_shortcuts), 3);
	expect_bound_id_at(0, "ctrl+shift+OBS_KEY_A");
	expect_bound_id_at(1, "OBS_KEY_F13");
	expect_bound_id_at(2, "OBS_KEY_F14");
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "reordered set with a new key");

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* Descriptions are compared byte by byte: the same name in another Unicode
 * normalization (U+00E9, then e and U+0301 combining acute) binds again; the
 * same bytes do not. */
static void descriptions_compared_as_bytes(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0};
	obs_hotkey_id id = register_bound("Mic \xc3\xa9", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_description("OBS_KEY_F13", "OBS: Mic \xc3\xa9");

	obs_hotkey_unregister(id);
	id = register_bound("Mic e\xcc\x81", &talk, f13);
	expect_bind(2);
	expect_description("OBS_KEY_F13", "OBS: Mic e\xcc\x81");

	obs_hotkey_unregister(id);
	register_bound("Mic e\xcc\x81", &talk, f13);
	expect_no_bind_after(2, 1000, "the same description bytes");

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* The dialog for a set with no new key (an action moved onto an approved
 * key, so only its description changed) is cancelled. Characterized, not
 * endorsed: the approved key is not bound again, because bind_approved()
 * binds only a strict part of the request, so every key stops until Ask
 * again. */
static void declined_description_change_unbinds_approved_key(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0}, mute = {0};
	register_bound("Push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "approved");

	fake.bind_response = 1;
	register_bound("Push-to-mute", &mute, f13);
	expect_bind(2);
	expect_description("OBS_KEY_F13", "OBS: Push-to-talk, Push-to-mute");
	expect_state(OBS_HOTKEYS_BACKGROUND_DECLINED, "dialog for a new description cancelled");
	expect_no_bind_after(2, 800, "declined description change");
	assert_false(background_flags() & OBS_HOTKEYS_BACKGROUND_PARTLY_DECLINED);
	assert_true(shortcut_info(f13).found);
	assert_false(shortcut_info(f13).bound);

	fake.bind_response = 0;
	obs_hotkeys_retry_background();
	expect_bind(3);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "asked again");
	assert_true(shortcut_info(f13).bound);

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

/* Partly declined (one key approved, a new one declined), then the approved
 * key's binding goes away: only declined keys are left, so nothing is
 * asked for and the state is declined. */
static void only_declined_keys_left_binds_nothing(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_DBUS_DAEMON();
	fake_portal_start(true);

	struct hotkey_log talk = {0}, record = {0};
	obs_hotkey_id talk_id = register_bound("push-to-talk", &talk, f13);

	struct obs_nix_portal_shortcuts *portal = obs_nix_portal_shortcuts_start(&key_state, test_key_name);
	expect_bind(1);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "approved");

	fake.decline_with_id = "ctrl+shift+OBS_KEY_A";
	register_bound("record", &record, ctrl_shift_a);
	expect_bind(3);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "approved key bound again");
	assert_true(background_flags() & OBS_HOTKEYS_BACKGROUND_PARTLY_DECLINED);

	struct fake_session *session = find_session(fake.last_bind_session);
	assert_non_null(session);
	obs_hotkey_unregister(talk_id);
	if (!wait_until(session_is_closed, session)) {
		fprintf(stderr, "session not closed after the approved key's binding went away\n");
		fail();
	}
	expect_state(OBS_HOTKEYS_BACKGROUND_DECLINED, "only declined keys left");
	expect_no_bind_after(3, 800, "only declined keys left");
	assert_false(background_flags() & OBS_HOTKEYS_BACKGROUND_PARTLY_DECLINED);
	assert_false(shortcut_info(ctrl_shift_a).bound);

	fake.decline_with_id = NULL;
	obs_hotkeys_retry_background();
	expect_bind(4);
	expect_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, "asked again");
	assert_int_equal(g_variant_n_children(fake.last_shortcuts), 1);
	assert_true(bound_has("ctrl+shift+OBS_KEY_A"));

	obs_nix_portal_shortcuts_stop(portal);
	fake_portal_stop();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(shortcut_ids_round_trip),
		cmocka_unit_test_setup_teardown(portal_drives_push_to_talk, setup, teardown),
		cmocka_unit_test_setup_teardown(rebinds_when_bindings_change, setup, teardown),
		cmocka_unit_test_setup_teardown(burst_of_changes_binds_once, setup, teardown),
		cmocka_unit_test_setup_teardown(closed_session_is_not_rebound, setup, teardown),
		cmocka_unit_test_setup_teardown(rebinds_after_portal_restart, setup, teardown),
		cmocka_unit_test_setup_teardown(no_portal_falls_back, setup, teardown),
		cmocka_unit_test_setup_teardown(disabled_background_press_closes_session, setup, teardown),
		cmocka_unit_test_setup_teardown(disabled_at_start_binds_once_enabled, setup, teardown),
		cmocka_unit_test_setup_teardown(declined_set_stays_declined_across_disable, setup, teardown),
		cmocka_unit_test_setup_teardown(native_without_portal_client, setup, teardown),
		cmocka_unit_test_setup_teardown(status_reports_bound_shortcuts, setup, teardown),
		cmocka_unit_test_setup_teardown(status_without_desktop_triggers, setup, teardown),
		cmocka_unit_test_setup_teardown(declined_then_asked_again, setup, teardown),
		cmocka_unit_test_setup_teardown(declined_new_key_keeps_approved_keys, setup, teardown),
		cmocka_unit_test_setup_teardown(closed_then_asked_again, setup, teardown),
		cmocka_unit_test_setup_teardown(paused_status_and_probe, setup, teardown),
		cmocka_unit_test_setup_teardown(status_without_portal, setup, teardown),
		cmocka_unit_test_setup_teardown(status_reports_portal_error, setup, teardown),
		cmocka_unit_test_setup_teardown(status_idle_with_mouse_only, setup, teardown),
		cmocka_unit_test_setup_teardown(untriggered_shortcut_is_not_bound, setup, teardown),
		cmocka_unit_test_setup_teardown(configurable_portal_without_triggers_binds_nothing, setup, teardown),
		cmocka_unit_test_setup_teardown(status_follows_portal_restart, setup, teardown),
		cmocka_unit_test_setup_teardown(descriptions_name_actions, setup, teardown),
		cmocka_unit_test_setup_teardown(declined_not_reasked_for_description, setup, teardown),
		cmocka_unit_test_setup_teardown(configure_opens_desktop_editor, setup, teardown),
		cmocka_unit_test_setup_teardown(configure_needs_version_2, setup, teardown),
		cmocka_unit_test_setup_teardown(configure_failure_is_reported, setup, teardown),
		cmocka_unit_test_setup_teardown(reordered_set_is_not_bound_again, setup, teardown),
		cmocka_unit_test_setup_teardown(descriptions_compared_as_bytes, setup, teardown),
		cmocka_unit_test_setup_teardown(declined_description_change_unbinds_approved_key, setup, teardown),
		cmocka_unit_test_setup_teardown(only_declined_keys_left_binds_nothing, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
