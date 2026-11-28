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

#include "plugins/obs-vst3/build/obs-vst3.editor.windows.pch.hpp"

using namespace Steinberg;

namespace {

constexpr int obsIconId = 101;

HMODULE getCurrentModule()
{
	HMODULE module = nullptr;

	GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
			   reinterpret_cast<LPCWSTR>(&getCurrentModule), &module);

	return module;
}

} // namespace

void VST3EditorWindow::initializePlatformWindow()
{
	if (const HICON icon = LoadIconW(getCurrentModule(), MAKEINTRESOURCEW(obsIconId))) {
		setWindowIcon(QIcon(QPixmap::fromImage(QImage::fromHICON(icon))));
	}
}

FIDString VST3EditorWindow::platformViewType()
{
	return kPlatformTypeHWND;
}

void VST3EditorWindow::preparePlatformView(const QSize &) {}

tresult VST3EditorWindow::queryPlatformFrameInterface(const TUID, void **obj)
{
	*obj = nullptr;
	return kNoInterface;
}

void VST3EditorWindow::handlePlatformResize(const QSize &)
{
	ViewRect actualRect = qtToVst3Rect(viewContainer_->size());
	notifyViewSize(&actualRect);
}

qreal VST3EditorWindow::vst3CoordinateScaleFactor() const
{
	return windowHandle() ? windowHandle()->devicePixelRatio() : devicePixelRatioF();
}

bool VST3EditorWindow::usesContentScaleFactor()
{
	return true;
}

bool VST3EditorWindow::nativeEvent(const QByteArray &eventType, void *message, qintptr *result)
{
	if (eventType != QByteArrayLiteral("windows_generic_MSG")) {
		return QWidget::nativeEvent(eventType, message, result);
	}

	auto *msg = static_cast<MSG *>(message);

	if (!view_ || !attached_ || !resizable_ || msg->hwnd != reinterpret_cast<HWND>(winId()) ||
	    msg->message != WM_SIZING) {
		return QWidget::nativeEvent(eventType, message, result);
	}

	auto *windowRect = reinterpret_cast<RECT *>(msg->lParam);
	if (!windowRect) {
		return QWidget::nativeEvent(eventType, message, result);
	}

	const auto style = static_cast<DWORD>(GetWindowLong(msg->hwnd, GWL_STYLE));
	const auto exStyle = static_cast<DWORD>(GetWindowLong(msg->hwnd, GWL_EXSTYLE));

	RECT nonClientRect{0, 0, 0, 0};
	AdjustWindowRectEx(&nonClientRect, style, FALSE, exStyle);

	const int extraWidth = nonClientRect.right - nonClientRect.left;
	const int extraHeight = nonClientRect.bottom - nonClientRect.top;

	const int windowWidth = windowRect->right - windowRect->left;
	const int windowHeight = windowRect->bottom - windowRect->top;

	ViewRect constrained(0, 0, windowWidth - extraWidth, windowHeight - extraHeight);
	constrainView(&constrained);

	const int newWindowWidth = constrained.getWidth() + extraWidth;
	const int newWindowHeight = constrained.getHeight() + extraHeight;

	switch (msg->wParam) {
	case WMSZ_LEFT:
	case WMSZ_TOPLEFT:
	case WMSZ_BOTTOMLEFT:
		windowRect->left = windowRect->right - newWindowWidth;
		break;

	default:
		windowRect->right = windowRect->left + newWindowWidth;
		break;
	}

	switch (msg->wParam) {
	case WMSZ_TOP:
	case WMSZ_TOPLEFT:
	case WMSZ_TOPRIGHT:
		windowRect->top = windowRect->bottom - newWindowHeight;
		break;

	default:
		windowRect->bottom = windowRect->top + newWindowHeight;
		break;
	}

	if (result) {
		*result = TRUE;
	}

	return true;
}
