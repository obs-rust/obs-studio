#pragma once

/* The window of a floating dock (obs-rust/obs-studio#5409, #5416).
 * Header-only, Qt Widgets only, so it can be unit tested without the
 * frontend. */

#include <QDockWidget>

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
 * dock ends, this clears the copy. Call it once per dock. */
inline void LayOutAfterFloatingMove(QDockWidget *dock)
{
	(void)dock;
}

/* Everything OBS sets up on the window of a dock. Call it before the dock first
 * floats. */
inline void SetUpDockWindow(QDockWidget *dock)
{
	KeepFloatingDockShown(dock);
	LayOutAfterFloatingMove(dock);
}
