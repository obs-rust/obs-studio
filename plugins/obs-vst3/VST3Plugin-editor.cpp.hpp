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

#include "plugins/obs-vst3/build/obs-vst3.core.pch.hpp"

using namespace Steinberg;
using namespace Steinberg::Vst;

namespace Steinberg {
const FUID IPlugView::iid(0x5BC32507, 0xD06049EA, 0xA6151B52, 0x2B755B29);
const FUID IPlugViewContentScaleSupport::iid(0x65ED9690, 0x8AC44525, 0x8AADEF7A, 0x72EA703F);
const FUID IPlugFrame::iid(0x367FAF01, 0xAFA94693, 0x8D4DA2A0, 0xED0882A3);
} // namespace Steinberg

void VST3Plugin::destroyEditor()
{
	if (window) {
		hideEditor();

		if (view) {
#ifdef OBS_VST3_RUST_HOST
			window->detach();
#else
			view->removed();
			view->setFrame(nullptr);
#endif
		}

		delete window;
		window = nullptr;
	}

#ifdef OBS_VST3_RUST_HOST
	vst3_rust_editor_destroy(view);
#endif
	view = nullptr;
}

#ifndef OBS_VST3_RUST_HOST
/* hack ripped from Juce, to create the view even with badly coded VST3s... */
void VST3Plugin::tryCreatingView()
{
	if (auto *raw = editController->createView(Vst::ViewType::kEditor)) {
		view = IPtr<IPlugView>::adopt(raw);
		return;
	}

	if (auto *raw = editController->createView(nullptr)) {
		view = IPtr<IPlugView>::adopt(raw);
		return;
	}

	IPlugView *raw = nullptr;
	if (editController->queryInterface(IPlugView::iid, reinterpret_cast<void **>(&raw)) == kResultOk) {
		view = IPtr<IPlugView>::adopt(raw);
	}
}

#endif

bool VST3Plugin::createView()
{
#ifdef OBS_VST3_RUST_HOST
	if (view) {
		return false;
	}
	view = vst3_rust_editor_create(rustHost, vst3_editor_platform_type(hostContext->backend()));
	return view != nullptr;
#else
	if (!editController) {
		infovst3plugin("VST3 does not provide an edit controller");
		return false;
	}

	if (view) {
		debugvst3plugin("Editor view or window already exists");
		return false;
	} else {
		tryCreatingView();
	}

	if (!view) {
		infovst3plugin("EditController does not provide its own view");
		return false;
	}

	const auto platformType = vst3_editor_platform_type(hostContext->backend());
	if (!platformType || view->isPlatformTypeSupported(platformType) != Steinberg::kResultTrue) {
		infovst3plugin("Editor view does not support this display backend");
		return false;
	}

	return true;
#endif
}

void VST3Plugin::showEditor()
{
	if (!view) {
		return;
	}

	if (!window) {
		int width = 800;
		int height = 600;
		Steinberg::ViewRect rect;
#ifdef OBS_VST3_RUST_HOST
		if (vst3_rust_editor_size(view, &rect)) {
#else
		if (view->getSize(&rect) == Steinberg::kResultOk) {
#endif
			width = rect.getWidth();
			height = rect.getHeight();
		} else {
			infovst3plugin("Failed to get size before attaching an IFrame. Not SDK compliant.");
		}
		std::string sourceName = obs_source_get_name(filterData->context);
		std::string windowName = sourceName + ": VST3 Plugin - " + name;

		window = new VST3EditorWindow(view, windowName);

		if (window->create(width, height, hostContext->getFUnknown())) {
			window->show();
			editorVisible = true;
		} else {
			infovst3plugin("Failed to create editor window");
			delete window;
			window = nullptr;
		}
	} else {
		window->show();
		editorVisible = true;
	}
}

void VST3Plugin::hideEditor()
{
	if (window && view) {
		window->close();
	}
	editorVisible = false;
}

// This function is required because we don't really close the GUI window; we hide it. This then means we have to track
// when a GUI has been closed by the user when clicking X. I decided to just hide because creating the GUI each time
// the user wants to display it, was prone to crashes. This also simplified the coding.
bool VST3Plugin::isEditorVisible()
{
#ifdef OBS_VST3_RUST_HOST
	if (!window) {
		editorVisible = false;
	}
#endif
	if (window) {
		bool wasClosed = window->getClosedState();
		if (wasClosed && editorVisible) {
			editorVisible = false;
		}
	}
	return editorVisible;
}
