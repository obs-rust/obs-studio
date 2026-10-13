/* What Settings -> Hotkeys shows about Wayland background hotkeys: the status
 * banner, the marker after each key field, and the Hyprland config lines
 * (frontend/settings/OBSHotkeyBackground.cpp). The inputs are what libobs
 * reports (obs_hotkeys_get_background_state and friends) and, on Hyprland,
 * what `hyprctl -j binds` prints; the hyprctl samples are real 0.56.2
 * output. */

/* Before cmocka.h, whose fail() macro would hit std::basic_ios::fail(). */
#include <fstream>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

#include "OBSHotkeyBackground.hpp"

#include <string>

using namespace HotkeyBackground;

static const obs_key_combination_t f13 = {0, OBS_KEY_F13};
static const obs_key_combination_t ctrl_shift_a = {INTERACT_CONTROL_KEY | INTERACT_SHIFT_KEY, OBS_KEY_A};
static const obs_key_combination_t mouse4 = {0, OBS_KEY_MOUSE4};

static const char *app = "com.obsproject.Studio";

static Input base_input(obs_hotkeys_background_state state)
{
	Input input;
	input.state = state;
	input.appId = app;
	input.combos = {
		{f13, "OBS_KEY_F13", "F13"},
		{ctrl_shift_a, "ctrl+shift+OBS_KEY_A", "CTRL+SHIFT+a"},
		{mouse4, "", ""},
	};
	return input;
}

static void bind_all(Input &input, const char *f13_trigger = "F13", const char *a_trigger = "CTRL+SHIFT+a")
{
	input.shortcuts = {{f13, f13_trigger, true}, {ctrl_shift_a, a_trigger, true}};
}

static DesktopConfig hyprland(std::set<std::string> bound)
{
	DesktopConfig desktop;
	desktop.name = "Hyprland";
	desktop.configFile = "~/.config/hypr/hyprland.lua";
	desktop.lua = true;
	desktop.binds = HyprlandBinds();
	desktop.binds->ids = bound;
	return desktop;
}

/* X11, Windows, macOS: nothing changes in Settings. */
static void native_shows_nothing(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_NATIVE);
	assert_int_equal((int)Compute(input).banner, (int)Banner::Hidden);
	assert_int_equal((int)MarkerFor(input, f13), (int)Marker::None);
	assert_int_equal((int)MarkerFor(input, mouse4), (int)Marker::None);
}

/* KDE/GNOME: the desktop reports triggers; working, with the mouse button
 * marked focus-only and the desktop's own trigger available per row. */
static void desktop_with_triggers_working(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_ACTIVE);
	input.flags = OBS_HOTKEYS_BACKGROUND_DESKTOP_TRIGGERS | OBS_HOTKEYS_BACKGROUND_PAUSED;
	bind_all(input, "Pause");

	View view = Compute(input);
	assert_int_equal((int)view.banner, (int)Banner::Working);
	assert_int_equal(view.keys, 2);
	assert_int_equal(view.working, 2);
	assert_true(view.focusOnlyKeys);
	assert_true(view.testKey);
	assert_false(view.copyLines);
	assert_int_equal((int)view.level, (int)Level::Ok);
	assert_int_equal((int)MarkerFor(input, f13), (int)Marker::Background);
	assert_int_equal((int)MarkerFor(input, mouse4), (int)Marker::FocusOnly);
	/* The user moved F13's shortcut to Pause in the desktop settings: the
	 * tooltip shows it (trigger texts are localized, so not compared). */
	assert_string_equal(DesktopTrigger(input, f13).c_str(), "Pause");
	/* Portal version 1: no desktop editor to open. */
	assert_false(view.configure);

	input.flags |= OBS_HOTKEYS_BACKGROUND_CONFIGURABLE;
	assert_true(Compute(input).configure);
	assert_false(Compute(input).configureFailed);

	/* The desktop's editor could not be opened: the banner says so. */
	input.flags |= OBS_HOTKEYS_BACKGROUND_CONFIGURE_FAILED;
	assert_true(Compute(input).configureFailed);
}

/* Hyprland takes its keys from its config, even if the portal reports
 * version 2: the config lines are the way to change keys there. */
static void configure_not_on_manual_desktops(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_ACTIVE);
	input.flags = OBS_HOTKEYS_BACKGROUND_CONFIGURABLE;
	bind_all(input, "", "");
	input.desktop = hyprland({std::string(app) + ":OBS_KEY_F13", std::string(app) + ":ctrl+shift+OBS_KEY_A"});
	assert_false(Compute(input).configure);
}

/* A key set in Settings is handed to the desktop only when Settings closes
 * (background presses are off while it is open). */
static void new_key_is_pending(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_ACTIVE);
	input.flags = OBS_HOTKEYS_BACKGROUND_DESKTOP_TRIGGERS | OBS_HOTKEYS_BACKGROUND_PAUSED;
	input.shortcuts = {{f13, "F13", true}};

	View view = Compute(input);
	assert_int_equal((int)view.banner, (int)Banner::Working);
	assert_true(view.pending);
	assert_int_equal((int)MarkerFor(input, ctrl_shift_a), (int)Marker::Pending);
}

/* KDE/GNOME before anything was bound: say when it happens, not "0 of 2". */
static void nothing_bound_yet(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_IDLE);
	input.flags = OBS_HOTKEYS_BACKGROUND_PAUSED;

	View view = Compute(input);
	assert_int_equal((int)view.banner, (int)Banner::Pending);
	assert_false(view.testKey);
	assert_int_equal((int)MarkerFor(input, f13), (int)Marker::Pending);
}

/* Hyprland, nothing in its config yet: a line per key, copy and check. */
static void hyprland_needs_config(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_ACTIVE);
	bind_all(input, "", "");
	input.desktop = hyprland({std::string(app) + ":OBS_KEY_F13"});

	View view = Compute(input);
	assert_int_equal((int)view.banner, (int)Banner::NeedsConfig);
	assert_int_equal((int)view.level, (int)Level::Warning);
	assert_int_equal(view.missing.size(), 1);
	assert_string_equal(view.missing[0].id.c_str(), "ctrl+shift+OBS_KEY_A");
	assert_true(view.copyLines);
	assert_true(view.checkAgain);
	assert_true(view.testKey);
	assert_int_equal((int)MarkerFor(input, f13), (int)Marker::Background);
	assert_int_equal((int)MarkerFor(input, ctrl_shift_a), (int)Marker::NotInConfig);

	input.desktop->binds->ids.insert(std::string(app) + ":ctrl+shift+OBS_KEY_A");
	view = Compute(input);
	assert_int_equal((int)view.banner, (int)Banner::Working);
	assert_int_equal(view.working, 2);
	/* Nothing left to add: no second copy of the lines. */
	assert_false(view.copyLines);
	assert_true(view.checkAgain);
}

/* Hand-written Lua binds (no OBS description) are invisible to hyprctl
 * except for their key: not "missing", but unverified until Test a key
 * receives them, after which they count as working. */
static void hyprland_hand_written_lua(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_ACTIVE);
	input.flags = OBS_HOTKEYS_BACKGROUND_PAUSED;
	bind_all(input, "", "");
	input.desktop = hyprland({std::string(app) + ":OBS_KEY_F13"});
	input.desktop->binds->opaqueKeys.insert("CTRL+SHIFT+A");

	View view = Compute(input);
	assert_int_equal((int)view.banner, (int)Banner::Unverified);
	assert_int_equal(view.unverified, 1);
	assert_true(view.missing.empty());
	assert_false(view.copyLines);
	assert_true(view.testKey);
	assert_true(view.paused);
	assert_int_equal((int)MarkerFor(input, ctrl_shift_a), (int)Marker::Unverified);

	input.confirmedIds.insert(std::string(app) + ":ctrl+shift+OBS_KEY_A");
	view = Compute(input);
	assert_int_equal((int)view.banner, (int)Banner::Working);
	assert_int_equal(view.working, 2);
	assert_int_equal((int)MarkerFor(input, ctrl_shift_a), (int)Marker::Background);
}

/* The id is bound in Hyprland, but on another key than OBS shows. */
static void hyprland_other_key(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_ACTIVE);
	bind_all(input, "", "");
	input.desktop = hyprland({std::string(app) + ":OBS_KEY_F13", std::string(app) + ":ctrl+shift+OBS_KEY_A"});
	input.desktop->binds->keys[std::string(app) + ":OBS_KEY_F13"] = "F14";
	input.desktop->binds->keys[std::string(app) + ":ctrl+shift+OBS_KEY_A"] = "CTRL+SHIFT+A";

	View view = Compute(input);
	assert_int_equal((int)view.banner, (int)Banner::Working);
	assert_int_equal((int)view.level, (int)Level::Warning);
	assert_int_equal(view.otherKey, 1);
	assert_int_equal((int)MarkerFor(input, f13), (int)Marker::OtherKey);
	assert_int_equal((int)MarkerFor(input, ctrl_shift_a), (int)Marker::Background);
	assert_string_equal(DesktopTrigger(input, f13).c_str(), "F14");
}

/* First run on Hyprland: no keys yet; say that a config line will be
 * needed rather than "keys also work in the background". */
static void hyprland_no_keys(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_IDLE);
	input.combos.clear();
	input.desktop = hyprland({});
	assert_int_equal((int)Compute(input).banner, (int)Banner::NoKeysConfig);
}

/* No trigger came back and OBS knows no config for this desktop: it still
 * says that the desktop needs the ids, and offers them. */
static void unknown_manual_desktop(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_ACTIVE);
	bind_all(input, "", "");

	View view = Compute(input);
	assert_int_equal((int)view.banner, (int)Banner::NeedsConfigUnknown);
	assert_true(view.copyLines);
	assert_false(view.checkAgain);
}

static void failures_offer_next_step(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_DECLINED);
	input.flags = OBS_HOTKEYS_BACKGROUND_DESKTOP_TRIGGERS;
	input.shortcuts = {{f13, "", false}, {ctrl_shift_a, "", false}};
	View view = Compute(input);
	assert_int_equal((int)view.banner, (int)Banner::Declined);
	assert_true(view.askAgain);
	assert_int_equal((int)MarkerFor(input, f13), (int)Marker::NotBound);

	input.state = OBS_HOTKEYS_BACKGROUND_CLOSED;
	assert_int_equal((int)Compute(input).banner, (int)Banner::Closed);
	assert_true(Compute(input).askAgain);

	input.state = OBS_HOTKEYS_BACKGROUND_FAILED;
	input.error = "An app id is required";
	assert_int_equal((int)Compute(input).banner, (int)Banner::AppIdMissing);
	/* Asking again cannot install the desktop file. */
	assert_false(Compute(input).askAgain);
	input.error = "Invalid session state";
	assert_int_equal((int)Compute(input).banner, (int)Banner::Failed);
	assert_true(Compute(input).askAgain);
	input.flags |= OBS_HOTKEYS_BACKGROUND_RESTARTING;
	input.error = "The desktop portal stopped";
	assert_int_equal((int)Compute(input).banner, (int)Banner::PortalRestarting);
	assert_false(Compute(input).askAgain);
	input.flags &= ~OBS_HOTKEYS_BACKGROUND_RESTARTING;

	input.state = OBS_HOTKEYS_BACKGROUND_UNAVAILABLE;
	view = Compute(input);
	assert_int_equal((int)view.banner, (int)Banner::Unavailable);
	assert_false(view.askAgain);
	assert_int_equal((int)MarkerFor(input, f13), (int)Marker::None);
	assert_int_equal((int)MarkerFor(input, mouse4), (int)Marker::FocusOnly);
}

static void waiting_for_dialog(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_WAITING);
	input.shortcuts = {{f13, "", false}, {ctrl_shift_a, "", false}};
	assert_int_equal((int)Compute(input).banner, (int)Banner::Waiting);
	assert_int_equal((int)MarkerFor(input, f13), (int)Marker::Pending);
}

static void off_by_setting(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_ACTIVE);
	bind_all(input);
	input.offBySetting = true;
	View view = Compute(input);
	assert_int_equal((int)view.banner, (int)Banner::OffBySetting);
	assert_false(view.testKey);
	assert_int_equal((int)MarkerFor(input, f13), (int)Marker::None);
}

static void only_focus_only_keys(void **)
{
	Input input = base_input(OBS_HOTKEYS_BACKGROUND_IDLE);
	input.combos = {{mouse4, "", ""}};
	assert_int_equal((int)Compute(input).banner, (int)Banner::FocusOnlyKeys);
	input.combos.clear();
	assert_int_equal((int)Compute(input).banner, (int)Banner::NoKeys);
}

/* The exact lines a Hyprland 0.56.2 accepted in the nested e2e test. */
static void hyprland_lines(void **)
{
	DesktopConfig desktop = hyprland({});
	Combo a = {ctrl_shift_a, "ctrl+shift+OBS_KEY_A", "CTRL+SHIFT+a"};
	Combo super_f1 = {{INTERACT_COMMAND_KEY, OBS_KEY_F1}, "super+OBS_KEY_F1", "LOGO+F1"};

	assert_string_equal(
		FormatHyprlandLine(desktop, app, a, true).c_str(),
		"hl.bind(\"CTRL + SHIFT + a\", hl.dsp.global(\"com.obsproject.Studio:ctrl+shift+OBS_KEY_A\"), "
		"{ non_consuming = true, description = \"OBS hotkey "
		"com.obsproject.Studio:ctrl+shift+OBS_KEY_A\" })");
	assert_string_equal(FormatHyprlandLine(desktop, app, super_f1, false).c_str(),
			    "hl.bind(\"SUPER + F1\", hl.dsp.global(\"com.obsproject.Studio:super+OBS_KEY_F1\"), "
			    "{ description = \"OBS hotkey com.obsproject.Studio:super+OBS_KEY_F1\" })");

	desktop.lua = false;
	assert_string_equal(FormatHyprlandLine(desktop, app, a, true).c_str(),
			    "bindn = CTRL SHIFT, a, global, com.obsproject.Studio:ctrl+shift+OBS_KEY_A");
	assert_string_equal(FormatHyprlandLine(desktop, app, {f13, "OBS_KEY_F13", "F13"}, false).c_str(),
			    "bind = , F13, global, com.obsproject.Studio:OBS_KEY_F13");

	/* Focus-only combinations get no line. */
	std::string lines = FormatConfigLines(desktop, app, {a, {mouse4, "", ""}}, true);
	assert_string_equal(lines.c_str(),
			    "bindn = CTRL SHIFT, a, global, com.obsproject.Studio:ctrl+shift+OBS_KEY_A\n");
}

/* `hyprctl -j binds` from Hyprland 0.56.2: Lua binds show only "__lua" and
 * their description; hyprlang binds show the dispatcher and its argument. */
static void hyprctl_binds(void **)
{
	const char *json = R"([
{"locked": false, "non_consuming": true, "has_description": true, "modmask": 0, "key": "F13",
 "description": "OBS hotkey com.obsproject.Studio:OBS_KEY_F13", "dispatcher": "__lua", "arg": "5"},
{"has_description": false, "modmask": 5, "key": "a", "description": "", "dispatcher": "__lua", "arg": "7"},
{"has_description": false, "modmask": 5, "key": "F14", "description": "", "dispatcher": "global",
 "arg": "com.obsproject.Studio:ctrl+shift+OBS_KEY_F14"},
{"has_description": false, "key": "Q", "dispatcher": "killactive", "arg": ""}])";

	auto binds = ParseHyprlandBinds(json);
	assert_true(binds.has_value());
	assert_int_equal(binds->ids.size(), 2);
	assert_true(binds->ids.count("com.obsproject.Studio:OBS_KEY_F13"));
	assert_true(binds->ids.count("com.obsproject.Studio:ctrl+shift+OBS_KEY_F14"));
	/* A prefix of a bound id is not bound. */
	assert_false(binds->ids.count("com.obsproject.Studio:OBS_KEY_F1"));
	assert_string_equal(binds->keys["com.obsproject.Studio:OBS_KEY_F13"].c_str(), "F13");
	assert_string_equal(binds->keys["com.obsproject.Studio:ctrl+shift+OBS_KEY_F14"].c_str(), "CTRL+SHIFT+F14");
	/* The Lua bind without a description: only its key is known. */
	assert_int_equal(binds->opaqueKeys.size(), 1);
	assert_true(binds->opaqueKeys.count("CTRL+SHIFT+A"));

	assert_false(ParseHyprlandBinds("not json").has_value());
	assert_false(ParseHyprlandBinds("{}").has_value());
	assert_true(ParseHyprlandBinds("[]")->ids.empty());

	assert_string_equal(NormalizeTrigger("CTRL+SHIFT+a").c_str(), "CTRL+SHIFT+A");
	assert_string_equal(NormalizeTrigger("Shift+Ctrl+a").c_str(), "CTRL+SHIFT+A");
	assert_string_equal(NormalizeTrigger("LOGO+F1").c_str(), "SUPER+F1");
}

static void hyprctl_version(void **)
{
	auto v = ParseHyprlandVersion(R"({"branch": "v0.56.2", "version": "0.56.2", "tag": "v0.56.2"})");
	assert_true(v.has_value());
	assert_int_equal(v->first, 0);
	assert_int_equal(v->second, 56);
	v = ParseHyprlandVersion(R"({"tag": "v0.54.3"})");
	assert_true(v.has_value());
	assert_int_equal(v->second, 54);
	assert_false(ParseHyprlandVersion("{}").has_value());
	assert_false(ParseHyprlandVersion("").has_value());
}

/* The value of key in the shipped en-US.ini, without quotes. */
static std::string locale_text(const char *key)
{
	std::ifstream file(HOTKEY_LOCALE_EN_US);
	std::string line, prefix = std::string(key) + "=";
	while (std::getline(file, line)) {
		if (line.compare(0, prefix.size(), prefix) == 0) {
			std::string value = line.substr(prefix.size());
			if (value.size() >= 2 && value.front() == '"' && value.back() == '"') {
				value = value.substr(1, value.size() - 2);
			}
			return value;
		}
	}
	return {};
}

/* Desktops without GlobalShortcuts (Sway, niri, ...): running OBS through
 * XWayland is no workaround there. On Sway 1.12, OBS with
 * QT_QPA_PLATFORM=xcb ran its hotkey while focused and not at all while
 * another window was focused, since the compositor does not show X11 apps
 * keys pressed in other windows. The banner must not promise it. */
static void unsupported_desktop_text(void **)
{
	std::string text = locale_text("Basic.Settings.Hotkeys.Background.Unavailable");
	assert_false(text.empty());
	if (text.find("QT_QPA_PLATFORM=xcb") != std::string::npos || text.find("Workaround") != std::string::npos) {
		fprintf(stderr, "unsupported-desktop text recommends XWayland: %s\n", text.c_str());
		fail();
	}
	assert_true(text.find("only work while OBS is focused") != std::string::npos);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(native_shows_nothing),
		cmocka_unit_test(desktop_with_triggers_working),
		cmocka_unit_test(new_key_is_pending),
		cmocka_unit_test(nothing_bound_yet),
		cmocka_unit_test(hyprland_needs_config),
		cmocka_unit_test(hyprland_hand_written_lua),
		cmocka_unit_test(hyprland_other_key),
		cmocka_unit_test(hyprland_no_keys),
		cmocka_unit_test(configure_not_on_manual_desktops),
		cmocka_unit_test(unknown_manual_desktop),
		cmocka_unit_test(failures_offer_next_step),
		cmocka_unit_test(waiting_for_dialog),
		cmocka_unit_test(off_by_setting),
		cmocka_unit_test(only_focus_only_keys),
		cmocka_unit_test(hyprland_lines),
		cmocka_unit_test(hyprctl_binds),
		cmocka_unit_test(hyprctl_version),
		cmocka_unit_test(unsupported_desktop_text),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
