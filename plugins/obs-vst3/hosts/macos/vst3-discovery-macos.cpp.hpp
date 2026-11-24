/******************************************************************************
    Copyright (C) 2025-2026 pkv <pkv@obsproject.com>
    This file is part of obs-vst3.
    It uses the Steinberg VST3 SDK, which is licensed under MIT license.
    See https://github.com/steinbergmedia/vst3sdk for details.
    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.
    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.
    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "plugins/obs-vst3/build/obs-vst3.discovery.macos.pch.hpp"

bool vst3_discovery_uses_binary_paths()
{
	return false;
}

std::vector<std::string> VST3Scanner::getDefaultSearchPaths()
{
	std::vector<std::string> paths;
	paths.emplace_back("/Library/Audio/Plug-Ins/VST3");
	if (const char *home = std::getenv("HOME")) {
		paths.emplace_back(std::string(home) + "/Library/Audio/Plug-Ins/VST3");
	}
	return paths;
}
