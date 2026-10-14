/******************************************************************************
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
******************************************************************************/

#pragma once

/* What Settings -> Hotkeys shows about background hotkeys (the desktop's
 * GlobalShortcuts portal on Wayland): the status banner and the marker after
 * each key field. Pure logic, no widgets, so it is tested on its own.
 *
 * The UI follows capabilities, not desktop names: libobs reports the state
 * and whether the desktop assigns triggers itself; a desktop that takes the
 * keys from its own config instead (Hyprland) is described by a
 * DesktopConfig entry, which formats the config lines and reports which
 * shortcut ids that config binds. A new desktop of that kind is a new
 * DesktopConfig, not new UI code. */

#include <obs.h>

#include <map>
#include <optional>
#include <set>
#include <string>
#include <vector>

namespace HotkeyBackground {

/* One key combination of the hotkeys in Settings (saved or not). */
struct Combo {
	obs_key_combination_t combo;
	/* Desktop shortcut id; empty for a focus-only combination (mouse
	 * buttons, modifier-only). */
	std::string id;
	/* The key OBS suggests ("CTRL+SHIFT+F13"); used for config lines. */
	std::string trigger;
};

/* A shortcut of the last binding, as libobs reports it. */
struct Shortcut {
	obs_key_combination_t combo;
	std::string trigger;
	bool bound = false;
};

/* What `hyprctl -j binds` says about OBS's shortcuts. */
struct HyprlandBinds {
	/* "app id:shortcut id" of every bind OBS can identify: hyprlang
	 * `global` binds, and Lua binds carrying OBS's description. */
	std::set<std::string> ids;
	/* The key Hyprland runs each of them on ("CTRL+SHIFT+A"). */
	std::map<std::string, std::string> keys;
	/* Keys of Lua binds OBS cannot identify: Hyprland 0.56 lists them
	 * only as "__lua"; one may be a hand-written OBS line. */
	std::set<std::string> opaqueKeys;
};

/* A desktop that takes the keys for OBS's shortcut ids from its own config. */
struct DesktopConfig {
	/* Shown in messages: "Hyprland". */
	std::string name;
	/* Where the lines go: "~/.config/hypr/hyprland.lua". */
	std::string configFile;
	/* Lua syntax (Hyprland 0.55+) instead of hyprlang. */
	bool lua = true;
	/* The binds, if the config could be read. */
	std::optional<HyprlandBinds> binds;
};

struct Input {
	obs_hotkeys_background_state state = OBS_HOTKEYS_BACKGROUND_NATIVE;
	uint32_t flags = 0;
	std::string error;
	std::string appId;
	std::vector<Combo> combos;
	std::vector<Shortcut> shortcuts;
	/* "Disable hotkeys when main window is not in focus". */
	bool offBySetting = false;
	std::optional<DesktopConfig> desktop;
	/* "app id:shortcut id" that "Test a key" received this session: they
	 * work, whatever OBS can read of the config. */
	std::set<std::string> confirmedIds;
};

enum class Banner {
	Hidden,
	Starting,
	Working,
	/* Some ids are not bound in the desktop's config. */
	NeedsConfig,
	/* Every key has a bind OBS cannot identify (hand-written Lua). */
	Unverified,
	/* The desktop reports no triggers and OBS cannot read its config. */
	NeedsConfigUnknown,
	Waiting,
	Declined,
	Closed,
	Unavailable,
	Failed,
	AppIdMissing,
	/* The desktop portal went away; OBS binds again when it is back. */
	PortalRestarting,
	OffBySetting,
	/* Only mouse buttons / modifier-only keys are set. */
	FocusOnlyKeys,
	/* No hotkeys set yet. */
	NoKeys,
	/* No hotkeys set yet, on a desktop that needs config lines. */
	NoKeysConfig,
	/* Nothing bound yet; the keys set are handed to the desktop when
	 * Settings closes. */
	Pending,
	/* The desktop assigns the keys itself (it has its own editor) and gave
	 * none of OBS's shortcuts a key. */
	NoDesktopKeys,
};

enum class Level { Ok, Warning, Info };

enum class Marker {
	None,
	Background,
	FocusOnly,
	/* Not bound in the desktop's config (Hyprland). */
	NotInConfig,
	/* A bind on this key exists but OBS cannot tell what it does. */
	Unverified,
	/* The desktop runs the shortcut on another key than OBS shows. */
	OtherKey,
	/* Set in Settings but not handed to the desktop yet: that happens when
	 * Settings closes. */
	Pending,
	/* The desktop has not bound it (declined, closed). */
	NotBound,
};

struct View {
	Banner banner = Banner::Hidden;
	Level level = Level::Info;
	/* Keyboard combinations, and how many of them work in the background. */
	size_t keys = 0;
	size_t working = 0;
	/* Combinations whose config line is missing (Hyprland). */
	std::vector<Combo> missing;
	/* Combinations with a bind OBS cannot identify. */
	size_t unverified = 0;
	/* Combinations the desktop runs on another key. */
	size_t otherKey = 0;
	/* Combinations the desktop has no key for. */
	size_t notBound = 0;
	/* The desktop takes the keys it runs: the focused app doesn't get
	 * them (KDE, GNOME). */
	bool desktopTakesKeys = false;
	/* The desktop's dialog for newly added keys was declined; the others
	 * stay bound. */
	bool newKeysDeclined = false;
	/* Some keys are set but not handed to the desktop until Settings
	 * closes. */
	bool pending = false;
	/* Background presses are off right now (Settings is open). */
	bool paused = false;
	/* Some combinations are mouse buttons or modifier-only. */
	bool focusOnlyKeys = false;
	bool copyLines = false;
	bool checkAgain = false;
	bool testKey = false;
	bool askAgain = false;
	/* The desktop can open its own editor for OBS's shortcuts. */
	bool configure = false;
	/* The last attempt to open that editor failed. */
	bool configureFailed = false;
};

View Compute(const Input &input);
Marker MarkerFor(const Input &input, const obs_key_combination_t &combo);
/* The key the desktop runs combo's shortcut on, if it reports one. */
std::string DesktopTrigger(const Input &input, const obs_key_combination_t &combo);

/* Config lines for the desktop, one per combination. nonConsuming: the
 * focused app (a game) still receives the key, as on X11. */
std::string FormatHyprlandLine(const DesktopConfig &desktop, const std::string &appId, const Combo &combo,
			       bool nonConsuming);
std::string FormatConfigLines(const DesktopConfig &desktop, const std::string &appId, const std::vector<Combo> &combos,
			      bool nonConsuming);

/* "CTRL+SHIFT+a" / "LOGO+F1" -> "CTRL+SHIFT+A" / "SUPER+F1", for comparing
 * keys. */
std::string NormalizeTrigger(const std::string &trigger);

/* `hyprctl -j binds` output; nullopt if it does not parse. */
std::optional<HyprlandBinds> ParseHyprlandBinds(const std::string &json);
/* `hyprctl -j version` output -> {major, minor}; nullopt if unknown. */
std::optional<std::pair<int, int>> ParseHyprlandVersion(const std::string &json);

} // namespace HotkeyBackground
