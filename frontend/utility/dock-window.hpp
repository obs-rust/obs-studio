#pragma once

/* The window of a floating dock (obs-rust/obs-studio#5409). Header-only, Qt
 * Widgets only, so it can be unit tested without the frontend. */

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
