#pragma once

/* The window of a dock (obs-rust/obs-studio#5409, #5416, #5408, #5457).
 * Header-only, Qt Widgets only, so it can be unit tested without the
 * frontend. */

#include <QCloseEvent>
#include <QDockWidget>
#include <QEvent>
#include <QKeySequence>
#include <QMainWindow>
#include <QShortcut>
#include <QVariant>

/* What OBS adds to the window of a dock: one per dock, a child of it, made by
 * SetUp(). Each part works around Qt behavior or adds what Qt does not have;
 * check a part again when Qt changes:
 *
 * - macOS panel (#5409, Qt 6.11): Qt makes a floating dock, a Qt::Tool
 *   window, an NSPanel that hides while OBS is not the active app, so a dock
 *   on a second display disappeared. Qt::WA_MacAlwaysShowToolWindow keeps it
 *   shown, as the main window is. It does nothing on other platforms.
 * - Stale drag layout (#5416, Qt 6.11.1, qmainwindowlayout.cpp and
 *   qdockwidget.cpp): while a dock is dragged, QMainWindowLayout keeps a copy
 *   of the layout (savedState) to put back if the drag does not dock it. A
 *   drag that ends outside the window leaves the copy set
 *   (restore(QInternal::KeepSavedState)), and on macOS every move of a
 *   floating dock's window ends so. While it is set,
 *   QMainWindowLayout::setGeometry() does nothing: the docks no longer
 *   followed a resize, and a dock docked again kept its floating screen
 *   position. After the move, restoreState(saveState()) clears the copy and
 *   keeps the layout. It hides and shows each floating dock, so a floating
 *   dock blinks once; Qt has no other public way to clear the copy.
 * - Close key (#5408): the close key (Cmd+W on macOS, Ctrl+W or Ctrl+F4
 *   elsewhere) closes the floating dock that has the focus, as it closes a
 *   window in other apps. The shortcut belongs to the dock and works only
 *   while the dock floats, so in the main window the key does nothing and an
 *   OBS hotkey on it still works. */
class DockWindow : public QObject {
public:
	/* Sets `dock` up once and returns its DockWindow; a second call returns
	 * the same one (an OBSDock is set up when it is made and again in
	 * AddDockWidget(); two shortcuts on one key are ambiguous, and Qt then
	 * activates neither). Call it before the dock first floats. */
	static DockWindow *SetUp(QDockWidget *dock)
	{
		static const char *const property = "obsDockWindow";
		if (auto *existing = dock->property(property).value<QObject *>()) {
			return static_cast<DockWindow *>(existing);
		}
		auto *window = new DockWindow(dock);
		dock->setProperty(property, QVariant::fromValue<QObject *>(window));
		return window;
	}

	QShortcut *CloseKey() const { return closeKey; }

protected:
	bool eventFilter(QObject *obj, QEvent *event) override
	{
		if (obj == dock) {
			TrackMove(event->type());
		}
		return false;
	}

private:
	explicit DockWindow(QDockWidget *dock) : QObject(dock), dock(dock), closeKey(new QShortcut(dock))
	{
		setObjectName(QStringLiteral("dockWindow"));
		dock->setAttribute(Qt::WA_MacAlwaysShowToolWindow);
		dock->installEventFilter(this);

		closeKey->setObjectName(QStringLiteral("floatingDockCloseKey"));
		closeKey->setKeys(QKeySequence::Close);
		closeKey->setContext(Qt::WindowShortcut);
		closeKey->setEnabled(dock->isFloating());
		connect(dock, &QDockWidget::topLevelChanged, closeKey, &QShortcut::setEnabled);
		connect(closeKey, &QShortcut::activated, dock, [dock]() { dock->close(); });
	}

	/* A press on the dock (its window frame or title bar), a move of its
	 * window while it floats, and the release: the move ended. */
	void TrackMove(QEvent::Type type)
	{
		switch (type) {
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
			if (moved) {
				ClearDragLayout();
			}
			pressed = false;
			moved = false;
			break;
		default:
			break;
		}
	}

	/* Queued, so after Qt ended the drag: restoreState() deletes the layout
	 * items, and a running drag still holds one. */
	void ClearDragLayout()
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
	QShortcut *closeKey;
	bool pressed = false;
	bool moved = false;
};

/* A main dock is not closable. A close of its floating window (the window
 * manager's close button or key, or the close key of DockWindow) puts it back
 * where it was docked. Returns true when it did; the caller then does no more
 * with the event. While the main window is hidden or OBS is closing (a quit
 * closes the main window first, obs-rust/obs-studio#5406), the dock closes as
 * before and keeps its floating state for the next start. */
inline bool CloseRedocksMainDock(QDockWidget *dock, QCloseEvent *event, bool closing)
{
	QWidget *main = dock->parentWidget();
	if ((dock->features() & QDockWidget::DockWidgetClosable) || !dock->isFloating() || closing || !main ||
	    !main->isVisible()) {
		return false;
	}

	event->ignore();
	/* Not inside the close event: setFloating() recreates the dock's
	 * window. */
	QMetaObject::invokeMethod(dock, [dock]() { dock->setFloating(false); }, Qt::QueuedConnection);
	return true;
}
