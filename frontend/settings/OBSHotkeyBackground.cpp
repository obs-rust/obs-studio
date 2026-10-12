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

#include "OBSHotkeyBackground.hpp"

#include <nlohmann/json.hpp>

#include <algorithm>
#include <cctype>
#include <cstdio>
#include <sstream>

namespace HotkeyBackground {

static bool SameCombo(const obs_key_combination_t &a, const obs_key_combination_t &b)
{
	return a.key == b.key && a.modifiers == b.modifiers;
}

static const Shortcut *FindShortcut(const Input &input, const obs_key_combination_t &combo)
{
	for (auto &shortcut : input.shortcuts) {
		if (SameCombo(shortcut.combo, combo)) {
			return &shortcut;
		}
	}
	return nullptr;
}

static const Combo *FindCombo(const Input &input, const obs_key_combination_t &combo)
{
	for (auto &c : input.combos) {
		if (SameCombo(c.combo, combo)) {
			return &c;
		}
	}
	return nullptr;
}

static std::string QualifiedId(const Input &input, const Combo &combo)
{
	return input.appId + ":" + combo.id;
}

/* The desktop takes the keys from its own config: a DesktopConfig entry
 * says so, or the desktop bound the shortcuts without reporting triggers. */
static bool ManualConfig(const Input &input)
{
	if (input.desktop) {
		return true;
	}
	if (input.flags & OBS_HOTKEYS_BACKGROUND_DESKTOP_TRIGGERS) {
		return false;
	}
	return std::any_of(input.shortcuts.begin(), input.shortcuts.end(),
			   [](const Shortcut &shortcut) { return shortcut.bound; });
}

static bool ConfigRead(const Input &input)
{
	return input.desktop && input.desktop->binds;
}

/* What the desktop's config says about one combination. */
enum class ConfigState { Unknown, Bound, OtherKey, Opaque, Missing };

static ConfigState InConfig(const Input &input, const Combo &combo)
{
	std::string id = QualifiedId(input, combo);
	if (input.confirmedIds.count(id)) {
		return ConfigState::Bound;
	}
	if (!ConfigRead(input)) {
		return ConfigState::Unknown;
	}
	const HyprlandBinds &binds = *input.desktop->binds;
	if (binds.ids.count(id)) {
		auto key = binds.keys.find(id);
		if (key != binds.keys.end() && !combo.trigger.empty() &&
		    key->second != NormalizeTrigger(combo.trigger)) {
			return ConfigState::OtherKey;
		}
		return ConfigState::Bound;
	}
	if (!combo.trigger.empty() && binds.opaqueKeys.count(NormalizeTrigger(combo.trigger))) {
		return ConfigState::Opaque;
	}
	return ConfigState::Missing;
}

static bool Failing(obs_hotkeys_background_state state)
{
	switch (state) {
	case OBS_HOTKEYS_BACKGROUND_UNAVAILABLE:
	case OBS_HOTKEYS_BACKGROUND_DECLINED:
	case OBS_HOTKEYS_BACKGROUND_CLOSED:
	case OBS_HOTKEYS_BACKGROUND_FAILED:
		return true;
	default:
		return false;
	}
}

Marker MarkerFor(const Input &input, const obs_key_combination_t &combo)
{
	if (input.state == OBS_HOTKEYS_BACKGROUND_NATIVE || obs_key_combination_is_empty(combo)) {
		return Marker::None;
	}

	const Combo *c = FindCombo(input, combo);
	if (!c || c->id.empty()) {
		return Marker::FocusOnly;
	}
	/* The banner says why nothing works in the background. */
	if (input.state == OBS_HOTKEYS_BACKGROUND_UNAVAILABLE || input.offBySetting) {
		return Marker::None;
	}

	const Shortcut *shortcut = FindShortcut(input, combo);
	bool idle = input.state == OBS_HOTKEYS_BACKGROUND_WAITING || input.state == OBS_HOTKEYS_BACKGROUND_IDLE;
	if (!shortcut) {
		return Failing(input.state) ? Marker::NotBound : Marker::Pending;
	}
	if (!shortcut->bound) {
		return idle ? Marker::Pending : Marker::NotBound;
	}
	switch (InConfig(input, *c)) {
	case ConfigState::Missing:
		return Marker::NotInConfig;
	case ConfigState::Opaque:
		return Marker::Unverified;
	case ConfigState::OtherKey:
		return Marker::OtherKey;
	case ConfigState::Unknown:
	case ConfigState::Bound:
		break;
	}
	/* A desktop's own trigger text is localized ("Strg+Umschalt+A"), so it
	 * is shown in the tooltip, not compared. */
	return Marker::Background;
}

std::string DesktopTrigger(const Input &input, const obs_key_combination_t &combo)
{
	const Combo *c = FindCombo(input, combo);
	if (c && ConfigRead(input)) {
		auto key = input.desktop->binds->keys.find(QualifiedId(input, *c));
		if (key != input.desktop->binds->keys.end()) {
			return key->second;
		}
	}
	const Shortcut *shortcut = FindShortcut(input, combo);
	return shortcut ? shortcut->trigger : std::string();
}

View Compute(const Input &input)
{
	View view;
	if (input.state == OBS_HOTKEYS_BACKGROUND_NATIVE) {
		return view;
	}
	view.paused = (input.flags & OBS_HOTKEYS_BACKGROUND_PAUSED) != 0;

	for (auto &combo : input.combos) {
		if (combo.id.empty()) {
			view.focusOnlyKeys = true;
			continue;
		}
		view.keys++;
		Marker marker = MarkerFor(input, combo.combo);
		if (marker == Marker::Background || marker == Marker::OtherKey) {
			view.working++;
		}
		if (marker == Marker::Pending) {
			view.pending = true;
		} else if (marker == Marker::Unverified) {
			view.unverified++;
		} else if (marker == Marker::OtherKey) {
			view.otherKey++;
		}
		if (input.desktop) {
			ConfigState state = InConfig(input, combo);
			if (state == ConfigState::Missing || state == ConfigState::Unknown) {
				view.missing.push_back(combo);
			}
		}
	}

	auto warn = [&](Banner banner) {
		view.banner = banner;
		view.level = Level::Warning;
	};

	if (input.offBySetting) {
		view.banner = Banner::OffBySetting;
		return view;
	}

	switch (input.state) {
	case OBS_HOTKEYS_BACKGROUND_NATIVE:
		break;
	case OBS_HOTKEYS_BACKGROUND_STARTING:
		view.banner = Banner::Starting;
		return view;
	case OBS_HOTKEYS_BACKGROUND_UNAVAILABLE:
		warn(Banner::Unavailable);
		return view;
	case OBS_HOTKEYS_BACKGROUND_DECLINED:
		warn(Banner::Declined);
		view.askAgain = true;
		return view;
	case OBS_HOTKEYS_BACKGROUND_CLOSED:
		warn(Banner::Closed);
		view.askAgain = true;
		return view;
	case OBS_HOTKEYS_BACKGROUND_FAILED:
		if (input.flags & OBS_HOTKEYS_BACKGROUND_RESTARTING) {
			warn(Banner::PortalRestarting);
		} else if (input.error.find("app id") != std::string::npos) {
			/* Asking again cannot install a desktop file. */
			warn(Banner::AppIdMissing);
		} else {
			warn(Banner::Failed);
			view.askAgain = true;
		}
		return view;
	case OBS_HOTKEYS_BACKGROUND_WAITING:
		if (!ManualConfig(input)) {
			view.banner = Banner::Waiting;
			return view;
		}
		break;
	case OBS_HOTKEYS_BACKGROUND_IDLE:
	case OBS_HOTKEYS_BACKGROUND_ACTIVE:
		break;
	}

	if (view.keys == 0) {
		view.banner = view.focusOnlyKeys ? Banner::FocusOnlyKeys
			      : input.desktop    ? Banner::NoKeysConfig
						 : Banner::NoKeys;
		return view;
	}

	if (ManualConfig(input)) {
		view.testKey = true;
		view.copyLines = !view.missing.empty();
		if (input.desktop && !input.desktop->binds) {
			warn(Banner::NeedsConfigUnknown);
			view.copyLines = true;
			return view;
		}
		if (!input.desktop) {
			warn(Banner::NeedsConfigUnknown);
			view.copyLines = true;
			return view;
		}
		view.checkAgain = true;
		if (!view.missing.empty()) {
			warn(Banner::NeedsConfig);
			return view;
		}
		if (view.unverified) {
			view.banner = Banner::Unverified;
			return view;
		}
		view.banner = Banner::Working;
		view.level = view.otherKey ? Level::Warning : Level::Ok;
		return view;
	}

	if (view.working == 0 && view.pending) {
		view.banner = Banner::Pending;
		return view;
	}
	view.testKey = true;
	view.banner = Banner::Working;
	view.level = (view.working == view.keys || view.pending) && !view.otherKey ? Level::Ok : Level::Warning;
	return view;
}

/* ------------------------------------------------------------------------ */
/* Hyprland                                                                  */

/* "CTRL+SHIFT+a" -> {"CTRL", "SHIFT"}, "a"; the XDG LOGO is Hyprland's
 * SUPER. */
static void SplitTrigger(const std::string &trigger, std::vector<std::string> &mods, std::string &key)
{
	std::stringstream stream(trigger);
	std::string part;
	std::vector<std::string> parts;
	while (std::getline(stream, part, '+')) {
		parts.push_back(part);
	}
	if (parts.empty()) {
		return;
	}
	key = parts.back();
	parts.pop_back();
	for (auto &mod : parts) {
		mods.push_back(mod == "LOGO" ? "SUPER" : mod);
	}
}

static std::string Join(const std::vector<std::string> &parts, const char *separator)
{
	std::string out;
	for (size_t i = 0; i < parts.size(); i++) {
		if (i) {
			out += separator;
		}
		out += parts[i];
	}
	return out;
}

static std::string Upper(std::string str)
{
	std::transform(str.begin(), str.end(), str.begin(), [](unsigned char c) { return (char)std::toupper(c); });
	return str;
}

static const char *const modifierOrder[] = {"CTRL", "ALT", "SHIFT", "SUPER"};

std::string NormalizeTrigger(const std::string &trigger)
{
	std::vector<std::string> mods;
	std::string key;
	SplitTrigger(Upper(trigger), mods, key);
	std::vector<std::string> ordered;
	for (const char *mod : modifierOrder) {
		if (std::find(mods.begin(), mods.end(), mod) != mods.end()) {
			ordered.push_back(mod);
		}
	}
	ordered.push_back(key);
	return Join(ordered, "+");
}

std::string FormatHyprlandLine(const DesktopConfig &desktop, const std::string &appId, const Combo &combo,
			       bool nonConsuming)
{
	std::vector<std::string> mods;
	std::string key;
	SplitTrigger(combo.trigger, mods, key);
	std::string id = appId + ":" + combo.id;

	if (!desktop.lua) {
		return std::string(nonConsuming ? "bindn" : "bind") + " = " + Join(mods, " ") + ", " + key +
		       ", global, " + id;
	}

	std::vector<std::string> parts = mods;
	parts.push_back(key);
	/* Lua binds show up in `hyprctl binds` only by their description, so
	 * it carries the id for "Check again". */
	std::string options = std::string("{ ") + (nonConsuming ? "non_consuming = true, " : "") +
			      "description = \"OBS hotkey " + id + "\" }";
	return "hl.bind(\"" + Join(parts, " + ") + "\", hl.dsp.global(\"" + id + "\"), " + options + ")";
}

std::string FormatConfigLines(const DesktopConfig &desktop, const std::string &appId, const std::vector<Combo> &combos,
			      bool nonConsuming)
{
	std::string out;
	for (auto &combo : combos) {
		if (combo.id.empty() || combo.trigger.empty()) {
			continue;
		}
		out += FormatHyprlandLine(desktop, appId, combo, nonConsuming);
		out += "\n";
	}
	return out;
}

/* "OBS hotkey com.obsproject.Studio:OBS_KEY_F13" -> every "app:id" token. */
static void IdsInDescription(const std::string &description, std::vector<std::string> &ids)
{
	std::stringstream stream(description);
	std::string word;
	while (stream >> word) {
		if (word.find(':') != std::string::npos) {
			ids.push_back(word);
		}
	}
}

/* Hyprland's modmask bits (wlr/xkb modifier order). */
static std::string BindKey(const nlohmann::json &bind)
{
	int modmask = bind.value("modmask", 0);
	std::vector<std::string> parts;
	if (modmask & 4) {
		parts.push_back("CTRL");
	}
	if (modmask & 8) {
		parts.push_back("ALT");
	}
	if (modmask & 1) {
		parts.push_back("SHIFT");
	}
	if (modmask & 64) {
		parts.push_back("SUPER");
	}
	parts.push_back(Upper(bind.value("key", "")));
	return Join(parts, "+");
}

std::optional<HyprlandBinds> ParseHyprlandBinds(const std::string &json)
{
	auto list = nlohmann::json::parse(json, nullptr, false);
	if (list.is_discarded() || !list.is_array()) {
		return std::nullopt;
	}

	HyprlandBinds binds;
	for (auto &bind : list) {
		if (!bind.is_object()) {
			continue;
		}
		std::string dispatcher = bind.value("dispatcher", "");
		std::vector<std::string> ids;
		if (dispatcher == "global") {
			ids.push_back(bind.value("arg", ""));
		} else if (bind.value("has_description", false)) {
			IdsInDescription(bind.value("description", ""), ids);
		}
		if (ids.empty() && dispatcher == "__lua") {
			binds.opaqueKeys.insert(BindKey(bind));
		}
		for (auto &id : ids) {
			binds.ids.insert(id);
			binds.keys[id] = BindKey(bind);
		}
	}
	return binds;
}

std::optional<std::pair<int, int>> ParseHyprlandVersion(const std::string &json)
{
	auto info = nlohmann::json::parse(json, nullptr, false);
	if (info.is_discarded() || !info.is_object()) {
		return std::nullopt;
	}

	std::string version = info.value("version", "");
	if (version.empty()) {
		version = info.value("tag", "");
		if (!version.empty() && version[0] == 'v') {
			version.erase(0, 1);
		}
	}
	int major = 0, minor = 0;
	if (std::sscanf(version.c_str(), "%d.%d", &major, &minor) != 2) {
		return std::nullopt;
	}
	return std::make_pair(major, minor);
}

} // namespace HotkeyBackground
