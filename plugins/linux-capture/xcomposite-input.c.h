#include "plugins/linux-capture/build/linux-capture-nix.pch.h"

#define WIN_STRING_DIV "\r\n"

static Display *disp = NULL;
static xcb_connection_t *xcomp_conn = NULL;
// Atoms used throughout our plugin
xcb_atom_t ATOM_UTF8_STRING;
xcb_atom_t ATOM_STRING;
xcb_atom_t ATOM_TEXT;
xcb_atom_t ATOM_COMPOUND_TEXT;
xcb_atom_t ATOM_WM_NAME;
xcb_atom_t ATOM_WM_CLASS;
xcb_atom_t ATOM__NET_WM_NAME;
xcb_atom_t ATOM__NET_SUPPORTING_WM_CHECK;
xcb_atom_t ATOM__NET_CLIENT_LIST;

struct xcompcap {
	obs_source_t *source;

	const char *windowName;
	xcb_window_t win;
	int crop_top;
	int crop_left;
	int crop_right;
	int crop_bot;
	bool include_border;
	bool exclude_alpha;

	struct xcomp_search search;
	struct xcomp_presence presence;
	bool window_changed;
	bool window_hooked;

	int width;
	int height;
	int border;

	Pixmap pixmap;
	gs_texture_t *gltex;

	pthread_mutex_t lock;

	bool show_cursor;
	bool cursor_outside;
	xcb_xcursor_t *cursor;
};

static void xcompcap_update(void *data, obs_data_t *settings);
static xcb_window_t convert_encoded_window_id(const char *str, char **p_wname, char **p_wcls);

xcb_atom_t get_atom(xcb_connection_t *xconn, const char *name);
xcb_atom_t get_atom(xcb_connection_t *xconn, const char *name)
{
	xcb_intern_atom_cookie_t atom_c = xcb_intern_atom(xconn, 1, (uint16_t)strlen(name), name);
	xcb_intern_atom_reply_t *atom_r = xcb_intern_atom_reply(xconn, atom_c, NULL);
	xcb_atom_t a = atom_r->atom;
	free(atom_r);
	return a;
}

void xcomp_gather_atoms(xcb_connection_t *xconn);
void xcomp_gather_atoms(xcb_connection_t *xconn)
{
	ATOM_UTF8_STRING = get_atom(xconn, "UTF8_STRING");
	ATOM_STRING = get_atom(xconn, "STRING");
	ATOM_TEXT = get_atom(xconn, "TEXT");
	ATOM_COMPOUND_TEXT = get_atom(xconn, "COMPOUND_TEXT");
	ATOM_WM_NAME = get_atom(xconn, "WM_NAME");
	ATOM_WM_CLASS = get_atom(xconn, "WM_CLASS");
	ATOM__NET_WM_NAME = get_atom(xconn, "_NET_WM_NAME");
	ATOM__NET_SUPPORTING_WM_CHECK = get_atom(xconn, "_NET_SUPPORTING_WM_CHECK");
	ATOM__NET_CLIENT_LIST = get_atom(xconn, "_NET_CLIENT_LIST");
}

bool xcomp_window_exists(xcb_connection_t *xconn, xcb_window_t win);
bool xcomp_window_exists(xcb_connection_t *xconn, xcb_window_t win)
{
	xcb_generic_error_t *err = NULL;
	xcb_get_window_attributes_cookie_t attr_cookie = xcb_get_window_attributes(xconn, win);
	xcb_get_window_attributes_reply_t *attr = xcb_get_window_attributes_reply(xconn, attr_cookie, &err);

	bool exists = err == NULL && attr->map_state == XCB_MAP_STATE_VIEWABLE;
	free(err);
	free(attr);
	return exists;
}

xcb_get_property_reply_t *xcomp_property_sync(xcb_connection_t *xconn, xcb_window_t win, xcb_atom_t atom);
xcb_get_property_reply_t *xcomp_property_sync(xcb_connection_t *xconn, xcb_window_t win, xcb_atom_t atom)
{
	if (atom == XCB_ATOM_NONE)
		return NULL;

	xcb_generic_error_t *err = NULL;
	// Read properties up to 4096*4 bytes
	xcb_get_property_cookie_t prop_cookie = xcb_get_property(xconn, 0, win, atom, 0, 0, 4096);
	xcb_get_property_reply_t *prop = xcb_get_property_reply(xconn, prop_cookie, &err);
	if (err != NULL || xcb_get_property_value_length(prop) == 0) {
		free(err);
		free(prop);
		return NULL;
	}

	return prop;
}

// See ICCCM https://www.x.org/releases/X11R7.6/doc/xorg-docs/specs/ICCCM/icccm.html#text_properties
// for more info on the galactic brained string types used in Xorg.

struct dstr xcomp_window_name(xcb_connection_t *xconn, Display *xdisp, xcb_window_t win);
struct dstr xcomp_window_name(xcb_connection_t *xconn, Display *xdisp, xcb_window_t win)
{
	struct dstr ret = {0};

	xcb_get_property_reply_t *name = xcomp_property_sync(xconn, win, ATOM__NET_WM_NAME);
	if (name) {
		// Guaranteed to be UTF8_STRING.
		const char *data = (const char *)xcb_get_property_value(name);
		dstr_ncopy(&ret, data, (size_t)xcb_get_property_value_length(name));
		free(name);
		return ret;
	}

	name = xcomp_property_sync(xconn, win, ATOM_WM_NAME);
	if (!name) {
		goto fail;
	}

	char *data = (char *)xcb_get_property_value(name);
	if (name->type == ATOM_UTF8_STRING) {
		dstr_ncopy(&ret, data, (size_t)xcb_get_property_value_length(name));
		free(name);
		return ret;
	}
	if (name->type == ATOM_STRING) { // Latin-1, safe enough
		dstr_ncopy(&ret, data, (size_t)xcb_get_property_value_length(name));
		free(name);
		return ret;
	}
	if (name->type == ATOM_TEXT) { // Default charset
		char *utf8;
		if (!os_mbs_to_utf8_ptr(data, (size_t)xcb_get_property_value_length(name), &utf8)) {
			free(name);
			goto fail;
		}
		free(name);
		dstr_init_move_array(&ret, utf8);
		return ret;
	}
	if (name->type == ATOM_COMPOUND_TEXT) { // LibX11 is the only decoder for these.
		XTextProperty xname = {
			(unsigned char *)data,
			name->type,
			8, // 8 by definition.
			1, // Only decode the first element of string arrays.
		};
		char **list;
		int len = 0;
		if (XmbTextPropertyToTextList(xdisp, &xname, &list, &len) < Success || !list || len < 1) {
			free(name);
			goto fail;
		}
		char *utf8;
		if (!os_mbs_to_utf8_ptr(list[0], 0, &utf8)) {
			XFreeStringList(list);
			free(name);
			goto fail;
		}
		dstr_init_move_array(&ret, utf8);
		XFreeStringList(list);
		free(name);
		return ret;
	}

fail:
	dstr_copy(&ret, "unknown");
	return ret;
}

struct dstr xcomp_window_class(xcb_connection_t *xconn, xcb_window_t win);
struct dstr xcomp_window_class(xcb_connection_t *xconn, xcb_window_t win)
{
	struct dstr ret = {0};
	dstr_copy(&ret, "unknown");
	xcb_get_property_reply_t *cls = xcomp_property_sync(xconn, win, ATOM_WM_CLASS);
	if (!cls)
		return ret;

	// WM_CLASS is formatted differently from other strings, it's two null terminated strings.
	// Since we want the first one, let's just let copy run strlen.
	dstr_copy(&ret, (const char *)xcb_get_property_value(cls));
	free(cls);
	return ret;
}

// Specification for checking for ewmh support at
// http://standards.freedesktop.org/wm-spec/wm-spec-latest.html#idm140200472693600

bool xcomp_check_ewmh(xcb_connection_t *xconn, xcb_window_t root);
bool xcomp_check_ewmh(xcb_connection_t *xconn, xcb_window_t root)
{
	xcb_get_property_reply_t *check = xcomp_property_sync(xconn, root, ATOM__NET_SUPPORTING_WM_CHECK);
	if (!check)
		return false;

	xcb_window_t ewmh_window = ((xcb_window_t *)xcb_get_property_value(check))[0];
	free(check);

	xcb_get_property_reply_t *check2 = xcomp_property_sync(xconn, ewmh_window, ATOM__NET_SUPPORTING_WM_CHECK);
	if (!check2)
		return false;
	free(check2);

	return true;
}

struct darray xcomp_top_level_windows(xcb_connection_t *xconn);
struct darray xcomp_top_level_windows(xcb_connection_t *xconn)
{
	DARRAY(xcb_window_t) res = {0};

	// EWMH top level window listing is not supported.
	if (ATOM__NET_CLIENT_LIST == XCB_ATOM_NONE)
		return res.da;

	xcb_screen_iterator_t screen_iter = xcb_setup_roots_iterator(xcb_get_setup(xconn));
	for (; screen_iter.rem > 0; xcb_screen_next(&screen_iter)) {
		xcb_generic_error_t *err = NULL;
		// Read properties up to 4096*4 bytes
		xcb_get_property_cookie_t cl_list_cookie =
			xcb_get_property(xconn, 0, screen_iter.data->root, ATOM__NET_CLIENT_LIST, 0, 0, 4096);
		xcb_get_property_reply_t *cl_list = xcb_get_property_reply(xconn, cl_list_cookie, &err);
		if (err != NULL) {
			goto done;
		}

		uint32_t len = (uint32_t)((size_t)xcb_get_property_value_length(cl_list) / sizeof(xcb_window_t));
		for (uint32_t i = 0; i < len; i++)
			da_push_back(res, &(((xcb_window_t *)xcb_get_property_value(cl_list))[i]));

	done:
		free(err);
		free(cl_list);
	}

	return res.da;
}

static xcb_window_t convert_encoded_window_id(const char *str, char **p_wname, char **p_wcls)
{
	size_t markSize = strlen(WIN_STRING_DIV);

	const char *firstMark = strstr(str, WIN_STRING_DIV);
	const char *secondMark = firstMark ? strstr(firstMark + markSize, WIN_STRING_DIV) : NULL;
	const char *strEnd = str + strlen(str);

	const char *secondStr = firstMark + markSize;
	const char *thirdStr = secondMark + markSize;

	// wstr only consists of the window-id
	if (!firstMark) {
		*p_wname = NULL;
		*p_wcls = NULL;
		return (xcb_window_t)atol(str);
	}

	// wstr also contains window-name and window-class
	char *wname = bzalloc((size_t)(secondMark - secondStr) + 1);
	char *wcls = bzalloc((size_t)(strEnd - thirdStr) + 1);
	memcpy(wname, secondStr, (size_t)(secondMark - secondStr));
	memcpy(wcls, thirdStr, (size_t)(strEnd - thirdStr));
	xcb_window_t ret = (xcb_window_t)strtol(str, NULL, 10);

	*p_wname = wname;
	*p_wcls = wcls;
	return ret;
}

xcb_window_t xcomp_find_window(xcb_connection_t *xconn, Display *xdisp, const char *str);
xcb_window_t xcomp_find_window(xcb_connection_t *xconn, Display *xdisp, const char *str)
{
	xcb_window_t ret = 0;
	char *wname = NULL;
	char *wcls = NULL;
	DARRAY(xcb_window_t) tlw = {0};

	if (!str || strlen(str) == 0) {
		goto cleanup;
	}

	ret = convert_encoded_window_id(str, &wname, &wcls);

	// first try to find a match by the window-id
	tlw.da = xcomp_top_level_windows(xconn);
	if (da_find(tlw, &ret, 0) != DARRAY_INVALID) {
		goto cleanup;
	}

	// then try to find a match by name & class
	for (size_t i = 0; i < tlw.num; i++) {
		xcb_window_t cwin = *(xcb_window_t *)darray_item(sizeof(xcb_window_t), &tlw.da, i);

		struct dstr cwname = xcomp_window_name(xconn, xdisp, cwin);
		struct dstr cwcls = xcomp_window_class(xconn, cwin);
		bool found = dstr_cmp(&cwname, wname) == 0 && dstr_cmp(&cwcls, wcls) == 0;

		dstr_free(&cwname);
		dstr_free(&cwcls);
		if (found) {
			ret = cwin;
			goto cleanup;
		}
	}

	blog(LOG_DEBUG, "Did not find new window id for Name '%s' or Class '%s'", wname, wcls);

cleanup:
	bfree(wname);
	bfree(wcls);
	da_free(tlw);
	return ret;
}

void xcomp_cleanup_pixmap(Display *xdisp, struct xcompcap *s);
void xcomp_cleanup_pixmap(Display *xdisp, struct xcompcap *s)
{
	if (s->gltex) {
		gs_texture_destroy(s->gltex);
		s->gltex = 0;
	}

	if (s->pixmap) {
		XFreePixmap(xdisp, s->pixmap);
		s->pixmap = 0;
	}
}

static int silence_x11_errors(Display *display, XErrorEvent *err)
{
	UNUSED_PARAMETER(display);
	UNUSED_PARAMETER(err);

	return 0;
}

void xcomp_create_pixmap(xcb_connection_t *xconn, struct xcompcap *s, int log_level);
void xcomp_create_pixmap(xcb_connection_t *xconn, struct xcompcap *s, int log_level)
{
	if (!s->win)
		return;

	xcb_generic_error_t *err = NULL;
	xcb_get_geometry_cookie_t geom_cookie = xcb_get_geometry(xconn, s->win);
	xcb_get_geometry_reply_t *geom = xcb_get_geometry_reply(xconn, geom_cookie, &err);
	if (err != NULL) {
		free(err);
		free(geom);
		return;
	}

	s->border = s->include_border ? geom->border_width : 0;
	s->width = geom->width;
	s->height = geom->height;
	// We don't have an alpha channel, but may have garbage in the texture.
	int32_t depth = geom->depth;
	if (depth != 32) {
		s->exclude_alpha = true;
	}
	free(geom);

	int vert_borders = s->crop_top + s->crop_bot + 2 * s->border;
	int hori_borders = s->crop_left + s->crop_right + 2 * s->border;
	// Skip 0 sized textures.
	if (vert_borders > s->height || hori_borders > s->width)
		return;

	s->pixmap = xcb_generate_id(xconn);
	xcb_void_cookie_t name_cookie =
		xcb_composite_name_window_pixmap_checked(xconn, s->win, (xcb_pixmap_t)s->pixmap);
	err = NULL;
	if ((err = xcb_request_check(xconn, name_cookie)) != NULL) {
		blog(log_level, "xcb_composite_name_window_pixmap failed");
		s->pixmap = 0;
		return;
	}

	XErrorHandler prev = XSetErrorHandler(silence_x11_errors);
	s->gltex = gs_texture_create_from_pixmap((uint32_t)s->width, (uint32_t)s->height, GS_BGRA_UNORM, GL_TEXTURE_2D,
						 (void *)s->pixmap);
	XSetErrorHandler(prev);
}

struct reg_item {
	struct xcompcap *src;
	xcb_window_t win;
};
static DARRAY(struct reg_item) watcher_registry = {0};
static pthread_mutex_t watcher_lock = PTHREAD_MUTEX_INITIALIZER;

void watcher_register(xcb_connection_t *xconn, struct xcompcap *s);
void watcher_register(xcb_connection_t *xconn, struct xcompcap *s)
{
	pthread_mutex_lock(&watcher_lock);

	if (xcomp_window_exists(xconn, s->win)) {
		// Subscribe to Events
		uint32_t vals[1] = {StructureNotifyMask | ExposureMask | VisibilityChangeMask};
		xcb_change_window_attributes(xconn, s->win, XCB_CW_EVENT_MASK, vals);
		xcb_composite_redirect_window(xconn, s->win, XCB_COMPOSITE_REDIRECT_AUTOMATIC);

		da_push_back(watcher_registry, (&(struct reg_item){s, s->win}));
	}

	pthread_mutex_unlock(&watcher_lock);
}

void watcher_unregister(xcb_connection_t *xconn, struct xcompcap *s);
void watcher_unregister(xcb_connection_t *xconn, struct xcompcap *s)
{
	pthread_mutex_lock(&watcher_lock);
	size_t idx = DARRAY_INVALID;
	xcb_window_t win = 0;
	for (size_t i = 0; i < watcher_registry.num; i++) {
		struct reg_item *item =
			(struct reg_item *)darray_item(sizeof(struct reg_item), &watcher_registry.da, i);

		if (item->src == s) {
			idx = i;
			win = item->win;
			break;
		}
	}
	if (idx == DARRAY_INVALID)
		goto done;

	da_erase(watcher_registry, idx);

	// Check if there are still sources listening for the same window.
	bool windowInUse = false;
	for (size_t i = 0; i < watcher_registry.num; i++) {
		struct reg_item *item =
			(struct reg_item *)darray_item(sizeof(struct reg_item), &watcher_registry.da, i);

		if (item->win == win) {
			windowInUse = true;
			break;
		}
	}

	if (!windowInUse && xcomp_window_exists(xconn, s->win)) {
		// Last source released, stop listening for events.
		uint32_t vals[1] = {0};
		xcb_change_window_attributes(xconn, win, XCB_CW_EVENT_MASK, vals);
	}

done:
	pthread_mutex_unlock(&watcher_lock);
}

void watcher_process(xcb_generic_event_t *ev);
void watcher_process(xcb_generic_event_t *ev)
{
	if (!ev)
		return;

	pthread_mutex_lock(&watcher_lock);
	xcb_window_t win = 0;
	bool changed = true;
	enum xcomp_presence_event presence = XCOMP_PRESENCE_OTHER;

	switch (ev->response_type & ~0x80) {
	case XCB_CONFIGURE_NOTIFY:
		win = ((xcb_configure_notify_event_t *)ev)->event;
		break;
	case XCB_MAP_NOTIFY:
		win = ((xcb_map_notify_event_t *)ev)->event;
		presence = XCOMP_PRESENCE_MAPPED;
		break;
	case XCB_UNMAP_NOTIFY:
		// Only the cached state changes; the tick's lost-window search
		// handles it like before (#1356).
		win = ((xcb_unmap_notify_event_t *)ev)->event;
		presence = XCOMP_PRESENCE_UNMAPPED;
		changed = false;
		break;
	case XCB_EXPOSE:
		win = ((xcb_expose_event_t *)ev)->window;
		break;
	case XCB_VISIBILITY_NOTIFY:
		win = ((xcb_visibility_notify_event_t *)ev)->window;
		break;
	case XCB_DESTROY_NOTIFY:
		win = ((xcb_destroy_notify_event_t *)ev)->event;
		presence = XCOMP_PRESENCE_DESTROYED;
		break;
	};

	if (win != 0) {
		for (size_t i = 0; i < watcher_registry.num; i++) {
			struct reg_item *item =
				(struct reg_item *)darray_item(sizeof(struct reg_item), &watcher_registry.da, i);
			if (item->win == win) {
				if (changed)
					item->src->window_changed = true;
				xcomp_presence_event(&item->src->presence, presence);
			}
		}
	}

	pthread_mutex_unlock(&watcher_lock);
}

void watcher_unload(void);
void watcher_unload(void)
{
	da_free(watcher_registry);
}

static void xcompcap_get_hooked(void *data, calldata_t *cd)
{
	struct xcompcap *s = data;
	if (!s)
		return;

	calldata_set_bool(cd, "hooked", s->window_hooked);

	if (s->window_hooked) {
		struct dstr wname = xcomp_window_name(xcomp_conn, disp, s->win);
		struct dstr wcls = xcomp_window_class(xcomp_conn, s->win);
		calldata_set_string(cd, "name", wname.array);
		calldata_set_string(cd, "class", wcls.array);
		dstr_free(&wname);
		dstr_free(&wcls);
	} else {
		calldata_set_string(cd, "name", "");
		calldata_set_string(cd, "class", "");
	}
}

static uint32_t xcompcap_get_width(void *data)
{
	struct xcompcap *s = (struct xcompcap *)data;
	if (!s->gltex)
		return 0;

	int border = s->crop_left + s->crop_right + 2 * s->border;
	int width = s->width - border;
	return width < 0 ? 0 : (uint32_t)width;
}

static uint32_t xcompcap_get_height(void *data)
{
	struct xcompcap *s = (struct xcompcap *)data;
	if (!s->gltex)
		return 0;

	int border = s->crop_bot + s->crop_top + 2 * s->border;
	int height = s->height - border;
	return height < 0 ? 0 : (uint32_t)height;
}

static void *xcompcap_create(obs_data_t *settings, obs_source_t *source)
{
	struct xcompcap *s = (struct xcompcap *)bzalloc(sizeof(struct xcompcap));
	pthread_mutex_init(&s->lock, NULL);
	s->show_cursor = true;
	s->source = source;
	s->window_hooked = false;

	obs_enter_graphics();
	s->cursor = xcb_xcursor_init(xcomp_conn);
	obs_leave_graphics();

	signal_handler_t *sh = obs_source_get_signal_handler(source);
	signal_handler_add(sh, "void unhooked(ptr source)");
	signal_handler_add(sh, "void hooked(ptr source, string name, string class)");

	proc_handler_t *ph = obs_source_get_proc_handler(source);
	proc_handler_add(ph, "void get_hooked(out bool hooked, out string name, out string class)", xcompcap_get_hooked,
			 s);

	xcompcap_update(s, settings);
	return s;
}

static void xcompcap_destroy(void *data)
{
	struct xcompcap *s = (struct xcompcap *)data;

	obs_enter_graphics();
	pthread_mutex_lock(&s->lock);

	watcher_unregister(xcomp_conn, s);
	xcomp_cleanup_pixmap(disp, s);

	if (s->cursor)
		xcb_xcursor_destroy(s->cursor);

	pthread_mutex_unlock(&s->lock);
	obs_leave_graphics();

	pthread_mutex_destroy(&s->lock);
	bfree(s);
}

static void xcompcap_video_tick(void *data, float seconds)
{
	struct xcompcap *s = (struct xcompcap *)data;

	if (!obs_source_showing(s->source))
		return;

	// Whether the window is viewable comes from the X events the watcher
	// receives (Map/Unmap/DestroyNotify), not a blocking round trip per
	// frame under obs_enter_graphics (#1356). xcb_poll_for_event reads the
	// socket without blocking; before, only the round trips pulled events
	// in. A real check still runs after window_changed and at most once a
	// second in case an event was missed, outside the graphics context.
	pthread_mutex_lock(&s->lock);
	xcb_generic_event_t *event;
	while ((event = xcb_poll_for_event(xcomp_conn))) {
		watcher_process(event);
		free(event);
	}
	if (s->window_changed)
		xcomp_presence_invalidate(&s->presence);
	if (xcomp_presence_check_due(&s->presence, seconds))
		xcomp_presence_set(&s->presence, xcomp_window_exists(xcomp_conn, s->win));
	bool window_viewable = s->presence.viewable;
	pthread_mutex_unlock(&s->lock);

	obs_enter_graphics();
	pthread_mutex_lock(&s->lock);

	// Send an unhooked signal if the window isn't found anymore
	if (s->window_hooked && !window_viewable) {
		s->window_hooked = false;

		signal_handler_t *sh = obs_source_get_signal_handler(s->source);
		calldata_t signal_data = {0};
		calldata_set_ptr(&signal_data, "source", s->source);
		signal_handler_signal(sh, "unhooked", &signal_data);
		calldata_free(&signal_data);
	}

	// Reacquire window after a backoff interval or immediately if
	// reconfigured. A window that stays unmapped (minimized, other
	// workspace) is searched less and less often (#1357); a MapNotify on it
	// sets window_changed through the watcher.
	bool window_lost = !window_viewable || !s->gltex;
	bool forced = s->window_changed;
	if (xcomp_search_due(&s->search, seconds, window_lost, forced)) {
		s->window_changed = false;
		xcb_window_t found = xcomp_find_window(xcomp_conn, disp, s->windowName);
		bool viewable = xcomp_window_exists(xcomp_conn, found);

		if (xcomp_search_needs_rebuild(s->win, found, viewable, forced)) {
			watcher_unregister(xcomp_conn, s);
			s->win = found;

			// Send a hooked signal if a new window has been found
			if (!s->window_hooked && viewable) {
				s->window_hooked = true;

				signal_handler_t *sh = obs_source_get_signal_handler(s->source);
				calldata_t signal_data = {0};
				calldata_set_ptr(&signal_data, "source", s->source);
				struct dstr wname = xcomp_window_name(xcomp_conn, disp, s->win);
				struct dstr wcls = xcomp_window_class(xcomp_conn, s->win);
				calldata_set_string(&signal_data, "name", wname.array);
				calldata_set_string(&signal_data, "class", wcls.array);
				signal_handler_signal(sh, "hooked", &signal_data);
				dstr_free(&wname);
				dstr_free(&wcls);
				calldata_free(&signal_data);
			}

			watcher_register(xcomp_conn, s);
			xcomp_cleanup_pixmap(disp, s);
			// Avoid excessive logging. We expect this to fail while windows are
			// minimized or on offscreen workspaces or already captured on NVIDIA.
			xcomp_create_pixmap(xcomp_conn, s, LOG_DEBUG);
			xcb_xcursor_offset_win(xcomp_conn, s->cursor, s->win);
			xcb_xcursor_offset(s->cursor, s->cursor->x_org + s->crop_left, s->cursor->y_org + s->crop_top);
		}

		if (found == s->win)
			xcomp_presence_set(&s->presence, viewable);
		xcomp_search_done(&s->search, s->gltex != NULL, forced);
	}

	if (!s->gltex)
		goto done;

	if (xcompcap_get_height(s) == 0 || xcompcap_get_width(s) == 0)
		goto done;

	if (s->show_cursor) {
		xcb_xcursor_update(xcomp_conn, s->cursor);

		s->cursor_outside = s->cursor->x < 0 || s->cursor->y < 0 || s->cursor->x > (int)xcompcap_get_width(s) ||
				    s->cursor->y > (int)xcompcap_get_height(s);
	}

done:
	pthread_mutex_unlock(&s->lock);
	obs_leave_graphics();
}

static void xcompcap_video_render(void *data, gs_effect_t *effect)
{
	gs_eparam_t *image; // Placate C++ goto rules.
	struct xcompcap *s = (struct xcompcap *)data;

	pthread_mutex_lock(&s->lock);

	if (!s->gltex)
		goto done;

	effect = obs_get_base_effect(OBS_EFFECT_DEFAULT);
	if (s->exclude_alpha)
		effect = obs_get_base_effect(OBS_EFFECT_OPAQUE);

	image = gs_effect_get_param_by_name(effect, "image");
	gs_effect_set_texture(image, s->gltex);

	while (gs_effect_loop(effect, "Draw")) {
		gs_draw_sprite_subregion(s->gltex, 0, (uint32_t)s->crop_left, (uint32_t)s->crop_top,
					 xcompcap_get_width(s), xcompcap_get_height(s));
	}

	if (s->gltex && s->show_cursor && !s->cursor_outside) {
		effect = obs_get_base_effect(OBS_EFFECT_DEFAULT);

		while (gs_effect_loop(effect, "Draw")) {
			xcb_xcursor_render(s->cursor);
		}
	}

done:
	pthread_mutex_unlock(&s->lock);
}

struct WindowInfo {
	struct dstr name_lower;
	struct dstr name;
	struct dstr desc;
};

static int cmp_wi(const void *a, const void *b)
{
	const struct WindowInfo *awi = (const struct WindowInfo *)a;
	const struct WindowInfo *bwi = (const struct WindowInfo *)b;
	const char *a_name = awi->name_lower.array ? awi->name_lower.array : "";
	const char *b_name = bwi->name_lower.array ? bwi->name_lower.array : "";
	return strcmp(a_name, b_name);
}

static inline bool compare_ids(const char *id1, const char *id2)
{
	if (!id1 || !id2)
		return false;

	id1 = strstr(id1, "\r\n");
	id2 = strstr(id2, "\r\n");

	return id1 && id2 && strcmp(id1, id2) == 0;
}

static obs_properties_t *xcompcap_props(void *data)
{
	struct xcompcap *s = (struct xcompcap *)data;

	obs_properties_t *props = obs_properties_create();
	obs_property_t *prop;

	prop = obs_properties_add_list(props, "capture_window", obs_module_text("Window"), OBS_COMBO_TYPE_LIST,
				       OBS_COMBO_FORMAT_STRING);

	DARRAY(struct WindowInfo) window_strings = {0};
	bool had_window_saved = false;

	if (s) {
		had_window_saved = s->windowName && *s->windowName;
		if (had_window_saved) {
			char *wname;
			char *wcls;

			convert_encoded_window_id(s->windowName, &wname, &wcls);
			bfree(wcls);

			struct dstr name = {0};
			struct dstr desc = {0};
			struct dstr name_lower = {0};

			dstr_copy(&name, wname ? wname : obs_module_text("UnknownWindow"));
			bfree(wname);

			dstr_copy(&desc, s->windowName);
			dstr_copy_dstr(&name_lower, &name);
			dstr_to_lower(&name_lower);

			da_push_back(window_strings, (&(struct WindowInfo){name_lower, name, desc}));
		} else {
			struct dstr select_window_str;
			dstr_init_copy(&select_window_str, obs_module_text("SelectAWindow"));
			da_push_back(window_strings, (&(struct WindowInfo){{0}, select_window_str, {0}}));
		}
	}

	struct darray windows = xcomp_top_level_windows(xcomp_conn);
	bool window_found = false;

	for (size_t w = 0; w < windows.num; w++) {
		xcb_window_t win = *(xcb_window_t *)darray_item(sizeof(xcb_window_t), &windows, w);

		struct dstr name = xcomp_window_name(xcomp_conn, disp, win);
		struct dstr cls = xcomp_window_class(xcomp_conn, win);

		struct dstr desc = {0};
		dstr_printf(&desc, "%d" WIN_STRING_DIV "%s" WIN_STRING_DIV "%s", win, name.array, cls.array);
		dstr_free(&cls);

		struct dstr name_lower;
		dstr_init_copy_dstr(&name_lower, &name);
		dstr_to_lower(&name_lower);

		if (!had_window_saved || (s && !compare_ids(desc.array, s->windowName)))
			da_push_back(window_strings, (&(struct WindowInfo){name_lower, name, desc}));
		else {
			window_found = true;
			dstr_free(&name);
			dstr_free(&name_lower);
			dstr_free(&desc);
		}
	}
	darray_free(&windows);

	if (window_strings.num > 2)
		qsort(window_strings.array + 1, window_strings.num - 1, sizeof(struct WindowInfo), cmp_wi);

	for (size_t i = 0; i < window_strings.num; i++) {
		struct WindowInfo *w =
			(struct WindowInfo *)darray_item(sizeof(struct WindowInfo), &window_strings.da, i);

		obs_property_list_add_string(prop, w->name.array, w->desc.array);

		dstr_free(&w->name_lower);
		dstr_free(&w->name);
		dstr_free(&w->desc);
	}
	da_free(window_strings);

	if (!had_window_saved || !window_found) {
		obs_property_list_item_disable(prop, 0, true);
	}

	prop = obs_properties_add_int(props, "cut_top", obs_module_text("CropTop"), 0, 4096, 1);
	obs_property_int_set_suffix(prop, " px");

	prop = obs_properties_add_int(props, "cut_left", obs_module_text("CropLeft"), 0, 4096, 1);
	obs_property_int_set_suffix(prop, " px");

	prop = obs_properties_add_int(props, "cut_right", obs_module_text("CropRight"), 0, 4096, 1);
	obs_property_int_set_suffix(prop, " px");

	prop = obs_properties_add_int(props, "cut_bot", obs_module_text("CropBottom"), 0, 4096, 1);
	obs_property_int_set_suffix(prop, " px");

	obs_properties_add_bool(props, "show_cursor", obs_module_text("CaptureCursor"));

	obs_properties_add_bool(props, "include_border", obs_module_text("IncludeXBorder"));

	obs_properties_add_bool(props, "exclude_alpha", obs_module_text("ExcludeAlpha"));

	return props;
}

static void xcompcap_defaults(obs_data_t *settings)
{
	obs_data_set_default_string(settings, "capture_window", "");
	obs_data_set_default_int(settings, "cut_top", 0);
	obs_data_set_default_int(settings, "cut_left", 0);
	obs_data_set_default_int(settings, "cut_right", 0);
	obs_data_set_default_int(settings, "cut_bot", 0);
	obs_data_set_default_bool(settings, "show_cursor", true);
	obs_data_set_default_bool(settings, "include_border", false);
	obs_data_set_default_bool(settings, "exclude_alpha", false);
}

static void xcompcap_update(void *data, obs_data_t *settings)
{
	struct xcompcap *s = (struct xcompcap *)data;

	obs_enter_graphics();
	pthread_mutex_lock(&s->lock);
	char *prev_name = bstrdup(s->windowName);

	s->crop_top = (int)obs_data_get_int(settings, "cut_top");
	s->crop_left = (int)obs_data_get_int(settings, "cut_left");
	s->crop_right = (int)obs_data_get_int(settings, "cut_right");
	s->crop_bot = (int)obs_data_get_int(settings, "cut_bot");
	s->show_cursor = obs_data_get_bool(settings, "show_cursor");
	s->include_border = obs_data_get_bool(settings, "include_border");
	s->exclude_alpha = obs_data_get_bool(settings, "exclude_alpha");

	s->windowName = obs_data_get_string(settings, "capture_window");
	if (s->window_hooked && strcmp(prev_name, s->windowName) != 0) {
		s->window_hooked = false;

		signal_handler_t *sh = obs_source_get_signal_handler(s->source);
		calldata_t signal_data = {0};
		calldata_set_ptr(&signal_data, "source", s->source);
		signal_handler_signal(sh, "unhooked", &signal_data);
		calldata_free(&signal_data);
	}
	bfree(prev_name);

	s->win = xcomp_find_window(xcomp_conn, disp, s->windowName);
	if (!s->window_hooked && xcomp_window_exists(xcomp_conn, s->win)) {
		s->window_hooked = true;

		signal_handler_t *sh = obs_source_get_signal_handler(s->source);
		calldata_t signal_data = {0};
		calldata_set_ptr(&signal_data, "source", s->source);
		struct dstr wname = xcomp_window_name(xcomp_conn, disp, s->win);
		struct dstr wcls = xcomp_window_class(xcomp_conn, s->win);
		calldata_set_string(&signal_data, "name", wname.array);
		calldata_set_string(&signal_data, "class", wcls.array);
		signal_handler_signal(sh, "hooked", &signal_data);
		dstr_free(&wname);
		dstr_free(&wcls);
		calldata_free(&signal_data);
	}

	if (s->win && s->windowName) {
		struct dstr wname = xcomp_window_name(xcomp_conn, disp, s->win);
		struct dstr wcls = xcomp_window_class(xcomp_conn, s->win);
		blog(LOG_INFO,
		     "[window-capture: '%s'] update settings:\n"
		     "\ttitle: %s\n"
		     "\tclass: %s\n",
		     obs_source_get_name(s->source), wname.array, wcls.array);
		dstr_free(&wname);
		dstr_free(&wcls);
	}

	watcher_register(xcomp_conn, s);
	xcomp_cleanup_pixmap(disp, s);
	xcomp_create_pixmap(xcomp_conn, s, LOG_ERROR);
	xcomp_search_reset(&s->search);
	xcomp_presence_invalidate(&s->presence);
	xcb_xcursor_offset_win(xcomp_conn, s->cursor, s->win);
	xcb_xcursor_offset(s->cursor, s->cursor->x_org + s->crop_left, s->cursor->y_org + s->crop_top);

	pthread_mutex_unlock(&s->lock);
	obs_leave_graphics();
}

static const char *xcompcap_getname(void *data)
{
	UNUSED_PARAMETER(data);
	return obs_module_text("XCCapture");
}

void xcomposite_load(void)
{
	disp = XOpenDisplay(NULL);
	xcomp_conn = XGetXCBConnection(disp);
	if (xcb_connection_has_error(xcomp_conn)) {
		blog(LOG_ERROR, "failed opening display");
		return;
	}

	const xcb_query_extension_reply_t *xcomp_ext = xcb_get_extension_data(xcomp_conn, &xcb_composite_id);
	if (!xcomp_ext->present) {
		blog(LOG_ERROR, "Xcomposite extension not supported");
		return;
	}

	xcb_composite_query_version_cookie_t version_cookie = xcb_composite_query_version(xcomp_conn, 0, 2);
	xcb_composite_query_version_reply_t *version =
		xcb_composite_query_version_reply(xcomp_conn, version_cookie, NULL);
	if (version->major_version == 0 && version->minor_version < 2) {
		blog(LOG_ERROR, "Xcomposite extension is too old: %d.%d < 0.2", version->major_version,
		     version->minor_version);
		free(version);
		return;
	}
	free(version);

	// Must be done before other helpers called.
	xcomp_gather_atoms(xcomp_conn);

	xcb_screen_t *screen_default = xcb_get_screen(xcomp_conn, DefaultScreen(disp));
	if (!screen_default || !xcomp_check_ewmh(xcomp_conn, screen_default->root)) {
		blog(LOG_ERROR,
		     "window manager does not support Extended Window Manager Hints (EWMH).\nXComposite capture disabled.");
		return;
	}

	struct obs_source_info sinfo = {
		.id = "xcomposite_input",
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW | OBS_SOURCE_DO_NOT_DUPLICATE,
		.get_name = xcompcap_getname,
		.create = xcompcap_create,
		.destroy = xcompcap_destroy,
		.get_properties = xcompcap_props,
		.get_defaults = xcompcap_defaults,
		.update = xcompcap_update,
		.video_tick = xcompcap_video_tick,
		.video_render = xcompcap_video_render,
		.get_width = xcompcap_get_width,
		.get_height = xcompcap_get_height,
		.icon_type = OBS_ICON_TYPE_WINDOW_CAPTURE,
	};

	obs_register_source(&sinfo);
}

void xcomposite_unload(void)
{
	XCloseDisplay(disp);
	disp = NULL;
	xcomp_conn = NULL;
	watcher_unload();
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef WIN_STRING_DIV
