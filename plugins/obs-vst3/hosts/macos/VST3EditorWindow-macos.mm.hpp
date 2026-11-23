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

#include "plugins/obs-vst3/build/obs-vst3.macos.pch.hpp"

void VST3EditorWindow::setMacContentAspectRatio(const QSize &size)
{
	if (size.isEmpty()) {
		return;
	}

	NSView *nativeView = (__bridge NSView *)(reinterpret_cast<void *>(winId()));

	if (!nativeView || !nativeView.window) {
		return;
	}

	nativeView.window.contentAspectRatio =
		NSMakeSize(static_cast<CGFloat>(size.width()), static_cast<CGFloat>(size.height()));
}
