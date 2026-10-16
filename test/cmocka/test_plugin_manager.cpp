/* Plugin Manager (frontend/plugin-manager) with third-party plugins that are
 * not loaded.
 *
 * Safe Mode and --only-bundled-plugins load only the bundled modules
 * (PluginManager::Mode::CoreOnly). That is not a failure, so OBS must not
 * show the "plugins failed to load" dialog, and the cached third-party
 * plugins are not "Missing": their rows must not offer the trash button that
 * deletes their saved settings (obsproject/obs-studio#13948).
 *
 * Removing a Missing entry with its trash button must remove that entry and
 * leave every other row acting on its own plugin: the next trash button
 * removes its own entry, and a toggle changes its own plugin
 * (obsproject/obs-studio#14005). Turning a plugin off still asks for a
 * restart.
 *
 * The real PluginManager, PluginManagerWindow and InstalledPluginRow are built
 * against stand-ins for the frontend app (plugin-manager-stubs/) and for the
 * libobs module calls (below), and run on Qt's offscreen platform: no display,
 * no libobs core. Buttons are clicked the way a user clicks them. */

#include <OBSApp.hpp>
#include <qt-wrappers.hpp>
#include <widgets/OBSBasic.hpp>

#include <plugin-manager/InstalledPluginRow.hpp>
#include <plugin-manager/PluginManager.hpp>
#include <plugin-manager/PluginManagerWindow.hpp>

#include <components/InfoChip.hpp>

#include <Idian/InlineButton.hpp>
#include <Idian/ListHeader.hpp>
#include <Idian/RowList.hpp>
#include <Idian/ToggleSwitch.hpp>

#include <QApplication>
#include <QLabel>
#include <QTemporaryDir>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

#include <algorithm>
#include <cstdio>
#include <deque>
#include <string>
#include <vector>

/* ---- frontend stand-ins ------------------------------------------------- */

bool safe_mode = false;
bool disable_3p_plugins = false;
bool restart = false;

static OBSApp app_stub;

OBSApp *App()
{
	return &app_stub;
}

OBSBasic *OBSBasic::Get()
{
	return nullptr;
}

QMessageBox::StandardButton OBSMessageBox::question(QWidget *, const QString &, const QString &,
						    QMessageBox::StandardButtons, QMessageBox::StandardButton)
{
	return QMessageBox::No;
}

extern "C" bool obs_frontend_is_safe_mode_enabled(void)
{
	return safe_mode;
}

/* ---- libobs module calls ------------------------------------------------ */

/* A module the fake libobs knows: loaded (obs_get_module), or not loaded on
 * purpose (obs_get_disabled_module). Anything else does not exist. */
struct FakeModule {
	std::string name;
	bool loaded;
	std::string file_name;
};

static std::deque<FakeModule> fake_modules;

static FakeModule *find_fake(const char *name, bool loaded)
{
	for (FakeModule &m : fake_modules) {
		if (m.name == name && m.loaded == loaded) {
			return &m;
		}
	}
	return nullptr;
}

static obs_module_t *as_module(FakeModule *m)
{
	return reinterpret_cast<obs_module_t *>(m);
}

static FakeModule *as_fake(obs_module_t *m)
{
	return reinterpret_cast<FakeModule *>(m);
}

extern "C" {
void blog(int, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	vfprintf(stderr, format, args);
	fputc('\n', stderr);
	va_end(args);
}

obs_module_t *obs_get_module(const char *name)
{
	return as_module(find_fake(name, true));
}

obs_module_t *obs_get_disabled_module(const char *name)
{
	return as_module(find_fake(name, false));
}

bool obs_is_legacy_module(obs_module_t *)
{
	return false;
}

bool obs_is_core_module(obs_module_t *)
{
	return false;
}

const char *obs_get_module_file_name(obs_module_t *module)
{
	FakeModule *m = as_fake(module);
	m->file_name = m->name + ".so";
	return m->file_name.c_str();
}

const char *obs_get_module_name(obs_module_t *)
{
	return nullptr;
}

const char *obs_get_module_id(obs_module_t *)
{
	return nullptr;
}

const char *obs_get_module_version(obs_module_t *)
{
	return nullptr;
}

bool obs_load_core_modules(void)
{
	return true;
}

void obs_log_loaded_modules(void) {}

void obs_post_load_modules(void) {}

void obs_enum_modules(obs_enum_module_callback_t callback, void *param)
{
	for (FakeModule &m : fake_modules) {
		if (m.loaded) {
			callback(param, as_module(&m));
		}
	}
}

void obs_add_disabled_module(const char *) {}

void obs_module_add_source(obs_module_t *, const char *) {}
void obs_module_add_output(obs_module_t *, const char *) {}
void obs_module_add_encoder(obs_module_t *, const char *) {}
void obs_module_add_service(obs_module_t *, const char *) {}

bool obs_enum_source_types(size_t, const char **)
{
	return false;
}

bool obs_enum_output_types(size_t, const char **)
{
	return false;
}

bool obs_enum_encoder_types(size_t, const char **)
{
	return false;
}

bool obs_enum_service_types(size_t, const char **)
{
	return false;
}

obs_module_t *obs_source_get_module(const char *)
{
	return nullptr;
}

obs_module_t *obs_output_get_module(const char *)
{
	return nullptr;
}

obs_module_t *obs_encoder_get_module(const char *)
{
	return nullptr;
}

obs_module_t *obs_service_get_module(const char *)
{
	return nullptr;
}
}

/* The platform loaders (PluginModuleLoader_*.cpp) are not built here: a full
 * load reports what the test sets. */
static OBS::PluginManager::State plugin_load_result;
static int plugin_load_calls;

namespace OBS {
PluginManager::State PluginManager::loadPlugins(bool)
{
	plugin_load_calls++;
	return plugin_load_result;
}

PluginManager::State PluginManager::loadLegacyPlugins(bool)
{
	return State::Success;
}
} // namespace OBS

/* ---- helpers ------------------------------------------------------------ */

using OBS::ModuleInfo;
using OBS::PluginManager;
using OBS::PluginManagerWindow;

static ModuleInfo cached(const char *name, bool enabled = true)
{
	ModuleInfo m{};
	m.module_name = name;
	m.enabled = enabled;
	m.enabledAtLaunch = enabled;
	return m;
}

static void add_fake(const char *name, bool loaded)
{
	fake_modules.push_back({name, loaded, {}});
}

/* One row of the window as the user sees it. */
struct Row {
	OBS::InstalledPluginRow *widget = nullptr;
	QString name;
	QString section; /* the ListHeader title key of its list */
	QStringList chips;
	idian::ToggleSwitch *toggle = nullptr;
	idian::InlineButton *trash = nullptr;
};

static QString section_of(QWidget *row)
{
	QWidget *w = row->parentWidget();
	while (w && !qobject_cast<idian::RowList *>(w)) {
		w = w->parentWidget();
	}
	if (!w) {
		return {};
	}
	for (idian::ListHeader *header : w->findChildren<idian::ListHeader *>()) {
		for (QLabel *label : header->findChildren<QLabel *>()) {
			if (label->text().startsWith("PluginManager.Section.") && label->text().endsWith(".Title")) {
				return label->text();
			}
		}
	}
	return {};
}

static std::vector<Row> rows_of(PluginManagerWindow &window)
{
	std::vector<Row> rows;
	for (OBS::InstalledPluginRow *widget : window.findChildren<OBS::InstalledPluginRow *>()) {
		Row row;
		row.widget = widget;
		row.section = section_of(widget);
		for (QLabel *label : widget->findChildren<QLabel *>()) {
			if (qobject_cast<InfoChip *>(label)) {
				row.chips << label->text();
			} else if (row.name.isEmpty() && label->property("class").toString().contains("title")) {
				row.name = label->text();
			}
		}
		row.toggle = widget->findChild<idian::ToggleSwitch *>();
		row.trash = widget->findChild<idian::InlineButton *>();
		rows.push_back(row);
	}
	return rows;
}

static Row row_named(PluginManagerWindow &window, const char *name)
{
	for (const Row &row : rows_of(window)) {
		if (row.name == name) {
			return row;
		}
	}
	fail_msg("no row for '%s' in the Plugin Manager", name);
	return {};
}

static bool has_label(QWidget &window, const char *text)
{
	for (QLabel *label : window.findChildren<QLabel *>()) {
		if (label->text() == text) {
			return true;
		}
	}
	return false;
}

static std::string names(const std::vector<ModuleInfo> &modules)
{
	std::string out;
	for (const ModuleInfo &m : modules) {
		out += (out.empty() ? "" : ", ") + m.module_name + (m.enabled ? "" : " (off)");
	}
	return out;
}

static const ModuleInfo *module_named(const std::vector<ModuleInfo> &modules, const char *name)
{
	for (const ModuleInfo &m : modules) {
		if (m.module_name == name) {
			return &m;
		}
	}
	return nullptr;
}

/* Rows of plugins that are cached but were skipped at launch (Safe Mode or
 * --only-bundled-plugins): listed as installed and disabled, never as
 * Missing, and without a trash button. */
static void assert_skipped_rows(PluginManagerWindow &window, const char *mode)
{
	for (const char *name : {"plugin-on", "plugin-off"}) {
		Row row = row_named(window, name);
		if (row.chips.contains("PluginManager.Status.Missing") || row.trash) {
			fail_msg(
				"%s: '%s' is listed as Missing (chips '%s', trash button %s); it was not loaded on purpose",
				mode, name, row.chips.join(",").toUtf8().constData(), row.trash ? "shown" : "hidden");
		}
		if (row.section != "PluginManager.Section.Manage.Title") {
			fail_msg("%s: '%s' is in section '%s', expected the Installed section", mode, name,
				 row.section.toUtf8().constData());
		}
		if (!row.chips.contains("PluginManager.Status.Disabled")) {
			fail_msg("%s: '%s' has chips '%s', expected 'Disabled'", mode, name,
				 row.chips.join(",").toUtf8().constData());
		}
	}
}

/* cmocka leaves a failed test with longjmp, past C++ destructors: the window
 * under test lives here and teardown deletes it. */
static PluginManagerWindow *window_under_test;

static PluginManagerWindow &open_window(const std::vector<ModuleInfo> &modules)
{
	window_under_test = new PluginManagerWindow(modules, {});
	return *window_under_test;
}

static int teardown(void **)
{
	delete window_under_test;
	window_under_test = nullptr;
	return 0;
}

static int setup(void **)
{
	safe_mode = false;
	disable_3p_plugins = false;
	restart = false;
	fake_modules.clear();
	plugin_load_result = PluginManager::State::Success;
	plugin_load_calls = 0;
	return 0;
}

/* ---- #13948: Safe Mode and --only-bundled-plugins ----------------------- */

static PluginManager::State load_state_for(PluginManager::Mode mode)
{
	QTemporaryDir config;
	assert_true(config.isValid());
	app_stub.userPluginManagerSettingsLocation = config.path().toStdString();
	std::filesystem::create_directories(app_stub.userPluginManagerSettingsLocation / OBS_CONFIG_DIRECTORY /
					    "plugin_manager");

	PluginManager manager;
	manager.setPluginMode(mode);
	manager.loadAllPlugins(false);
	return manager.loadState();
}

static const char *state_name(PluginManager::State state)
{
	switch (state) {
	case PluginManager::State::Success:
		return "Success";
	case PluginManager::State::PartialFailure:
		return "PartialFailure";
	case PluginManager::State::Failure:
		return "Failure";
	}
	return "?";
}

/* Safe Mode and --only-bundled-plugins: the bundled modules loaded and no
 * third-party plugin was meant to load, so nothing failed. Any other state
 * puts up the "plugins failed to load" dialog after the main window opens. */
static void core_only_load_reports_success(void **)
{
	PluginManager::State state = load_state_for(PluginManager::Mode::CoreOnly);
	if (state != PluginManager::State::Success) {
		fail_msg("a core-only load (Safe Mode, --only-bundled-plugins) left the load state at %s; "
			 "OBS shows the plugin failure dialog for anything but Success",
			 state_name(state));
	}
	assert_int_equal(plugin_load_calls, 0);
}

/* Guard: a normal start still reports what the third-party load did. */
static void full_load_reports_plugin_failures(void **)
{
	plugin_load_result = PluginManager::State::PartialFailure;
	assert_int_equal(load_state_for(PluginManager::Mode::Full), PluginManager::State::PartialFailure);
	assert_int_equal(plugin_load_calls, 1);

	plugin_load_result = PluginManager::State::Success;
	assert_int_equal(load_state_for(PluginManager::Mode::Full), PluginManager::State::Success);
}

static void safe_mode_lists_skipped_plugins_as_installed(void **)
{
	safe_mode = true;
	/* Cached from an earlier normal start, not loaded now. */
	PluginManagerWindow &window = open_window({cached("plugin-on"), cached("plugin-off", false)});
	assert_skipped_rows(window, "Safe Mode");
	assert_true(has_label(window, "PluginManager.SafeMode"));
}

static void only_bundled_plugins_lists_skipped_plugins_as_installed(void **)
{
	disable_3p_plugins = true;
	PluginManagerWindow &window = open_window({cached("plugin-on"), cached("plugin-off", false)});
	assert_skipped_rows(window, "--only-bundled-plugins");
	assert_true(has_label(window, "PluginManager.OnlyBundledPlugins"));
}

/* Guard: on a normal start a cached plugin that is gone is still Missing and
 * can be removed. */
static void normal_start_lists_gone_plugins_as_missing(void **)
{
	add_fake("plugin-loaded", true);
	PluginManagerWindow &window = open_window({cached("plugin-gone"), cached("plugin-loaded")});

	Row gone = row_named(window, "plugin-gone");
	assert_true(gone.chips.contains("PluginManager.Status.Missing"));
	assert_string_equal(gone.section.toUtf8().constData(), "PluginManager.Section.Missing.Title");
	assert_non_null(gone.trash);

	Row loaded = row_named(window, "plugin-loaded");
	assert_string_equal(loaded.section.toUtf8().constData(), "PluginManager.Section.Manage.Title");
	assert_null(loaded.trash);
	assert_false(has_label(window, "PluginManager.SafeMode"));
}

/* ---- #14005: removing Missing entries ----------------------------------- */

/* Three cached plugins that are gone (Missing) before two that loaded. */
static std::vector<ModuleInfo> cache_with_missing_entries()
{
	add_fake("plugin-x", true);
	add_fake("plugin-y", true);
	return {cached("ghost-a"), cached("ghost-b"), cached("ghost-c"), cached("plugin-x"), cached("plugin-y")};
}

static void removing_missing_entries_removes_the_clicked_ones(void **)
{
	PluginManagerWindow &window = open_window(cache_with_missing_entries());
	row_named(window, "ghost-a").trash->click();
	row_named(window, "ghost-c").trash->click();

	std::string kept = names(window.getModules());
	if (kept != "ghost-b, plugin-x, plugin-y") {
		fail_msg(
			"removed ghost-a, then ghost-c: the window would save '%s', expected 'ghost-b, plugin-x, plugin-y'",
			kept.c_str());
	}
}

static void toggle_after_a_removal_changes_its_own_plugin(void **)
{
	PluginManagerWindow &window = open_window(cache_with_missing_entries());
	row_named(window, "ghost-a").trash->click();
	row_named(window, "plugin-x").toggle->click();

	std::vector<ModuleInfo> modules = window.getModules();
	const ModuleInfo *x = module_named(modules, "plugin-x");
	const ModuleInfo *y = module_named(modules, "plugin-y");
	assert_non_null(x);
	assert_non_null(y);
	if (x->enabled || !y->enabled) {
		fail_msg("removed ghost-a, then turned plugin-x off: the window would save '%s', expected plugin-x off",
			 names(modules).c_str());
	}
}

/* Guard: turning a loaded plugin off asks for a restart, turning it back on
 * does not. */
static void toggle_asks_for_a_restart(void **)
{
	PluginManagerWindow &window = open_window(cache_with_missing_entries());
	assert_false(window.isEnabledPluginsChanged());

	row_named(window, "plugin-x").toggle->click();
	assert_false(module_named(window.getModules(), "plugin-x")->enabled);
	assert_true(window.isEnabledPluginsChanged());

	row_named(window, "plugin-x").toggle->click();
	assert_true(module_named(window.getModules(), "plugin-x")->enabled);
	assert_false(window.isEnabledPluginsChanged());
}

int main(int argc, char **argv)
{
	qputenv("QT_QPA_PLATFORM", "offscreen");
	QApplication app(argc, argv);

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(core_only_load_reports_success, setup, teardown),
		cmocka_unit_test_setup_teardown(full_load_reports_plugin_failures, setup, teardown),
		cmocka_unit_test_setup_teardown(safe_mode_lists_skipped_plugins_as_installed, setup, teardown),
		cmocka_unit_test_setup_teardown(only_bundled_plugins_lists_skipped_plugins_as_installed, setup,
						teardown),
		cmocka_unit_test_setup_teardown(normal_start_lists_gone_plugins_as_missing, setup, teardown),
		cmocka_unit_test_setup_teardown(removing_missing_entries_removes_the_clicked_ones, setup, teardown),
		cmocka_unit_test_setup_teardown(toggle_after_a_removal_changes_its_own_plugin, setup, teardown),
		cmocka_unit_test_setup_teardown(toggle_asks_for_a_restart, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
