#pragma once

/* The window of a floating dock (obs-rust/obs-studio#5409, #5416, #5408).
 * Header-only, Qt Widgets only, so it can be unit tested without the
 * frontend. */

#include <QCloseEvent>
#include <QDockWidget>
#include <QEvent>
#include <QKeySequence>
#include <QMainWindow>
#include <QShortcut>

/* A floating dock is a tool window. On macOS, Qt makes a tool window a panel
 * that hides while the application is inactive, so a dock on a second display
 * disappeared whenever another app was in front, while the main window stayed.
 * Keep it shown, as the main window is. The attribute does nothing on other
 * platforms. Call it before the dock first floats. */
inline void KeepFloatingDockShown(QDockWidget *dock)
{
	dock->setAttribute(Qt::WA_MacAlwaysShowToolWindow);
}

/* While a dock is dragged, Qt keeps a copy of the main window's layout, to put
 * back if the drag does not dock it. A drag that ends outside the window
 * leaves the copy set (QMainWindowLayout::restore(QInternal::KeepSavedState)),
 * and on macOS every move of a floating dock's window ends so. While the copy
 * is set, QMainWindowLayout::setGeometry() does nothing: the docks no longer
 * followed a resize of the main window, and a dock docked again kept its
 * floating screen position, outside the window. When the move of a floating
 * dock ends, this clears the copy. A second call does nothing (an OBSDock is
 * set up when it is made and again in AddDockWidget()). */
class FloatingDockMoveEnd : public QObject {
public:
	explicit FloatingDockMoveEnd(QDockWidget *dock) : QObject(dock), dock(dock) {}

protected:
	bool eventFilter(QObject *obj, QEvent *event) override
	{
		if (obj != dock) {
			return false;
		}
		switch (event->type()) {
		case QEvent::NonClientAreaMouseButtonPress:
		case QEvent::MouseButtonPress:
			pressed = true;
			moved = false;
			break;
		case QEvent::Move:
			moved = moved || (pressed && dock->isFloating());
			break;
		case QEvent::NonClientAreaMouseButtonRelease:
		case QEvent::MouseButtonRelease:
			/* Not during the drag: restoreState() deletes the layout
			 * items, and the drag still holds one. */
			if (moved) {
				Clear();
			}
			pressed = false;
			moved = false;
			break;
		default:
			break;
		}
		return false;
	}

private:
	/* After Qt ended the drag: restoreState() clears the copy, saveState()
	 * keeps the layout as it is. restoreState() hides and shows each
	 * floating dock, so a floating dock blinks once when a move ends; Qt
	 * has no other public way to clear the copy. */
	void Clear()
	{
		QMetaObject::invokeMethod(
			this,
			[this]() {
				auto *window = qobject_cast<QMainWindow *>(dock->parentWidget());
				if (window && dock->isFloating()) {
					window->restoreState(window->saveState());
				}
			},
			Qt::QueuedConnection);
	}

	QDockWidget *dock;
	bool pressed = false;
	bool moved = false;
};

inline void LayOutAfterFloatingMove(QDockWidget *dock)
{
	static const char *const installed = "obsFloatingDockMoveEnd";
	if (dock->property(installed).toBool()) {
		return;
	}
	dock->setProperty(installed, true);
	dock->installEventFilter(new FloatingDockMoveEnd(dock));
}

/* A main dock is not closable. A close of its floating window (the window
 * manager's close button or key, or the dock close key below) puts it back
 * where it was docked. Returns true when it did; the caller then does no more
 * with the event. While the main window is hidden or OBS is closing (a quit
 * closes the main window first, obs-rust/obs-studio#5406), the dock closes as
 * before and keeps its floating state for the next start. */
inline bool CloseRedocksMainDock(QDockWidget *dock, QCloseEvent *event, bool closing)
{
	(void)dock;
	(void)event;
	(void)closing;
	return false;
}

/* The close key (Cmd+W on macOS, Ctrl+W or Ctrl+F4 elsewhere) closes a
 * floating dock, as it closes a window in other apps. The shortcut belongs to
 * the dock and works only while the dock floats, so in the main window the key
 * does nothing and an OBS hotkey on it still works. */
inline QShortcut *AddFloatingDockCloseKey(QDockWidget *dock)
{
	(void)dock;
	return nullptr;
}

/* Everything OBS sets up on the window of a dock. Call it before the dock first
 * floats. */
inline void SetUpDockWindow(QDockWidget *dock)
{
	KeepFloatingDockShown(dock);
	LayOutAfterFloatingMove(dock);
	AddFloatingDockCloseKey(dock);
}
