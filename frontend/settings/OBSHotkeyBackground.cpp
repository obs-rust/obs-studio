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

/* Stubs until Settings -> Hotkeys shows background hotkeys. */

namespace HotkeyBackground {

View Compute(const Input &)
{
	return View();
}

Marker MarkerFor(const Input &, const obs_key_combination_t &)
{
	return Marker::None;
}

std::string DesktopTrigger(const Input &, const obs_key_combination_t &)
{
	return {};
}

std::string FormatHyprlandLine(const DesktopConfig &, const std::string &, const Combo &, bool)
{
	return {};
}

std::string FormatConfigLines(const DesktopConfig &, const std::string &, const std::vector<Combo> &, bool)
{
	return {};
}

std::string NormalizeTrigger(const std::string &)
{
	return {};
}

std::optional<HyprlandBinds> ParseHyprlandBinds(const std::string &)
{
	return std::nullopt;
}

std::optional<std::pair<int, int>> ParseHyprlandVersion(const std::string &)
{
	return std::nullopt;
}

} // namespace HotkeyBackground
