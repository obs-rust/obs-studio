#pragma once

/* OBSBasic::close() for a main window that may never have been shown
 * (obs-rust/obs-studio#1400). Header-only, Qt Widgets only, so it can be
 * unit tested without the frontend. */

#include <QApplication>
#include <QCloseEvent>
#include <QWidget>
#include <QWindow>

/* Closes the top-level widget: sends it a close event, which the widget may
 * accept or ignore. Returns whether the close event was accepted.
 *
 * A widget that was created but never shown can have a QWindow with no
 * platform window yet (OBS started with --minimize-to-tray on Wayland). For a
 * top-level widget QWidget::close() asks QWindow::close(), which returns true
 * without a close event when there is no platform window, and QWidget then
 * skips its own close handling: nothing closes. Send the close event
 * directly in that case. Only then: the normal path also does the widget's
 * own close handling (hiding it, emitting the window's close signals), which
 * a window with nothing on screen does not need. */
inline bool CloseTopLevel(QWidget *widget)
{
	QWindow *window = widget->windowHandle();
	if (window && !window->handle()) {
		QCloseEvent closeEvent;
		QApplication::sendEvent(widget, &closeEvent);
		return closeEvent.isAccepted();
	}

	return widget->close();
}
