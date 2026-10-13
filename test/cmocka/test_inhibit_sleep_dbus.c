/* os_inhibit_sleep_* on Linux and FreeBSD must ask the desktop to stop idle
 * sleep and locking: first through org.freedesktop.portal.Inhibit, then,
 * when there is no portal, through org.freedesktop.ScreenSaver (or the other
 * session D-Bus inhibitors in util/platform-nix-dbus.c). The frontend holds
 * one inhibitor for as long as an output is active.
 *
 * Both fakes run on a private D-Bus session bus (GTestDBus starts its own
 * dbus-daemon; the user's session bus is never touched), served from their
 * own thread so the synchronous calls libobs makes cannot deadlock. No
 * display is needed. Without dbus-daemon every case is skipped. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <inttypes.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#include <gio/gio.h>

#include <util/c99defs.h>
#include <util/platform.h>

#define PORTAL_NAME "org.freedesktop.portal.Desktop"
#define PORTAL_PATH "/org/freedesktop/portal/desktop"
#define INHIBIT_IFACE "org.freedesktop.portal.Inhibit"
#define REQUEST_IFACE "org.freedesktop.portal.Request"
#define SCREENSAVER_NAME "org.freedesktop.ScreenSaver"
#define SCREENSAVER_PATH "/ScreenSaver"
#define SCREENSAVER_COOKIE 4242u

#define REASON "OBS Video/audio"

static const char fake_xml[] = "<node>"
			       "  <interface name='org.freedesktop.portal.Inhibit'>"
			       "    <method name='Inhibit'>"
			       "      <arg type='s' name='window' direction='in'/>"
			       "      <arg type='u' name='flags' direction='in'/>"
			       "      <arg type='a{sv}' name='options' direction='in'/>"
			       "      <arg type='o' name='handle' direction='out'/>"
			       "    </method>"
			       "    <property name='version' type='u' access='read'/>"
			       "  </interface>"
			       "  <interface name='org.freedesktop.portal.Request'>"
			       "    <method name='Close'/>"
			       "  </interface>"
			       "  <interface name='org.freedesktop.ScreenSaver'>"
			       "    <method name='Inhibit'>"
			       "      <arg type='s' name='application_name' direction='in'/>"
			       "      <arg type='s' name='reason_for_inhibit' direction='in'/>"
			       "      <arg type='u' name='cookie' direction='out'/>"
			       "    </method>"
			       "    <method name='UnInhibit'>"
			       "      <arg type='u' name='cookie' direction='in'/>"
			       "    </method>"
			       "  </interface>"
			       "</node>";

static struct fake_desktop {
	GTestDBus *bus;
	GThread *thread;
	GMainContext *context;
	GMainLoop *loop;
	GDBusConnection *conn;
	GDBusNodeInfo *info;
	guint portal_registration;
	guint screensaver_registration;
	guint request_registration;

	GMutex lock;
	GCond ready_cond;
	bool ready;

	/* org.freedesktop.portal.Inhibit */
	int portal_inhibits;
	uint32_t portal_flags;
	char *portal_reason;
	char *request_path;
	int request_closes;
	char *closed_path;

	/* org.freedesktop.ScreenSaver */
	int screensaver_inhibits;
	char *screensaver_app;
	char *screensaver_reason;
	int screensaver_uninhibits;
	uint32_t uninhibit_cookie;
} fake;

static bool have_bus;

static void request_method_call(GDBusConnection *conn, const char *sender, const char *path, const char *iface,
				const char *method, GVariant *params, GDBusMethodInvocation *invocation, gpointer data)
{
	UNUSED_PARAMETER(conn);
	UNUSED_PARAMETER(sender);
	UNUSED_PARAMETER(iface);
	UNUSED_PARAMETER(params);
	UNUSED_PARAMETER(data);

	if (strcmp(method, "Close") == 0) {
		g_mutex_lock(&fake.lock);
		fake.request_closes++;
		g_free(fake.closed_path);
		fake.closed_path = g_strdup(path);
		g_mutex_unlock(&fake.lock);
	}
	g_dbus_method_invocation_return_value(invocation, NULL);
}

static const GDBusInterfaceVTable request_vtable = {.method_call = request_method_call};

static void portal_method_call(GDBusConnection *conn, const char *sender, const char *path, const char *iface,
			       const char *method, GVariant *params, GDBusMethodInvocation *invocation, gpointer data)
{
	UNUSED_PARAMETER(path);
	UNUSED_PARAMETER(iface);
	UNUSED_PARAMETER(data);

	if (strcmp(method, "Inhibit") != 0) {
		g_dbus_method_invocation_return_dbus_error(invocation, "org.freedesktop.DBus.Error.UnknownMethod",
							   method);
		return;
	}

	const char *window = NULL;
	uint32_t flags = 0;
	g_autoptr(GVariant) options = NULL;
	g_variant_get(params, "(&su@a{sv})", &window, &flags, &options);

	const char *token = NULL;
	const char *reason = NULL;
	g_variant_lookup(options, "handle_token", "&s", &token);
	g_variant_lookup(options, "reason", "&s", &reason);

	/* The request path the portal spec defines: the caller's unique
	 * name without ':' and with '.' as '_', then the token. */
	g_autofree char *sender_part = g_strdup(sender + 1);
	g_strdelimit(sender_part, ".", '_');
	g_autofree char *request_path = g_strdup_printf(PORTAL_PATH "/request/%s/%s", sender_part, token ? token : "t");

	if (fake.request_registration)
		g_dbus_connection_unregister_object(conn, fake.request_registration);
	fake.request_registration = g_dbus_connection_register_object(
		conn, request_path, g_dbus_node_info_lookup_interface(fake.info, REQUEST_IFACE), &request_vtable, NULL,
		NULL, NULL);

	g_mutex_lock(&fake.lock);
	fake.portal_inhibits++;
	fake.portal_flags = flags;
	g_free(fake.portal_reason);
	fake.portal_reason = g_strdup(reason ? reason : "");
	g_free(fake.request_path);
	fake.request_path = g_strdup(request_path);
	g_mutex_unlock(&fake.lock);

	g_dbus_method_invocation_return_value(invocation, g_variant_new("(o)", request_path));
}

static GVariant *portal_get_property(GDBusConnection *conn, const char *sender, const char *path, const char *iface,
				     const char *property, GError **error, gpointer data)
{
	UNUSED_PARAMETER(conn);
	UNUSED_PARAMETER(sender);
	UNUSED_PARAMETER(path);
	UNUSED_PARAMETER(iface);
	UNUSED_PARAMETER(data);

	if (strcmp(property, "version") == 0)
		return g_variant_new_uint32(3);
	g_set_error(error, G_DBUS_ERROR, G_DBUS_ERROR_UNKNOWN_PROPERTY, "%s", property);
	return NULL;
}

static const GDBusInterfaceVTable portal_vtable = {
	.method_call = portal_method_call,
	.get_property = portal_get_property,
};

static void screensaver_method_call(GDBusConnection *conn, const char *sender, const char *path, const char *iface,
				    const char *method, GVariant *params, GDBusMethodInvocation *invocation,
				    gpointer data)
{
	UNUSED_PARAMETER(conn);
	UNUSED_PARAMETER(sender);
	UNUSED_PARAMETER(path);
	UNUSED_PARAMETER(iface);
	UNUSED_PARAMETER(data);

	if (strcmp(method, "Inhibit") == 0) {
		const char *app = NULL;
		const char *reason = NULL;
		g_variant_get(params, "(&s&s)", &app, &reason);

		g_mutex_lock(&fake.lock);
		fake.screensaver_inhibits++;
		g_free(fake.screensaver_app);
		fake.screensaver_app = g_strdup(app);
		g_free(fake.screensaver_reason);
		fake.screensaver_reason = g_strdup(reason);
		g_mutex_unlock(&fake.lock);

		g_dbus_method_invocation_return_value(invocation, g_variant_new("(u)", SCREENSAVER_COOKIE));
		return;
	}

	if (strcmp(method, "UnInhibit") == 0) {
		uint32_t cookie = 0;
		g_variant_get(params, "(u)", &cookie);

		g_mutex_lock(&fake.lock);
		fake.screensaver_uninhibits++;
		fake.uninhibit_cookie = cookie;
		g_mutex_unlock(&fake.lock);

		g_dbus_method_invocation_return_value(invocation, NULL);
		return;
	}

	g_dbus_method_invocation_return_dbus_error(invocation, "org.freedesktop.DBus.Error.UnknownMethod", method);
}

static const GDBusInterfaceVTable screensaver_vtable = {.method_call = screensaver_method_call};

/* Serves both fake objects from their own main context, so a synchronous
 * call from the test thread (libobs's D-Bus code) is answered. */
static gpointer fake_thread(gpointer data)
{
	UNUSED_PARAMETER(data);
	g_main_context_push_thread_default(fake.context);

	g_autoptr(GError) error = NULL;
	fake.conn = g_dbus_connection_new_for_address_sync(g_test_dbus_get_bus_address(fake.bus),
							   G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT |
								   G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION,
							   NULL, NULL, &error);
	if (fake.conn) {
		fake.portal_registration = g_dbus_connection_register_object(
			fake.conn, PORTAL_PATH, g_dbus_node_info_lookup_interface(fake.info, INHIBIT_IFACE),
			&portal_vtable, NULL, NULL, NULL);
		fake.screensaver_registration = g_dbus_connection_register_object(
			fake.conn, SCREENSAVER_PATH, g_dbus_node_info_lookup_interface(fake.info, SCREENSAVER_NAME),
			&screensaver_vtable, NULL, NULL, NULL);
	} else {
		fprintf(stderr, "fake desktop: %s\n", error ? error->message : "no connection");
	}

	g_mutex_lock(&fake.lock);
	fake.ready = true;
	g_cond_signal(&fake.ready_cond);
	g_mutex_unlock(&fake.lock);

	g_main_loop_run(fake.loop);

	if (fake.conn) {
		if (fake.request_registration)
			g_dbus_connection_unregister_object(fake.conn, fake.request_registration);
		g_dbus_connection_unregister_object(fake.conn, fake.portal_registration);
		g_dbus_connection_unregister_object(fake.conn, fake.screensaver_registration);
		g_dbus_connection_close_sync(fake.conn, NULL, NULL);
		g_clear_object(&fake.conn);
	}

	g_main_context_pop_thread_default(fake.context);
	return NULL;
}

/* Takes or drops a well-known name, as a desktop service starting or
 * exiting does. */
static void fake_own_name(const char *name, bool own)
{
	g_autoptr(GError) error = NULL;
	g_autoptr(GVariant) reply = g_dbus_connection_call_sync(
		fake.conn, "org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
		own ? "RequestName" : "ReleaseName",
		own ? g_variant_new("(su)", name, 4 /* DO_NOT_QUEUE */) : g_variant_new("(s)", name),
		G_VARIANT_TYPE("(u)"), G_DBUS_CALL_FLAGS_NONE, -1, NULL, &error);
	if (!reply) {
		fprintf(stderr, "%s %s: %s\n", own ? "RequestName" : "ReleaseName", name, error->message);
		fail();
	}
	uint32_t result = 0;
	g_variant_get(reply, "(u)", &result);
	if (result != 1 /* PRIMARY_OWNER or RELEASED */) {
		fprintf(stderr, "%s %s returned %u\n", own ? "RequestName" : "ReleaseName", name, result);
		fail();
	}
}

/* Runs the test thread's default main context (OBS's Qt loop does this in
 * the app), so the portal client's async replies are handled. */
static void pump_for(int ms)
{
	int64_t end = g_get_monotonic_time() + (int64_t)ms * 1000;
	while (g_get_monotonic_time() < end) {
		while (g_main_context_iteration(NULL, FALSE))
			;
		g_usleep(5000);
	}
}

static bool wait_for_count(const int *counter, int want, int ms)
{
	int64_t end = g_get_monotonic_time() + (int64_t)ms * 1000;
	for (;;) {
		g_mutex_lock(&fake.lock);
		bool done = *counter >= want;
		g_mutex_unlock(&fake.lock);
		if (done)
			return true;
		if (g_get_monotonic_time() >= end)
			return false;
		while (g_main_context_iteration(NULL, FALSE))
			;
		g_usleep(5000);
	}
}

static bool dbus_daemon_available(void)
{
	char *path = g_find_program_in_path("dbus-daemon");
	bool found = path != NULL;
	g_free(path);
	return found;
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	/* In a Flatpak only the portal is used, so the ScreenSaver case would
	 * not apply; never the case in a build tree. */
	if (!dbus_daemon_available() || access("/.flatpak-info", F_OK) == 0)
		return 0;

	memset(&fake, 0, sizeof(fake));
	g_mutex_init(&fake.lock);
	g_cond_init(&fake.ready_cond);

	/* Sets DBUS_SESSION_BUS_ADDRESS, so libobs's g_bus_get_sync()
	 * connects to this bus. */
	fake.bus = g_test_dbus_new(G_TEST_DBUS_NONE);
	g_test_dbus_up(fake.bus);

	g_autoptr(GError) error = NULL;
	fake.info = g_dbus_node_info_new_for_xml(fake_xml, &error);
	assert_non_null(fake.info);

	fake.context = g_main_context_new();
	fake.loop = g_main_loop_new(fake.context, FALSE);
	fake.thread = g_thread_new("fake-desktop", fake_thread, NULL);

	g_mutex_lock(&fake.lock);
	while (!fake.ready)
		g_cond_wait(&fake.ready_cond, &fake.lock);
	g_mutex_unlock(&fake.lock);

	assert_non_null(fake.conn);
	assert_true(fake.portal_registration > 0);
	assert_true(fake.screensaver_registration > 0);
	have_bus = true;
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (!have_bus)
		return 0;

	g_main_loop_quit(fake.loop);
	g_thread_join(fake.thread);
	g_main_loop_unref(fake.loop);
	g_main_context_unref(fake.context);
	g_dbus_node_info_unref(fake.info);

	/* libobs's session bus singleton may still be alive; stop does not
	 * wait for it (g_test_dbus_down would). */
	g_test_dbus_stop(fake.bus);
	g_clear_object(&fake.bus);

	g_free(fake.portal_reason);
	g_free(fake.request_path);
	g_free(fake.closed_path);
	g_free(fake.screensaver_app);
	g_free(fake.screensaver_reason);
	g_cond_clear(&fake.ready_cond);
	g_mutex_clear(&fake.lock);
	have_bus = false;
	return 0;
}

#define REQUIRE_BUS()                                                                         \
	do {                                                                                  \
		if (!have_bus) {                                                              \
			fprintf(stderr, "dbus-daemon not found (or in a Flatpak); skipping\n"); \
			skip();                                                               \
		}                                                                             \
	} while (false)

/* A desktop with xdg-desktop-portal (GNOME, KDE, Hyprland, wlroots): the
 * inhibitor goes through org.freedesktop.portal.Inhibit with the caller's
 * reason, and deactivating closes that request. */
static void test_portal_inhibit_and_release(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_BUS();

	fake_own_name(PORTAL_NAME, true);

	os_inhibit_t *inhibit = os_inhibit_sleep_create(REASON);
	assert_non_null(inhibit);
	assert_true(os_inhibit_sleep_set_active(inhibit, true));

	if (!wait_for_count(&fake.portal_inhibits, 1, 3000)) {
		fprintf(stderr, "set_active(true) made no org.freedesktop.portal.Inhibit.Inhibit call\n");
		os_inhibit_sleep_destroy(inhibit);
		fake_own_name(PORTAL_NAME, false);
		fail();
	}

	g_mutex_lock(&fake.lock);
	assert_int_equal(fake.portal_inhibits, 1);
	assert_string_equal(fake.portal_reason, REASON);
	/* suspend | idle */
	assert_int_equal(fake.portal_flags, 0xC);
	g_autofree char *request_path = g_strdup(fake.request_path);
	g_mutex_unlock(&fake.lock);

	/* Let the client see the Inhibit reply, as the app's main loop does,
	 * so deactivating closes the request rather than cancelling a call
	 * that is still in flight. */
	pump_for(300);

	assert_true(os_inhibit_sleep_set_active(inhibit, false));
	if (!wait_for_count(&fake.request_closes, 1, 3000)) {
		fprintf(stderr, "set_active(false) did not close the portal request\n");
		os_inhibit_sleep_destroy(inhibit);
		fake_own_name(PORTAL_NAME, false);
		fail();
	}

	g_mutex_lock(&fake.lock);
	assert_int_equal(fake.request_closes, 1);
	assert_string_equal(fake.closed_path, request_path);
	g_mutex_unlock(&fake.lock);

	os_inhibit_sleep_destroy(inhibit);
	pump_for(50);

	g_mutex_lock(&fake.lock);
	assert_int_equal(fake.portal_inhibits, 1);
	assert_int_equal(fake.screensaver_inhibits, 0);
	g_mutex_unlock(&fake.lock);

	fake_own_name(PORTAL_NAME, false);
}

/* Destroy before the portal answers Inhibit. Its cancelled async callback
 * still runs when the main context is pumped. */
static void test_portal_destroy_while_inhibit_pending(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_BUS();

	fake_own_name(PORTAL_NAME, true);
	os_inhibit_t *inhibit = os_inhibit_sleep_create(REASON);
	assert_non_null(inhibit);
	assert_true(os_inhibit_sleep_set_active(inhibit, true));
	os_inhibit_sleep_destroy(inhibit);
	pump_for(300);
	fake_own_name(PORTAL_NAME, false);
}

/* No portal, but a session screensaver service (KDE, older GNOME, Xfce):
 * libobs falls back to org.freedesktop.ScreenSaver.Inhibit and releases
 * the same cookie. */
static void test_screensaver_inhibit_and_release(void **state)
{
	UNUSED_PARAMETER(state);
	REQUIRE_BUS();

	fake_own_name(SCREENSAVER_NAME, true);

	os_inhibit_t *inhibit = os_inhibit_sleep_create(REASON);
	assert_non_null(inhibit);
	assert_true(os_inhibit_sleep_set_active(inhibit, true));

	if (!wait_for_count(&fake.screensaver_inhibits, 1, 3000)) {
		fprintf(stderr, "set_active(true) made no org.freedesktop.ScreenSaver.Inhibit call\n");
		os_inhibit_sleep_destroy(inhibit);
		fake_own_name(SCREENSAVER_NAME, false);
		fail();
	}

	g_mutex_lock(&fake.lock);
	assert_int_equal(fake.screensaver_inhibits, 1);
	assert_string_equal(fake.screensaver_app, "libobs");
	assert_string_equal(fake.screensaver_reason, REASON);
	g_mutex_unlock(&fake.lock);

	assert_true(os_inhibit_sleep_set_active(inhibit, false));
	if (!wait_for_count(&fake.screensaver_uninhibits, 1, 3000)) {
		fprintf(stderr, "set_active(false) made no org.freedesktop.ScreenSaver.UnInhibit call\n");
		os_inhibit_sleep_destroy(inhibit);
		fake_own_name(SCREENSAVER_NAME, false);
		fail();
	}

	g_mutex_lock(&fake.lock);
	assert_int_equal(fake.screensaver_uninhibits, 1);
	assert_int_equal(fake.uninhibit_cookie, SCREENSAVER_COOKIE);
	g_mutex_unlock(&fake.lock);

	os_inhibit_sleep_destroy(inhibit);

	g_mutex_lock(&fake.lock);
	assert_int_equal(fake.screensaver_inhibits, 1);
	assert_int_equal(fake.screensaver_uninhibits, 1);
	g_mutex_unlock(&fake.lock);

	fake_own_name(SCREENSAVER_NAME, false);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_portal_inhibit_and_release),
		cmocka_unit_test(test_portal_destroy_while_inhibit_pending),
		cmocka_unit_test(test_screensaver_inhibit_and_release),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
