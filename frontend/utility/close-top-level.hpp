#pragma once

/* OBSBasic::close() for a main window that may never have been shown
 * (obs-rust/obs-studio#1400). Header-only, Qt Widgets only, so it can be
 * unit tested without the frontend. */

#include <QApplication>
#include <QCloseEvent>
#include <QPointer>
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

/* An application quit (QEvent::Quit) closes the main window first
 * (obs-rust/obs-studio#5406).
 *
 * Cmd+Q, logoff, shutdown and QCoreApplication::quit() reach the application
 * as QEvent::Quit. Qt then closes the visible top-level windows in no fixed
 * order and cancels the quit when one of them refuses its close. The main
 * docks are not closable: QDockWidget refuses their close while the main
 * window is visible, so a floating main dock that came first cancelled the
 * whole quit, and OBS stayed open.
 *
 * Installed on the application: the main window gets its close first, as if
 * the user had closed it (OBSBasic::closeEvent() saves, stops the outputs and
 * may ask). If it refuses, the quit is cancelled, as Qt cancels it. Once the
 * main window is hidden, Qt closes the main docks in any order. A hidden main
 * window (minimized to the tray) is left to Qt. */
class QuitClosesMainWindowFirst : public QObject {
public:
	QuitClosesMainWindowFirst(QWidget *main, QObject *parent) : QObject(parent), main(main) {}

protected:
	bool eventFilter(QObject *obj, QEvent *event) override
	{
		if (obj != qApp || event->type() != QEvent::Quit || !main || !main->isVisible()) {
			return false;
		}
		if (!CloseTopLevel(main)) {
			event->ignore();
			return true;
		}
		return false;
	}

private:
	/* OBSBasic deletes itself on close. */
	QPointer<QWidget> main;
};
