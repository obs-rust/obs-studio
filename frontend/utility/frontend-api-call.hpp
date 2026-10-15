#pragma once

#include <QMetaObject>
#include <QObject>
#include <QThread>

#include <utility>

/* How OBSStudioAPI runs a frontend API call that changes state the caller
 * can read back right away (creating, duplicating or removing a profile)
 * on the UI thread, where `ui` (the main window) lives. `must_not_wait` is
 * true on libobs's own threads (graphics, audio, destroy), which activating
 * a profile can wait for. */
template<typename Fn> void RunFrontendApiCall(QObject *ui, Fn &&fn, bool must_not_wait)
{
	(void)must_not_wait;
	QMetaObject::invokeMethod(ui, std::forward<Fn>(fn));
}
