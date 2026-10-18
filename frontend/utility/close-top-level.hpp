#pragma once

/* OBSBasic::close() for a main window that may never have been shown
 * (obs-rust/obs-studio#1400). Header-only, Qt Widgets only, so it can be
 * unit tested without the frontend. */

#include <QWidget>

/* Closes the top-level widget: sends it a close event, which the widget may
 * accept or ignore. Returns whether the close event was accepted. */
inline bool CloseTopLevel(QWidget *widget)
{
	return widget->close();
}
