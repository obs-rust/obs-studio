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

#include "plugins/obs-vst3/build/obs-vst3.editor.linux.pch.hpp"

using namespace Steinberg;

void VST3EditorWindow::initializePlatformWindow()
{
	resizeTimer_ = new QTimer(this);
	resizeTimer_->setSingleShot(true);
	resizeTimer_->setInterval(10);
	connect(resizeTimer_, &QTimer::timeout, this, [this] { handleDeferredResize(); });
}

FIDString VST3EditorWindow::platformViewType()
{
	return kPlatformTypeX11EmbedWindowID;
}

void VST3EditorWindow::preparePlatformView(const QSize &) {}

tresult VST3EditorWindow::queryPlatformFrameInterface(const TUID iid, void **obj)
{
	if (auto *hostApp = getHostApp(); hostApp && FUnknownPrivate::iidEqual(iid, Linux::IRunLoop::iid)) {
		return hostApp->queryInterface(iid, obj);
	}
	*obj = nullptr;
	return kNoInterface;
}

void VST3EditorWindow::handlePlatformResize(const QSize &)
{
	resizeTimer_->start();
}

qreal VST3EditorWindow::vst3CoordinateScaleFactor() const
{
	if (QGuiApplication::platformName() == QStringLiteral("xcb")) {
		return windowHandle() ? windowHandle()->devicePixelRatio() : devicePixelRatioF();
	}
	return 1.0;
}

bool VST3EditorWindow::usesContentScaleFactor()
{
	return false;
}

bool VST3EditorWindow::nativeEvent(const QByteArray &eventType, void *message, qintptr *result)
{
	return QWidget::nativeEvent(eventType, message, result);
}

// TODO: improve X11 handling. It gave me a lot of headaches. Currently we use a throttle timer to call the plugin
// onSize. Without it I had repaint issues. Known BUG: currently there's still repaint issues with LSP VST3s when
// they are resized with their inner handle but not when the container is resized. Other VST3s on linux are fine.
void VST3EditorWindow::handleDeferredResize()
{
	if (!view_ || !attached_ || resizingFromPlugin_ || correctingHostResize_) {
		return;
	}

	const QSize requestedQtSize = viewContainer_->size();
	ViewRect requestedRect = qtToVst3Rect(requestedQtSize);
	ViewRect constrainedRect = requestedRect;

	tresult constraintResult = kResultFalse;

	if (resizable_) {
		constraintResult = constrainView(&constrainedRect);
	}

	if (constraintResult == kResultTrue && !sameSize(requestedRect, constrainedRect)) {
		const QSize constrainedQtSize = vst3ToQtSize(constrainedRect);

		correctingHostResize_ = true;
		if (constrainedRect.getWidth() > requestedRect.getWidth() &&
		    constrainedRect.getHeight() > requestedRect.getHeight()) {
			setMinimumSize(constrainedQtSize);
		} else {
			resize(constrainedQtSize);
		}
		correctingHostResize_ = false;

		ViewRect actualRect = qtToVst3Rect(viewContainer_->size());
		notifyViewSize(&actualRect);
		return;
	}
	notifyViewSize(&requestedRect);
}
