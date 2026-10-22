/*
This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 2 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/

/* The session and BindShortcuts handling follows the GlobalShortcuts portal
 * client by Adam Fallon (obsproject/obs-studio#13680); feeding the hotkey
 * thread one shortcut per key combination follows the xx-hotkey-v1 client by
 * Aurelle (obsproject/obs-studio#13661). */

#include "libobs/build/libobs-nix-portal-shortcuts.pch.h"

#define PORTAL_NAME "org.freedesktop.portal.Desktop"
#define PORTAL_PATH "/org/freedesktop/portal/desktop"
#define SHORTCUTS_IFACE "org.freedesktop.portal.GlobalShortcuts"
#define REQUEST_IFACE "org.freedesktop.portal.Request"
#define SESSION_IFACE "org.freedesktop.portal.Session"
#define REGISTRY_IFACE "org.freedesktop.host.portal.Registry"
#define APP_ID "com.obsproject.Studio"

/* How often the bindings are compared with what the portal has bound. A
 * change is bound once it has been stable for one interval, so loading a
 * profile does not rebind for every source. */
/* A binding change is bound once the bindings have not changed for this
 * long, so loading a profile or scene collection binds once, not once per
 * source. */
#define SYNC_DELAY_MS 500
/* While a tapped shortcut waits for the hotkey thread (25 ms cycle). */
#define FLUSH_INTERVAL_MS 25

typedef DARRAY(obs_key_combination_t) combo_array_t;
/* Shortcut descriptions, parallel to a combo_array_t. */
typedef DARRAY(char *) description_array_t;

/* At most this many actions are named in a shortcut's description. */
#define MAX_DESCRIBED_ACTIONS 3

struct obs_nix_portal_shortcuts {
	struct obs_nix_hotkey_state *state;
	obs_nix_portal_key_name_func key_name;

	pthread_t thread;
	GMainContext *context;
	volatile bool stopping;
	/* Armed by the hotkey signals from any thread; when it fires, the
	 * portal thread brings the session in line with the bindings. */
	GSource *sync_source;
	bool sync_due;

	GDBusConnection *conn;
	char *sender;
	unsigned int token;
	char *session_path;
	guint activated_id;
	guint deactivated_id;
	guint changed_id;
	guint closed_id;
	guint watch_id;
	/* The portal left the bus; a new instance is bound from scratch. */
	bool portal_gone;
	GSource *flush_source;

	/* With a session: the combinations bound in it, and the description
	 * each was bound with. */
	combo_array_t bound;
	description_array_t bound_descriptions;
	/* Without one: combinations the user cancelled, the portal refused or
	 * the desktop closed. They are not bound again (no repeated prompt)
	 * until the bindings gain a combination or the portal restarts. */
	combo_array_t declined;
	/* The combinations the desktop last approved in full. When its dialog
	 * for a set with new ones is declined, these are bound again. */
	combo_array_t approved;
	/* The session binds only the approved part of its set. */
	bool partly_declined;
	/* GlobalShortcuts interface version; 2 adds ConfigureShortcuts. */
	uint32_t version;

	/* Background presses are disabled (obs_hotkey_enable_background_press):
	 * no session is held, so the desktop gives the keys back. */
	bool paused;

	/* Why the last portal request returned nothing: the user or the
	 * desktop said no (declined), or the call failed (error). */
	bool request_declined;
	char *request_error;
};

/* ------------------------------------------------------------------------ */
/* Collecting the combinations to bind                                       */

/* The arrays as the combination sets (obs-nix-portal-shortcut-sets.c) read
 * them. */
static struct obs_nix_portal_combos combos_of(const combo_array_t *combos)
{
	struct obs_nix_portal_combos view = {combos->array, combos->num};
	return view;
}

static struct obs_nix_portal_descriptions descriptions_of(const description_array_t *descriptions)
{
	struct obs_nix_portal_descriptions view = {(const char *const *)descriptions->array, descriptions->num};
	return view;
}

static bool has_combo(const combo_array_t *combos, obs_key_combination_t combo)
{
	struct obs_nix_portal_combos view = combos_of(combos);
	return obs_nix_portal_sets_has(&view, combo);
}

static void free_descriptions(description_array_t *descriptions)
{
	for (size_t i = 0; i < descriptions->num; i++)
		bfree(descriptions->array[i]);
	da_free(*descriptions);
}

static void copy_descriptions(description_array_t *dst, const description_array_t *src)
{
	free_descriptions(dst);
	for (size_t i = 0; i < src->num; i++) {
		char *copy = bstrdup(src->array[i]);
		da_push_back(*dst, &copy);
	}
}

/* Objects referenced while naming actions; released once the hotkey mutex
 * is no longer held (releasing a source may unregister its hotkeys). */
struct named_object {
	obs_hotkey_registerer_t type;
	void *object;
};

struct collected {
	combo_array_t combos;
	description_array_t descriptions;
	DARRAY(size_t) actions;
	DARRAY(struct named_object) objects;
};

/* "Push-to-talk (Mic)", "Start Recording". */
static void action_label(struct collected *collected, obs_hotkey_t *hotkey, struct dstr *label)
{
	const char *description = obs_hotkey_get_description(hotkey);
	if (!description || !*description)
		description = obs_hotkey_get_name(hotkey);
	dstr_copy(label, description);

	void *registerer = obs_hotkey_get_registerer(hotkey);
	struct named_object named = {obs_hotkey_get_registerer_type(hotkey), NULL};
	const char *owner = NULL;
	switch (named.type) {
	case OBS_HOTKEY_REGISTERER_FRONTEND:
		return;
	case OBS_HOTKEY_REGISTERER_SOURCE:
		named.object = obs_weak_source_get_source(registerer);
		owner = named.object ? obs_source_get_name(named.object) : NULL;
		break;
	case OBS_HOTKEY_REGISTERER_OUTPUT:
		named.object = obs_weak_output_get_output(registerer);
		owner = named.object ? obs_output_get_name(named.object) : NULL;
		break;
	case OBS_HOTKEY_REGISTERER_ENCODER:
		named.object = obs_weak_encoder_get_encoder(registerer);
		owner = named.object ? obs_encoder_get_name(named.object) : NULL;
		break;
	case OBS_HOTKEY_REGISTERER_SERVICE:
		named.object = obs_weak_service_get_service(registerer);
		owner = named.object ? obs_service_get_name(named.object) : NULL;
		break;
	}
	if (named.object)
		da_push_back(collected->objects, &named);
	if (owner && *owner)
		dstr_catf(label, " (%s)", owner);
}

static void release_named_objects(struct collected *collected)
{
	for (size_t i = 0; i < collected->objects.num; i++) {
		struct named_object *named = &collected->objects.array[i];
		switch (named->type) {
		case OBS_HOTKEY_REGISTERER_FRONTEND:
			break;
		case OBS_HOTKEY_REGISTERER_SOURCE:
			obs_source_release(named->object);
			break;
		case OBS_HOTKEY_REGISTERER_OUTPUT:
			obs_output_release(named->object);
			break;
		case OBS_HOTKEY_REGISTERER_ENCODER:
			obs_encoder_release(named->object);
			break;
		case OBS_HOTKEY_REGISTERER_SERVICE:
			obs_service_release(named->object);
			break;
		}
	}
	da_free(collected->objects);
}

/* One entry per bindable combination, described by the actions it runs:
 * "OBS: Push-to-talk (Mic), Start Recording". */
static bool collect_binding(void *data, size_t idx, obs_hotkey_binding_t *binding)
{
	UNUSED_PARAMETER(idx);
	struct collected *collected = data;
	obs_key_combination_t combo = obs_hotkey_binding_get_key_combination(binding);
	if (!bindable(combo))
		return true;

	size_t i = 0;
	while (i < collected->combos.num && !same_combo(collected->combos.array[i], combo))
		i++;
	if (i == collected->combos.num) {
		char *description = bstrdup("OBS:");
		size_t none = 0;
		da_push_back(collected->combos, &combo);
		da_push_back(collected->descriptions, &description);
		da_push_back(collected->actions, &none);
	}

	size_t *actions = &collected->actions.array[i];
	if (++*actions > MAX_DESCRIBED_ACTIONS)
		return true;

	struct dstr description = {0}, label = {0};
	action_label(collected, obs_hotkey_binding_get_hotkey(binding), &label);
	dstr_copy(&description, collected->descriptions.array[i]);
	dstr_cat(&description, *actions == 1 ? " " : ", ");
	dstr_cat_dstr(&description, &label);
	bfree(collected->descriptions.array[i]);
	collected->descriptions.array[i] = description.array;
	dstr_free(&label);
	return true;
}

static void collect_bindings(struct collected *collected)
{
	memset(collected, 0, sizeof(*collected));
	obs_enum_hotkey_bindings(collect_binding, collected);
	release_named_objects(collected);

	for (size_t i = 0; i < collected->combos.num; i++) {
		size_t more = collected->actions.array[i] > MAX_DESCRIBED_ACTIONS
				      ? collected->actions.array[i] - MAX_DESCRIBED_ACTIONS
				      : 0;
		if (!more)
			continue;
		struct dstr description = {0};
		dstr_printf(&description, "%s and %zu more", collected->descriptions.array[i], more);
		bfree(collected->descriptions.array[i]);
		collected->descriptions.array[i] = description.array;
	}
	da_free(collected->actions);
}

static void free_collected(struct collected *collected)
{
	da_free(collected->combos);
	free_descriptions(&collected->descriptions);
}

/* ------------------------------------------------------------------------ */
/* Status for the frontend (obs_hotkeys_get_background_state)               */

static inline struct obs_hotkeys_background *background_lock(void)
{
	pthread_mutex_lock(&obs->hotkeys.mutex);
	return &obs->hotkeys.background;
}

static inline void background_unlock(void)
{
	pthread_mutex_unlock(&obs->hotkeys.mutex);
	obs_hotkeys_background_status_changed();
}

static void set_state(enum obs_hotkeys_background_state state, const char *error)
{
	struct obs_hotkeys_background *background = background_lock();
	background->state = state;
	bfree(background->error);
	background->error = error ? bstrdup(error) : NULL;
	background_unlock();
}

static void set_flag(uint32_t flag, bool set)
{
	struct obs_hotkeys_background *background = background_lock();
	uint32_t flags = set ? background->flags | flag : background->flags & ~flag;
	bool changed = flags != background->flags;
	background->flags = flags;
	pthread_mutex_unlock(&obs->hotkeys.mutex);
	if (changed)
		obs_hotkeys_background_status_changed();
}

/* The combinations handed to the desktop, none bound yet. */
static void set_shortcuts(struct obs_hotkeys_background *background, const combo_array_t *combos)
{
	obs_hotkeys_background_clear_shortcuts(background);
	for (size_t i = 0; i < combos->num; i++) {
		struct obs_hotkey_background_shortcut *shortcut = da_push_back_new(background->shortcuts);
		shortcut->combo = combos->array[i];
	}
}

static void set_shortcuts_bound(struct obs_hotkeys_background *background, bool bound)
{
	for (size_t i = 0; i < background->shortcuts.num; i++)
		background->shortcuts.array[i].bound = bound;
}

static bool has_trigger(GVariant *props)
{
	const char *trigger = NULL;
	g_variant_lookup(props, "trigger_description", "&s", &trigger);
	return trigger && *trigger;
}

/* Records the triggers the desktop reports (BindShortcuts, ShortcutsChanged).
 * A desktop that reports any, or that has its own shortcut editor (portal
 * version 2: KDE), lets the user pick the keys. On such a desktop a shortcut
 * without a trigger is not bound (left without a key in the dialog, or the
 * dialog was cancelled before: KDE then returns every shortcut without a key
 * and does not ask again). On one that reports none and has no editor
 * (Hyprland) every returned shortcut is bound and the keys come from its
 * config. */
static void set_triggers(GVariant *shortcuts, bool bound)
{
	struct obs_hotkeys_background *background = background_lock();
	bool desktop_keys = (background->flags & OBS_HOTKEYS_BACKGROUND_CONFIGURABLE) != 0;
	GVariantIter iter;
	const char *id;
	GVariant *props;
	g_variant_iter_init(&iter, shortcuts);
	while (g_variant_iter_next(&iter, "(&s@a{sv})", &id, &props)) {
		desktop_keys = desktop_keys || has_trigger(props);
		g_variant_unref(props);
	}

	g_variant_iter_init(&iter, shortcuts);
	while (g_variant_iter_next(&iter, "(&s@a{sv})", &id, &props)) {
		const char *trigger = NULL;
		obs_key_combination_t combo;
		g_variant_lookup(props, "trigger_description", "&s", &trigger);
		bool triggered = trigger && *trigger;
		char *copy = triggered ? bstrdup(trigger) : NULL;
		g_variant_unref(props);
		if (!obs_nix_portal_shortcut_combo(id, &combo)) {
			bfree(copy);
			continue;
		}

		for (size_t i = 0; i < background->shortcuts.num; i++) {
			struct obs_hotkey_background_shortcut *shortcut = &background->shortcuts.array[i];
			if (!same_combo(shortcut->combo, combo))
				continue;
			bfree(shortcut->trigger);
			shortcut->trigger = copy ? bstrdup(copy) : NULL;
			if (bound || desktop_keys)
				shortcut->bound = !desktop_keys || triggered;
		}
		bfree(copy);
	}
	if (desktop_keys)
		background->flags |= OBS_HOTKEYS_BACKGROUND_DESKTOP_TRIGGERS;
	else if (bound)
		background->flags &= ~OBS_HOTKEYS_BACKGROUND_DESKTOP_TRIGGERS;
	background_unlock();
}

static bool probing(void)
{
	pthread_mutex_lock(&obs->hotkeys.mutex);
	bool probe = obs->hotkeys.background.probe;
	pthread_mutex_unlock(&obs->hotkeys.mutex);
	return probe;
}

/* ------------------------------------------------------------------------ */
/* Portal requests                                                           */

struct request_result {
	bool done;
	uint32_t response;
	GVariant *results;
};

static void request_response(GDBusConnection *conn, const char *sender_name, const char *object_path,
			     const char *interface_name, const char *signal_name, GVariant *parameters,
			     gpointer user_data)
{
	UNUSED_PARAMETER(conn);
	UNUSED_PARAMETER(sender_name);
	UNUSED_PARAMETER(object_path);
	UNUSED_PARAMETER(interface_name);
	UNUSED_PARAMETER(signal_name);
	struct request_result *result = user_data;

	if (result->done || !g_variant_is_of_type(parameters, G_VARIANT_TYPE("(ua{sv})")))
		return;
	g_variant_get(parameters, "(u@a{sv})", &result->response, &result->results);
	result->done = true;
}

static char *new_token(struct obs_nix_portal_shortcuts *portal, const char *kind)
{
	return g_strdup_printf("obs_shortcuts_%s_%u_%u", kind, (unsigned)getpid(), ++portal->token);
}

static inline bool waiting_for_portal(struct obs_nix_portal_shortcuts *portal)
{
	return !os_atomic_load_bool(&portal->stopping) && !portal->portal_gone;
}

/* Calls a portal method that answers through a Request object and waits for
 * the Response (BindShortcuts may wait for the user in a dialog). Returns
 * the results on success, NULL if the call failed or was cancelled, or if
 * the client is stopping or the portal went away meanwhile. params carries
 * handle_token = token in its options. */
static GVariant *portal_request(struct obs_nix_portal_shortcuts *portal, const char *method, GVariant *params,
				const char *token)
{
	struct request_result result = {0};
	char *path = g_strdup_printf(PORTAL_PATH "/request/%s/%s", portal->sender, token);

	portal->request_declined = false;
	g_clear_pointer(&portal->request_error, g_free);

	/* Subscribed before the call, so a fast Response is not missed. */
	guint signal_id = g_dbus_connection_signal_subscribe(portal->conn, PORTAL_NAME, REQUEST_IFACE, "Response", path,
							     NULL, G_DBUS_SIGNAL_FLAGS_NO_MATCH_RULE, request_response,
							     &result, NULL);

	GError *error = NULL;
	GVariant *reply = g_dbus_connection_call_sync(portal->conn, PORTAL_NAME, PORTAL_PATH, SHORTCUTS_IFACE, method,
						      params, G_VARIANT_TYPE("(o)"), G_DBUS_CALL_FLAGS_NONE, -1, NULL,
						      &error);
	if (!reply) {
		blog(LOG_WARNING, "[portal] GlobalShortcuts.%s failed: %s", method, error->message);
		g_dbus_error_strip_remote_error(error);
		portal->request_error = g_strdup(error->message);
		g_error_free(error);
		g_dbus_connection_signal_unsubscribe(portal->conn, signal_id);
		g_free(path);
		return NULL;
	}

	const char *handle = NULL;
	g_variant_get(reply, "(&o)", &handle);
	if (strcmp(handle, path) != 0) {
		/* Portal older than 0.9 ignores handle_token. */
		g_dbus_connection_signal_unsubscribe(portal->conn, signal_id);
		signal_id = g_dbus_connection_signal_subscribe(portal->conn, PORTAL_NAME, REQUEST_IFACE, "Response",
							       handle, NULL, G_DBUS_SIGNAL_FLAGS_NO_MATCH_RULE,
							       request_response, &result, NULL);
	}
	g_variant_unref(reply);

	/* Only signal handlers and the sync timer run in here; the timer just
	 * marks a sync as due, so sync_shortcuts() is never re-entered. */
	while (!result.done && waiting_for_portal(portal))
		g_main_context_iteration(portal->context, true);

	g_dbus_connection_signal_unsubscribe(portal->conn, signal_id);
	g_free(path);

	if (!result.done)
		return NULL;
	if (result.response != 0) {
		blog(LOG_INFO, "[portal] GlobalShortcuts.%s was %s", method,
		     result.response == 1 ? "cancelled" : "refused");
		portal->request_declined = true;
		g_clear_pointer(&result.results, g_variant_unref);
		return NULL;
	}
	return result.results;
}

/* ------------------------------------------------------------------------ */
/* Activated / Deactivated                                                   */

static gboolean flush_released(gpointer data)
{
	struct obs_nix_portal_shortcuts *portal = data;
	if (obs_nix_hotkey_state_flush(portal->state))
		return G_SOURCE_CONTINUE;

	g_source_unref(portal->flush_source);
	portal->flush_source = NULL;
	return G_SOURCE_REMOVE;
}

static void set_shortcut(struct obs_nix_portal_shortcuts *portal, obs_key_combination_t combo, bool active)
{
	obs_nix_hotkey_state_set_shortcut(portal->state, combo, active);

	/* A release the hotkey thread has not seen yet is kept until it has
	 * (so a tap still presses); drop it once it has. */
	if (!active && !portal->flush_source && obs_nix_hotkey_state_flush(portal->state)) {
		portal->flush_source = g_timeout_source_new(FLUSH_INTERVAL_MS);
		g_source_set_callback(portal->flush_source, flush_released, portal, NULL);
		g_source_attach(portal->flush_source, portal->context);
	}
}

static void release_bound(struct obs_nix_portal_shortcuts *portal)
{
	for (size_t i = 0; i < portal->bound.num; i++)
		set_shortcut(portal, portal->bound.array[i], false);
}

static void shortcut_signal(GDBusConnection *conn, const char *sender_name, const char *object_path,
			    const char *interface_name, const char *signal_name, GVariant *parameters,
			    gpointer user_data)
{
	UNUSED_PARAMETER(conn);
	UNUSED_PARAMETER(sender_name);
	UNUSED_PARAMETER(object_path);
	UNUSED_PARAMETER(interface_name);
	struct obs_nix_portal_shortcuts *portal = user_data;
	const char *session_path, *id;
	obs_key_combination_t combo;

	if (!g_variant_is_of_type(parameters, G_VARIANT_TYPE("(osta{sv})")))
		return;
	g_variant_get(parameters, "(&o&st@a{sv})", &session_path, &id, NULL, NULL);
	if (!portal->session_path || strcmp(session_path, portal->session_path) != 0)
		return;
	if (!obs_nix_portal_shortcut_combo(id, &combo) || !has_combo(&portal->bound, combo))
		return;

	bool active = strcmp(signal_name, "Activated") == 0;
	if (probing()) {
		/* Testing a key from Settings: report it, press nothing. */
		if (active) {
			calldata_t data;
			uint8_t stack[256];
			calldata_init_fixed(&data, stack, sizeof(stack));
			calldata_set_string(&data, "id", id);
			signal_handler_signal(obs->hotkeys.signals, "hotkey_background_probe", &data);
		}
		return;
	}
	set_shortcut(portal, combo, active);
}

static void log_shortcuts(GVariant *shortcuts, const char *what)
{
	GVariantIter iter;
	const char *id;
	GVariant *props;
	g_variant_iter_init(&iter, shortcuts);
	while (g_variant_iter_next(&iter, "(&s@a{sv})", &id, &props)) {
		const char *trigger = NULL;
		g_variant_lookup(props, "trigger_description", "&s", &trigger);
		blog(LOG_INFO, "[portal] global shortcut %s %s: %s", id, what,
		     trigger && *trigger ? trigger : "(no trigger)");
		g_variant_unref(props);
	}
}

/* The user changed a trigger in the desktop settings. The shortcut id names
 * the OBS combination, so the new trigger already drives the same hotkeys;
 * only log it and show it. */
static void shortcuts_changed(GDBusConnection *conn, const char *sender_name, const char *object_path,
			      const char *interface_name, const char *signal_name, GVariant *parameters,
			      gpointer user_data)
{
	UNUSED_PARAMETER(conn);
	UNUSED_PARAMETER(sender_name);
	UNUSED_PARAMETER(object_path);
	UNUSED_PARAMETER(interface_name);
	UNUSED_PARAMETER(signal_name);
	struct obs_nix_portal_shortcuts *portal = user_data;
	const char *session_path;
	GVariant *shortcuts;

	if (!g_variant_is_of_type(parameters, G_VARIANT_TYPE("(oa(sa{sv}))")))
		return;
	g_variant_get(parameters, "(&o@a(sa{sv}))", &session_path, &shortcuts);
	if (portal->session_path && strcmp(session_path, portal->session_path) == 0) {
		log_shortcuts(shortcuts, "changed");
		set_triggers(shortcuts, false);
	}
	g_variant_unref(shortcuts);
}

/* ------------------------------------------------------------------------ */
/* Sessions                                                                  */

/* Forgets the session without telling the portal; held shortcuts are
 * released. */
static void forget_session(struct obs_nix_portal_shortcuts *portal)
{
	g_clear_pointer(&portal->session_path, g_free);
	release_bound(portal);
	da_resize(portal->bound, 0);
	free_descriptions(&portal->bound_descriptions);
}

static void close_session(struct obs_nix_portal_shortcuts *portal)
{
	if (!portal->session_path)
		return;

	if (!portal->portal_gone) {
		GVariant *reply = g_dbus_connection_call_sync(portal->conn, PORTAL_NAME, portal->session_path,
							      SESSION_IFACE, "Close", NULL, NULL,
							      G_DBUS_CALL_FLAGS_NO_AUTO_START, 1000, NULL, NULL);
		if (reply)
			g_variant_unref(reply);
	}
	forget_session(portal);
}

/* The desktop closed the session (the user revoked it, or the backend went
 * away): do not bind the same shortcuts again behind the user's back, as
 * that may prompt again. A new combination, or a portal restart, binds. */
static void session_closed(GDBusConnection *conn, const char *sender_name, const char *object_path,
			   const char *interface_name, const char *signal_name, GVariant *parameters,
			   gpointer user_data)
{
	UNUSED_PARAMETER(conn);
	UNUSED_PARAMETER(sender_name);
	UNUSED_PARAMETER(interface_name);
	UNUSED_PARAMETER(signal_name);
	UNUSED_PARAMETER(parameters);
	struct obs_nix_portal_shortcuts *portal = user_data;

	if (!portal->session_path || strcmp(object_path, portal->session_path) != 0)
		return;

	blog(LOG_INFO, "[portal] The desktop closed the GlobalShortcuts session; "
		       "global hotkeys are bound again when a new key combination is assigned");
	da_copy(portal->declined, portal->bound);
	da_resize(portal->approved, 0);
	portal->partly_declined = false;
	forget_session(portal);

	struct obs_hotkeys_background *background = background_lock();
	background->state = OBS_HOTKEYS_BACKGROUND_CLOSED;
	background->flags &= ~OBS_HOTKEYS_BACKGROUND_PARTLY_DECLINED;
	set_shortcuts_bound(background, false);
	background_unlock();
}

static bool create_session(struct obs_nix_portal_shortcuts *portal)
{
	char *token = new_token(portal, "request");
	char *session_token = new_token(portal, "session");

	GVariantBuilder options;
	g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
	g_variant_builder_add(&options, "{sv}", "handle_token", g_variant_new_string(token));
	g_variant_builder_add(&options, "{sv}", "session_handle_token", g_variant_new_string(session_token));

	GVariant *results = portal_request(portal, "CreateSession", g_variant_new("(a{sv})", &options), token);
	g_free(token);
	g_free(session_token);
	if (!results)
		return false;

	GVariant *handle = g_variant_lookup_value(results, "session_handle", NULL);
	if (handle && (g_variant_is_of_type(handle, G_VARIANT_TYPE_STRING) ||
		       g_variant_is_of_type(handle, G_VARIANT_TYPE_OBJECT_PATH)))
		portal->session_path = g_variant_dup_string(handle, NULL);
	g_clear_pointer(&handle, g_variant_unref);
	g_variant_unref(results);

	if (!portal->session_path) {
		blog(LOG_WARNING, "[portal] GlobalShortcuts.CreateSession returned no session");
		return false;
	}
	return true;
}

static bool bind_shortcuts(struct obs_nix_portal_shortcuts *portal, const combo_array_t *combos,
			   const description_array_t *descriptions)
{
	GVariantBuilder shortcuts;
	struct dstr id = {0}, trigger = {0};

	g_variant_builder_init(&shortcuts, G_VARIANT_TYPE("a(sa{sv})"));
	for (size_t i = 0; i < combos->num; i++) {
		if (!obs_nix_portal_shortcut_id(combos->array[i], &id))
			continue;

		GVariantBuilder props;
		g_variant_builder_init(&props, G_VARIANT_TYPE_VARDICT);
		bool has_trigger = obs_nix_portal_shortcut_trigger(combos->array[i], portal->key_name, &trigger);
		/* The actions the key runs, so the desktop's list says what each
		 * shortcut does. */
		const char *description = i < descriptions->num ? descriptions->array[i] : "OBS";
		g_variant_builder_add(&props, "{sv}", "description", g_variant_new_string(description));
		if (has_trigger)
			g_variant_builder_add(&props, "{sv}", "preferred_trigger", g_variant_new_string(trigger.array));
		g_variant_builder_add(&shortcuts, "(sa{sv})", id.array, &props);
	}
	dstr_free(&id);
	dstr_free(&trigger);

	char *token = new_token(portal, "request");
	GVariantBuilder options;
	g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
	g_variant_builder_add(&options, "{sv}", "handle_token", g_variant_new_string(token));

	GVariant *results = portal_request(
		portal, "BindShortcuts",
		g_variant_new("(oa(sa{sv})sa{sv})", portal->session_path, &shortcuts, "", &options), token);
	g_free(token);
	if (!results)
		return false;

	GVariant *bound = g_variant_lookup_value(results, "shortcuts", G_VARIANT_TYPE("a(sa{sv})"));
	if (bound) {
		log_shortcuts(bound, "bound");
		set_triggers(bound, true);
		g_variant_unref(bound);
	}
	g_variant_unref(results);
	return true;
}

/* The combinations of combos at the positions selected (from
 * obs-nix-portal-shortcut-sets.c), with their descriptions. */
static void take_combos(const combo_array_t *combos, const description_array_t *descriptions, const size_t *selected,
			size_t num_selected, combo_array_t *out, description_array_t *out_descriptions)
{
	for (size_t j = 0; j < num_selected; j++) {
		size_t i = selected[j];
		char *description = bstrdup(i < descriptions->num ? descriptions->array[i] : "OBS");
		da_push_back(*out, &combos->array[i]);
		da_push_back(*out_descriptions, &description);
	}
}

/* The desktop's dialog for a request with new combinations was declined
 * (GNOME lists every shortcut again in it): bind the ones it approved
 * before again, which needs no dialog, so they keep working. The rest of
 * the bindings stay unbound, and are left out of later binds, until
 * obs_hotkeys_retry_background(). The session's set is the full one, so
 * the next sync does not bind again by itself. */
static bool bind_approved(struct obs_nix_portal_shortcuts *portal, const combo_array_t *current,
			  const description_array_t *descriptions, const combo_array_t *request)
{
	struct obs_nix_portal_combos current_combos = combos_of(current);
	struct obs_nix_portal_combos approved = combos_of(&portal->approved);
	struct obs_nix_portal_combos requested = combos_of(request);
	DARRAY(size_t) part_selected;
	DARRAY(size_t) declined_selected;
	da_init(part_selected);
	da_init(declined_selected);
	da_resize(part_selected, current->num);
	da_resize(declined_selected, current->num);
	size_t num_part, num_declined;
	bool bound = obs_nix_portal_sets_partition(&current_combos, &approved, &requested, part_selected.array,
						   &num_part, declined_selected.array, &num_declined);

	combo_array_t part;
	description_array_t part_descriptions;
	da_init(part);
	da_init(part_descriptions);
	take_combos(current, descriptions, part_selected.array, num_part, &part, &part_descriptions);
	if (bound) {
		blog(LOG_INFO,
		     "[portal] The desktop declined the new shortcuts; binding the %zu it approved before again",
		     part.num);
		da_copy(portal->bound, *current);
		copy_descriptions(&portal->bound_descriptions, descriptions);
		bound = create_session(portal) && bind_shortcuts(portal, &part, &part_descriptions);
		if (!bound)
			close_session(portal);
	}
	if (bound) {
		da_resize(portal->declined, 0);
		for (size_t i = 0; i < num_declined; i++)
			da_push_back(portal->declined, &current->array[declined_selected.array[i]]);
		portal->partly_declined = true;
		set_flag(OBS_HOTKEYS_BACKGROUND_PARTLY_DECLINED, true);
		set_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, NULL);
	}
	da_free(part_selected);
	da_free(declined_selected);
	da_free(part);
	free_descriptions(&part_descriptions);
	return bound;
}

/* The hotkey thread drops every press while this is set (the hotkey focus
 * setting, or a dialog that disables hotkeys). */
static bool background_press_disabled(void)
{
	pthread_mutex_lock(&obs->hotkeys.mutex);
	bool disabled = obs->hotkeys.thread_disable_press;
	pthread_mutex_unlock(&obs->hotkeys.mutex);
	return disabled;
}

/* Takes a pending obs_hotkeys_retry_background(). */
static bool take_retry(void)
{
	pthread_mutex_lock(&obs->hotkeys.mutex);
	bool retry = obs->hotkeys.background.retry;
	obs->hotkeys.background.retry = false;
	pthread_mutex_unlock(&obs->hotkeys.mutex);
	return retry;
}

/* Brings the portal session in line with the current bindings. The portal
 * accepts one BindShortcuts call per session, so a change closes the
 * session and binds the new set in a fresh one; the ids of combinations
 * that stay bound do not change, so the desktop keeps their triggers.
 *
 * While background presses are disabled the wanted set is empty: OBS would
 * drop every activation, so the desktop must not hold the keys for it.
 * Nothing is marked declined by that, so enabling presses binds the same
 * set again without a new prompt, and the status keeps the last binding.
 * A probe (testing a key from Settings) keeps the set bound meanwhile. */
static void sync_shortcuts(struct obs_nix_portal_shortcuts *portal)
{
	struct collected collected = {0};
	combo_array_t current;
	da_init(current);

	bool retry = take_retry();
	if (retry)
		da_resize(portal->declined, 0);

	bool paused = background_press_disabled();
	if (paused != portal->paused) {
		blog(LOG_INFO, "[portal] Hotkeys %s; global hotkeys %s", paused ? "disabled" : "enabled",
		     paused ? "released until they are enabled" : "bound again");
		portal->paused = paused;
	}
	bool wanted = !paused || probing();
	set_flag(OBS_HOTKEYS_BACKGROUND_PAUSED, !wanted);
	if (wanted) {
		collect_bindings(&collected);
		da_move(current, collected.combos);
	}

	if (wanted && current.num == 0) {
		struct obs_hotkeys_background *background = background_lock();
		background->state = OBS_HOTKEYS_BACKGROUND_IDLE;
		obs_hotkeys_background_clear_shortcuts(background);
		background_unlock();
	}

	struct obs_nix_portal_combos current_combos = combos_of(&current);
	struct obs_nix_portal_descriptions current_descriptions = descriptions_of(&collected.descriptions);
	struct obs_nix_portal_combos bound_combos = combos_of(&portal->bound);
	struct obs_nix_portal_descriptions bound_descriptions = descriptions_of(&portal->bound_descriptions);
	struct obs_nix_portal_combos declined = combos_of(&portal->declined);
	bool unchanged = obs_nix_portal_sets_unchanged(portal->session_path != NULL, retry, portal->partly_declined,
						       &current_combos, &current_descriptions, &bound_combos,
						       &bound_descriptions, &declined);
	if (unchanged) {
		da_free(current);
		free_collected(&collected);
		return;
	}

	close_session(portal);
	if (current.num == 0) {
		da_free(current);
		free_collected(&collected);
		return;
	}

	struct obs_hotkeys_background *background = background_lock();
	background->state = OBS_HOTKEYS_BACKGROUND_WAITING;
	bfree(background->error);
	background->error = NULL;
	set_shortcuts(background, &current);
	background_unlock();

	combo_array_t request;
	description_array_t request_descriptions;
	DARRAY(size_t) request_selected;
	da_init(request);
	da_init(request_descriptions);
	da_init(request_selected);
	da_resize(request_selected, current.num);
	size_t num_request;
	current_combos = combos_of(&current);
	declined = combos_of(&portal->declined);
	bool partly = obs_nix_portal_sets_request(&current_combos, portal->partly_declined, &declined,
						  request_selected.array, &num_request);
	take_combos(&current, &collected.descriptions, request_selected.array, num_request, &request,
		    &request_descriptions);
	da_free(request_selected);

	/* Set before binding: Activated may be dispatched right after the
	 * BindShortcuts response, while bind_shortcuts() is still waiting. */
	da_copy(portal->bound, current);
	copy_descriptions(&portal->bound_descriptions, &collected.descriptions);
	bool bound = request.num > 0 && create_session(portal) &&
		     bind_shortcuts(portal, &request, &request_descriptions);
	if (!bound) {
		close_session(portal);
		bool nothing = request.num == 0;
		bool kept = portal->request_declined && !nothing &&
			    bind_approved(portal, &current, &collected.descriptions, &request);
		free_collected(&collected);
		da_free(request);
		free_descriptions(&request_descriptions);
		if (kept) {
			da_free(current);
			return;
		}
		da_move(portal->declined, current);
		portal->partly_declined = false;
		set_flag(OBS_HOTKEYS_BACKGROUND_PARTLY_DECLINED, false);
		if (portal->request_declined || nothing)
			set_state(OBS_HOTKEYS_BACKGROUND_DECLINED, NULL);
		else if (!portal->portal_gone && !os_atomic_load_bool(&portal->stopping))
			set_state(OBS_HOTKEYS_BACKGROUND_FAILED, portal->request_error);
		return;
	}
	if (!partly)
		da_resize(portal->declined, 0);
	da_copy(portal->approved, request);
	free_collected(&collected);
	da_free(request);
	free_descriptions(&request_descriptions);
	da_free(current);
	portal->partly_declined = partly;
	set_flag(OBS_HOTKEYS_BACKGROUND_PARTLY_DECLINED, partly);
	set_state(OBS_HOTKEYS_BACKGROUND_ACTIVE, NULL);
}

/* Takes a pending obs_hotkeys_configure_background(). Without a session it
 * stays pending for the bind a probe is about to make (the probe ending
 * drops it), unless the portal cannot do it at all. */
static bool take_configure(struct obs_nix_portal_shortcuts *portal)
{
	pthread_mutex_lock(&obs->hotkeys.mutex);
	bool configure = obs->hotkeys.background.configure;
	if (configure && (portal->session_path || portal->version < 2))
		obs->hotkeys.background.configure = false;
	else
		configure = false;
	pthread_mutex_unlock(&obs->hotkeys.mutex);
	return configure;
}

/* Opens the desktop's editor for the shortcuts of the current session
 * (after sync_shortcuts(), so a probe has bound them). */
static void configure_shortcuts(struct obs_nix_portal_shortcuts *portal)
{
	if (!take_configure(portal))
		return;
	if (portal->version < 2) {
		blog(LOG_WARNING, "[portal] Cannot open the desktop's shortcut settings: portal version %u",
		     portal->version);
		set_flag(OBS_HOTKEYS_BACKGROUND_CONFIGURE_FAILED, true);
		return;
	}

	GError *error = NULL;
	GVariantBuilder options;
	g_variant_builder_init(&options, G_VARIANT_TYPE_VARDICT);
	GVariant *reply = g_dbus_connection_call_sync(portal->conn, PORTAL_NAME, PORTAL_PATH, SHORTCUTS_IFACE,
						      "ConfigureShortcuts",
						      g_variant_new("(osa{sv})", portal->session_path, "", &options),
						      NULL, G_DBUS_CALL_FLAGS_NONE, 5000, NULL, &error);
	if (!reply) {
		blog(LOG_WARNING, "[portal] GlobalShortcuts.ConfigureShortcuts failed: %s", error->message);
		g_error_free(error);
		set_flag(OBS_HOTKEYS_BACKGROUND_CONFIGURE_FAILED, true);
		return;
	}
	g_variant_unref(reply);
	blog(LOG_INFO, "[portal] Opened the desktop's shortcut settings");
}

/* ------------------------------------------------------------------------ */
/* Thread                                                                    */

/* Host (non-sandboxed) apps tell the portal who they are, so the desktop
 * stores their shortcuts under a stable app id; this must be the first
 * portal call on the connection. Sandboxed apps and older portals refuse
 * it, which is fine. */
static void register_app_id(struct obs_nix_portal_shortcuts *portal)
{
	GError *error = NULL;
	GVariant *reply = g_dbus_connection_call_sync(portal->conn, PORTAL_NAME, PORTAL_PATH, REGISTRY_IFACE,
						      "Register", g_variant_new("(sa{sv})", APP_ID, NULL), NULL,
						      G_DBUS_CALL_FLAGS_NONE, 5000, NULL, &error);
	if (reply) {
		g_variant_unref(reply);
		return;
	}
	/* Without the id the portal may refuse the session ("An app id is
	 * required"); the desktop file must be installed for it. No portal, or
	 * one older than 1.18 (no Registry), is reported elsewhere. */
	if (!g_error_matches(error, G_DBUS_ERROR, G_DBUS_ERROR_SERVICE_UNKNOWN) &&
	    !g_error_matches(error, G_DBUS_ERROR, G_DBUS_ERROR_UNKNOWN_METHOD) &&
	    !g_error_matches(error, G_DBUS_ERROR, G_DBUS_ERROR_UNKNOWN_INTERFACE))
		blog(LOG_WARNING, "[portal] Registering the app id %s failed (%s); is %s.desktop installed?", APP_ID,
		     error->message, APP_ID);
	g_error_free(error);
}

static void portal_vanished(GDBusConnection *conn, const char *name, gpointer user_data)
{
	UNUSED_PARAMETER(conn);
	UNUSED_PARAMETER(name);
	struct obs_nix_portal_shortcuts *portal = user_data;

	if (portal->portal_gone)
		return;
	blog(LOG_WARNING, "[portal] The portal left the session bus; global hotkeys are paused");
	portal->portal_gone = true;
	forget_session(portal);

	struct obs_hotkeys_background *background = background_lock();
	background->state = OBS_HOTKEYS_BACKGROUND_FAILED;
	background->flags |= OBS_HOTKEYS_BACKGROUND_RESTARTING;
	bfree(background->error);
	background->error = bstrdup("The desktop portal stopped");
	set_shortcuts_bound(background, false);
	background_unlock();
}

/* A restarted portal knows nothing of us: register again and bind. */
static void portal_appeared(GDBusConnection *conn, const char *name, const char *owner, gpointer user_data)
{
	UNUSED_PARAMETER(conn);
	UNUSED_PARAMETER(name);
	UNUSED_PARAMETER(owner);
	struct obs_nix_portal_shortcuts *portal = user_data;

	if (!portal->portal_gone)
		return;
	blog(LOG_INFO, "[portal] The portal is back; binding global hotkeys again");
	portal->portal_gone = false;
	register_app_id(portal);
	da_resize(portal->declined, 0);
	portal->partly_declined = false;
	portal->sync_due = true;

	/* Nothing is bound until the next sync, which may wait for Settings
	 * to close. */
	struct obs_hotkeys_background *background = background_lock();
	background->state = OBS_HOTKEYS_BACKGROUND_IDLE;
	background->flags &= ~(OBS_HOTKEYS_BACKGROUND_RESTARTING | OBS_HOTKEYS_BACKGROUND_PARTLY_DECLINED);
	bfree(background->error);
	background->error = NULL;
	background_unlock();
}

static bool connect_portal(struct obs_nix_portal_shortcuts *portal)
{
	GError *error = NULL;
	char *address = g_dbus_address_get_for_bus_sync(G_BUS_TYPE_SESSION, NULL, &error);
	if (!address) {
		blog(LOG_WARNING, "[portal] No D-Bus session bus (%s); global hotkeys are disabled", error->message);
		set_state(OBS_HOTKEYS_BACKGROUND_UNAVAILABLE, error->message);
		g_error_free(error);
		return false;
	}

	/* A private connection, so the app id registered below and the
	 * dispatch of its signals belong to this thread only. */
	portal->conn = g_dbus_connection_new_for_address_sync(
		address, G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT | G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION,
		NULL, NULL, &error);
	g_free(address);
	if (!portal->conn) {
		blog(LOG_WARNING, "[portal] Cannot connect to the session bus (%s); global hotkeys are disabled",
		     error->message);
		set_state(OBS_HOTKEYS_BACKGROUND_UNAVAILABLE, error->message);
		g_error_free(error);
		return false;
	}

	char *sender = g_strdup(g_dbus_connection_get_unique_name(portal->conn) + 1);
	g_strdelimit(sender, ".", '_');
	portal->sender = sender;

	register_app_id(portal);

	GVariant *reply =
		g_dbus_connection_call_sync(portal->conn, PORTAL_NAME, PORTAL_PATH, "org.freedesktop.DBus.Properties",
					    "Get", g_variant_new("(ss)", SHORTCUTS_IFACE, "version"),
					    G_VARIANT_TYPE("(v)"), G_DBUS_CALL_FLAGS_NONE, 5000, NULL, &error);
	if (!reply) {
		blog(LOG_WARNING,
		     "[portal] GlobalShortcuts portal not available (%s); "
		     "hotkeys only work while OBS is focused",
		     error->message);
		g_dbus_error_strip_remote_error(error);
		set_state(OBS_HOTKEYS_BACKGROUND_UNAVAILABLE, error->message);
		g_error_free(error);
		return false;
	}

	GVariant *version = NULL;
	g_variant_get(reply, "(v)", &version);
	uint32_t version_num = g_variant_is_of_type(version, G_VARIANT_TYPE_UINT32) ? g_variant_get_uint32(version) : 0;
	blog(LOG_INFO, "[portal] GlobalShortcuts portal version %u; global hotkeys enabled", version_num);
	g_variant_unref(version);
	g_variant_unref(reply);

	/* Version 2 adds ConfigureShortcuts (the desktop's own editor). */
	portal->version = version_num;
	set_flag(OBS_HOTKEYS_BACKGROUND_CONFIGURABLE, version_num >= 2);
	set_state(OBS_HOTKEYS_BACKGROUND_IDLE, NULL);

	portal->activated_id =
		g_dbus_connection_signal_subscribe(portal->conn, PORTAL_NAME, SHORTCUTS_IFACE, "Activated", PORTAL_PATH,
						   NULL, G_DBUS_SIGNAL_FLAGS_NONE, shortcut_signal, portal, NULL);
	portal->deactivated_id = g_dbus_connection_signal_subscribe(portal->conn, PORTAL_NAME, SHORTCUTS_IFACE,
								    "Deactivated", PORTAL_PATH, NULL,
								    G_DBUS_SIGNAL_FLAGS_NONE, shortcut_signal, portal,
								    NULL);
	portal->changed_id = g_dbus_connection_signal_subscribe(portal->conn, PORTAL_NAME, SHORTCUTS_IFACE,
								"ShortcutsChanged", PORTAL_PATH, NULL,
								G_DBUS_SIGNAL_FLAGS_NONE, shortcuts_changed, portal,
								NULL);
	portal->closed_id = g_dbus_connection_signal_subscribe(portal->conn, PORTAL_NAME, SESSION_IFACE, "Closed", NULL,
							       NULL, G_DBUS_SIGNAL_FLAGS_NONE, session_closed, portal,
							       NULL);
	portal->watch_id = g_bus_watch_name_on_connection(portal->conn, PORTAL_NAME, G_BUS_NAME_WATCHER_FLAGS_NONE,
							  portal_appeared, portal_vanished, portal, NULL);
	return true;
}

static void disconnect_portal(struct obs_nix_portal_shortcuts *portal)
{
	if (!portal->conn)
		return;

	if (portal->watch_id)
		g_bus_unwatch_name(portal->watch_id);
	guint *ids[] = {&portal->activated_id, &portal->deactivated_id, &portal->changed_id, &portal->closed_id};
	for (size_t i = 0; i < sizeof(ids) / sizeof(ids[0]); i++) {
		if (*ids[i])
			g_dbus_connection_signal_unsubscribe(portal->conn, *ids[i]);
	}
	g_dbus_connection_close_sync(portal->conn, NULL, NULL);
	g_clear_object(&portal->conn);
}

static void *portal_thread(void *data)
{
	struct obs_nix_portal_shortcuts *portal = data;

	os_set_thread_name("libobs: portal shortcuts");
	g_main_context_push_thread_default(portal->context);

	if (connect_portal(portal)) {
		/* Bindings loaded before the client started. */
		portal->sync_due = true;
		while (!os_atomic_load_bool(&portal->stopping)) {
			if (portal->sync_due && !portal->portal_gone) {
				portal->sync_due = false;
				sync_shortcuts(portal);
				configure_shortcuts(portal);
			} else {
				g_main_context_iteration(portal->context, true);
			}
		}

		close_session(portal);
		obs_nix_hotkey_state_flush(portal->state);
	}

	if (portal->flush_source) {
		g_source_destroy(portal->flush_source);
		g_clear_pointer(&portal->flush_source, g_source_unref);
	}
	disconnect_portal(portal);
	/* Run pending unsubscribe notifications before the context goes. */
	while (g_main_context_iteration(portal->context, false))
		;

	g_main_context_pop_thread_default(portal->context);
	return NULL;
}

/* ------------------------------------------------------------------------ */
/* Binding changes                                                           */

struct sync_source {
	GSource base;
	struct obs_nix_portal_shortcuts *portal;
};

static gboolean sync_dispatch(GSource *source, GSourceFunc callback, gpointer user_data)
{
	UNUSED_PARAMETER(callback);
	UNUSED_PARAMETER(user_data);
	g_source_set_ready_time(source, -1);
	((struct sync_source *)source)->portal->sync_due = true;
	return G_SOURCE_CONTINUE;
}

static GSourceFuncs sync_source_funcs = {NULL, NULL, sync_dispatch, NULL, NULL, NULL};

/* hotkey_bindings_changed / hotkey_unregister, on any thread: (re)start the
 * delay before the portal thread binds the new set. */
static void bindings_changed(void *data, calldata_t *params)
{
	UNUSED_PARAMETER(params);
	struct obs_nix_portal_shortcuts *portal = data;
	g_source_set_ready_time(portal->sync_source, g_get_monotonic_time() + SYNC_DELAY_MS * G_TIME_SPAN_MILLISECOND);
}

/* hotkey_background_press_changed: right away, so the desktop gives a key
 * back (or holds it again) as soon as OBS stops (or starts) using it. */
static void background_press_changed(void *data, calldata_t *params)
{
	UNUSED_PARAMETER(params);
	struct obs_nix_portal_shortcuts *portal = data;
	g_source_set_ready_time(portal->sync_source, 0);
}

static const struct {
	const char *name;
	signal_callback_t callback;
} hotkey_signals[] = {
	{"hotkey_bindings_changed", bindings_changed},
	{"hotkey_unregister", bindings_changed},
	{"hotkey_background_press_changed", background_press_changed},
	/* A source's name is part of its hotkeys' descriptions. */
	{"source_rename", bindings_changed},
};

static void connect_binding_signals(struct obs_nix_portal_shortcuts *portal, bool connect)
{
	signal_handler_t *signals = obs->hotkeys.signals;
	if (!signals)
		return;
	for (size_t i = 0; i < sizeof(hotkey_signals) / sizeof(hotkey_signals[0]); i++) {
		if (connect)
			signal_handler_connect(signals, hotkey_signals[i].name, hotkey_signals[i].callback, portal);
		else
			signal_handler_disconnect(signals, hotkey_signals[i].name, hotkey_signals[i].callback, portal);
	}
}

static bool published_trigger(void *data, obs_key_combination_t combo, struct dstr *trigger)
{
	struct obs_nix_portal_shortcuts *portal = data;
	return obs_nix_portal_shortcut_trigger(combo, portal->key_name, trigger);
}

/* obs_hotkeys_retry_background / obs_hotkeys_set_background_probe. */
static void wake_portal(void *data)
{
	struct obs_nix_portal_shortcuts *portal = data;
	g_source_set_ready_time(portal->sync_source, 0);
}

static void unpublish(struct obs_nix_portal_shortcuts *portal)
{
	pthread_mutex_lock(&obs->hotkeys.mutex);
	if (obs->hotkeys.background.wake_data == portal) {
		obs->hotkeys.background.wake = NULL;
		obs->hotkeys.background.shortcut_trigger = NULL;
		obs->hotkeys.background.wake_data = NULL;
	}
	pthread_mutex_unlock(&obs->hotkeys.mutex);
}

static void free_portal(struct obs_nix_portal_shortcuts *portal)
{
	g_source_destroy(portal->sync_source);
	g_source_unref(portal->sync_source);
	g_main_context_unref(portal->context);
	g_free(portal->sender);
	g_free(portal->session_path);
	g_free(portal->request_error);
	da_free(portal->bound);
	free_descriptions(&portal->bound_descriptions);
	da_free(portal->declined);
	da_free(portal->approved);
	bfree(portal);
}

struct obs_nix_portal_shortcuts *obs_nix_portal_shortcuts_start(struct obs_nix_hotkey_state *state,
								obs_nix_portal_key_name_func key_name)
{
	struct obs_nix_portal_shortcuts *portal = bzalloc(sizeof(*portal));
	portal->state = state;
	portal->key_name = key_name;
	portal->context = g_main_context_new();

	portal->sync_source = g_source_new(&sync_source_funcs, sizeof(struct sync_source));
	((struct sync_source *)portal->sync_source)->portal = portal;
	g_source_attach(portal->sync_source, portal->context);

	struct obs_hotkeys_background *background = background_lock();
	background->state = OBS_HOTKEYS_BACKGROUND_STARTING;
	background->app_id = APP_ID;
	background->shortcut_id = obs_nix_portal_shortcut_id;
	background->shortcut_trigger = published_trigger;
	background->wake = wake_portal;
	background->wake_data = portal;
	background_unlock();

	connect_binding_signals(portal, true);
	if (pthread_create(&portal->thread, NULL, portal_thread, portal) != 0) {
		blog(LOG_WARNING, "[portal] Failed to start the portal shortcuts thread");
		connect_binding_signals(portal, false);
		unpublish(portal);
		set_state(OBS_HOTKEYS_BACKGROUND_UNAVAILABLE, NULL);
		free_portal(portal);
		return NULL;
	}
	return portal;
}

void obs_nix_portal_shortcuts_stop(struct obs_nix_portal_shortcuts *portal)
{
	if (!portal)
		return;

	/* No signal handler or wake runs after this returns. */
	connect_binding_signals(portal, false);
	unpublish(portal);

	os_atomic_set_bool(&portal->stopping, true);
	g_main_context_wakeup(portal->context);
	pthread_join(portal->thread, NULL);

	free_portal(portal);
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef PORTAL_NAME
#undef PORTAL_PATH
#undef SHORTCUTS_IFACE
#undef REQUEST_IFACE
#undef SESSION_IFACE
#undef REGISTRY_IFACE
#undef APP_ID
#undef SYNC_DELAY_MS
#undef FLUSH_INTERVAL_MS
#undef MAX_DESCRIBED_ACTIONS
