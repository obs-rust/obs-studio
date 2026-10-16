/* VST 2.x filter: closing the plug-in editor with its window's own close
 * button must put the filter's properties back to "Open Plug-in Interface"
 * (obsproject/obs-studio#13580). The Filters dialog only rebuilds its view
 * when a button callback returns true or when the filter sends
 * "update_properties"; a window-manager close did neither, so the view kept
 * "Close Plug-in Interface", which did nothing, and the editor could not be
 * opened again until the dialog was reopened.
 *
 * Loads the real obs-vst module and a small VST 2.x plug-in built next to
 * this test (vst_editor_close_plugin.cpp). The "view" below holds its
 * obs_properties_t the way OBSPropertiesView does and rebuilds it when the
 * filter sends "update_properties". A window-manager close is a QCloseEvent
 * delivered to the editor's window, as the platform plug-in does for
 * WM_DELETE_WINDOW. The other ways the editor closes (its Close button,
 * removing the filter, removing the source) must close it exactly once and
 * must not rebuild the view while a button callback is still using it.
 * Qt's offscreen platform; obs_startup needs a display, so the cases run
 * under xvfb-run and skip without one. */

#include <QApplication>
#include <QCloseEvent>
#include <QElapsedTimer>
#include <QWidget>
#include <QWindow>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

#include <obs.h>
#include <util/platform.h>

#define PARENT_ID "test_vst_parent"
#define OPEN_BUTTON "open_vst_settings"
#define CLOSE_BUTTON "close_vst_settings"

static bool obs_up;
static void *plugin_handle;
static void (*editor_counts)(int *opens, int *closes);

/* ---------------------------------------------------------------- view */

struct View {
	obs_source_t *filter = nullptr;
	obs_properties_t *props = nullptr;
	bool in_click = false;
	int updates = 0;
	int updates_inside_click = 0;
};

static void reload(View *v)
{
	obs_properties_destroy(v->props);
	v->props = obs_source_properties(v->filter);
}

static void on_update_properties(void *data, calldata_t *)
{
	View *v = static_cast<View *>(data);
	v->updates++;
	if (v->in_click) {
		/* OBSPropertiesView would free the properties the button
		 * callback is still using. Count it instead. */
		v->updates_inside_click++;
		return;
	}
	reload(v);
}

static void view_attach(View *v, obs_source_t *filter)
{
	v->filter = filter;
	v->props = obs_source_properties(filter);
	signal_handler_connect(obs_source_get_signal_handler(filter), "update_properties", on_update_properties, v);
}

static void view_detach(View *v)
{
	signal_handler_disconnect(obs_source_get_signal_handler(v->filter), "update_properties", on_update_properties,
				  v);
	obs_properties_destroy(v->props);
	v->props = nullptr;
}

static bool visible(View *v, const char *name)
{
	obs_property_t *p = obs_properties_get(v->props, name);
	return p && obs_property_visible(p);
}

/* Like WidgetInfo::ButtonClicked: run the callback, and when it asks for a
 * refresh, rebuild the widgets from the same properties object. */
static void click(View *v, const char *name)
{
	obs_property_t *p = obs_properties_get(v->props, name);
	assert_non_null(p);
	assert_true(obs_property_visible(p));
	v->in_click = true;
	obs_property_button_clicked(p, v->filter);
	v->in_click = false;
}

/* ------------------------------------------------------------- helpers */

static void pump(int ms = 200)
{
	QElapsedTimer t;
	t.start();
	while (t.elapsed() < ms) {
		QCoreApplication::processEvents(QEventLoop::AllEvents, 20);
		QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
	}
}

static QWidget *editor_window()
{
	for (QWidget *w : QApplication::topLevelWidgets()) {
		if (w->isVisible() && w->windowTitle().contains("Editor Close Test")) {
			return w;
		}
	}
	return nullptr;
}

/* What the platform plug-in delivers when the window manager closes the
 * window (WM_DELETE_WINDOW on X11, the red X on Windows/macOS). */
static void wm_close(QWidget *w)
{
	QWindow *window = w->windowHandle();
	assert_non_null(window);
	QCloseEvent ev;
	QCoreApplication::sendEvent(window, &ev);
}

static void counts(int *opens, int *closes)
{
	editor_counts(opens, closes);
}

static const char *parent_name(void *)
{
	return "VST test parent";
}

static void *parent_create(obs_data_t *, obs_source_t *)
{
	return bzalloc(1);
}

static void parent_destroy(void *data)
{
	bfree(data);
}

static obs_source_t *create_parent(void)
{
	return obs_source_create_private(PARENT_ID, "vst parent", nullptr);
}

static obs_source_t *create_filter(obs_source_t *parent)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_string(settings, "plugin_path", VST_TEST_PLUGIN_PATH);
	obs_source_t *filter = obs_source_create_private("vst_filter", "VST 2.x Plug-in", settings);
	obs_data_release(settings);
	assert_non_null(filter);
	obs_source_filter_add(parent, filter);
	return filter;
}

static void wait_destroyed(void)
{
	obs_wait_for_destroy_queue();
	pump();
	obs_wait_for_destroy_queue();
	pump();
}

/* --------------------------------------------------------------- cases */

/* The bug: open with the button, close with the window's own close button. */
static void wm_close_shows_open_button_again(void **)
{
	if (!obs_up) {
		skip();
	}

	obs_source_t *parent = create_parent();
	obs_source_t *filter = create_filter(parent);
	View v;
	view_attach(&v, filter);
	assert_true(visible(&v, OPEN_BUTTON));
	assert_false(visible(&v, CLOSE_BUTTON));

	int opens0, closes0, opens, closes;
	counts(&opens0, &closes0);

	click(&v, OPEN_BUTTON);
	pump();
	QWidget *editor = editor_window();
	assert_non_null(editor);
	counts(&opens, &closes);
	assert_int_equal(opens, opens0 + 1);
	assert_false(visible(&v, OPEN_BUTTON));
	assert_true(visible(&v, CLOSE_BUTTON));

	wm_close(editor);
	pump();
	assert_null(editor_window());
	counts(&opens, &closes);
	assert_int_equal(closes, closes0 + 1);

	/* The view must have been told, and now offer to open it again. */
	assert_true(v.updates >= 1);
	assert_true(visible(&v, OPEN_BUTTON));
	assert_false(visible(&v, CLOSE_BUTTON));

	/* And opening it again works without reopening the dialog. */
	click(&v, OPEN_BUTTON);
	pump();
	editor = editor_window();
	assert_non_null(editor);
	counts(&opens, &closes);
	assert_int_equal(opens, opens0 + 2);

	wm_close(editor);
	pump();
	assert_null(editor_window());
	assert_true(visible(&v, OPEN_BUTTON));
	assert_false(visible(&v, CLOSE_BUTTON));
	counts(&opens, &closes);
	assert_int_equal(closes, closes0 + 2);

	view_detach(&v);
	obs_source_release(filter);
	obs_source_release(parent);
	wait_destroyed();
}

/* The dialog's own Close button keeps working: one effEditClose, and the
 * view is not rebuilt while the button callback still uses it. */
static void close_button_closes_once(void **)
{
	if (!obs_up) {
		skip();
	}

	obs_source_t *parent = create_parent();
	obs_source_t *filter = create_filter(parent);
	View v;
	view_attach(&v, filter);

	int opens0, closes0, opens, closes;
	counts(&opens0, &closes0);

	click(&v, OPEN_BUTTON);
	pump();
	assert_non_null(editor_window());

	click(&v, CLOSE_BUTTON);
	assert_int_equal(v.updates_inside_click, 0);
	pump();
	assert_null(editor_window());
	assert_int_equal(v.updates_inside_click, 0);
	assert_true(visible(&v, OPEN_BUTTON));
	assert_false(visible(&v, CLOSE_BUTTON));
	counts(&opens, &closes);
	assert_int_equal(opens, opens0 + 1);
	assert_int_equal(closes, closes0 + 1);

	view_detach(&v);
	obs_source_release(filter);
	obs_source_release(parent);
	wait_destroyed();
	counts(&opens, &closes);
	assert_int_equal(closes, closes0 + 1);
}

/* Removing the filter while its editor is open closes the editor once,
 * after the filter's source is gone. */
static void filter_removed_with_editor_open(void **)
{
	if (!obs_up) {
		skip();
	}

	obs_source_t *parent = create_parent();
	obs_source_t *filter = create_filter(parent);
	View v;
	view_attach(&v, filter);

	int opens0, closes0, opens, closes;
	counts(&opens0, &closes0);

	click(&v, OPEN_BUTTON);
	pump();
	assert_non_null(editor_window());

	view_detach(&v);
	obs_source_filter_remove(parent, filter);
	obs_source_release(filter);
	wait_destroyed();

	assert_null(editor_window());
	counts(&opens, &closes);
	assert_int_equal(opens, opens0 + 1);
	assert_int_equal(closes, closes0 + 1);

	obs_source_release(parent);
	wait_destroyed();
}

/* Removing the source the filter is on, editor open: same. */
static void source_removed_with_editor_open(void **)
{
	if (!obs_up) {
		skip();
	}

	obs_source_t *parent = create_parent();
	obs_source_t *filter = create_filter(parent);
	View v;
	view_attach(&v, filter);

	int opens0, closes0, opens, closes;
	counts(&opens0, &closes0);

	click(&v, OPEN_BUTTON);
	pump();
	assert_non_null(editor_window());

	view_detach(&v);
	obs_source_release(filter);
	obs_source_remove(parent);
	obs_source_release(parent);
	wait_destroyed();

	assert_null(editor_window());
	counts(&opens, &closes);
	assert_int_equal(opens, opens0 + 1);
	assert_int_equal(closes, closes0 + 1);
}

/* ---------------------------------------------------------------- main */

static void quiet_log(int, const char *, va_list, void *) {}

static int setup(void **)
{
	base_set_log_handler(quiet_log, nullptr);
	obs_up = obs_startup("en-US", nullptr, nullptr);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	obs_audio_info oai = {};
	oai.samples_per_sec = 48000;
	oai.speakers = SPEAKERS_STEREO;
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "skipped: obs_reset_audio failed\n");
		obs_up = false;
		return 0;
	}

	obs_source_info parent = {};
	parent.id = PARENT_ID;
	parent.type = OBS_SOURCE_TYPE_INPUT;
	parent.output_flags = OBS_SOURCE_AUDIO;
	parent.get_name = parent_name;
	parent.create = parent_create;
	parent.destroy = parent_destroy;
	obs_register_source(&parent);

	obs_module_t *module = nullptr;
	if (obs_open_module(&module, VST_MODULE_PATH, VST_DATA_PATH) != MODULE_SUCCESS || !obs_init_module(module)) {
		fprintf(stderr, "could not load obs-vst\n");
		return -1;
	}

	/* The same library obs-vst loads; it counts effEditOpen/effEditClose. */
	plugin_handle = os_dlopen(VST_TEST_PLUGIN_PATH);
	if (!plugin_handle) {
		return -1;
	}
	editor_counts = (void (*)(int *, int *))os_dlsym(plugin_handle, "vst_test_editor_counts");
	return editor_counts ? 0 : -1;
}

static int teardown(void **)
{
	if (obs_up) {
		pump();
		obs_shutdown();
	}
	if (plugin_handle) {
		os_dlclose(plugin_handle);
	}
	return 0;
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(wm_close_shows_open_button_again),
		cmocka_unit_test(close_button_closes_once),
		cmocka_unit_test(filter_removed_with_editor_open),
		cmocka_unit_test(source_removed_with_editor_open),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
