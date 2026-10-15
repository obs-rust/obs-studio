#pragma once

#include <QMetaObject>
#include <QObject>
#include <QThread>

#include <utility>

/* How OBSStudioAPI runs a frontend API call that changes state the caller
 * can read back right away (creating, duplicating or removing a profile)
 * on the UI thread, where `ui` (the main window) lives:
 * - on the UI thread the call runs at once;
 * - from another thread (an obs-websocket request, a plugin's worker) the
 *   caller waits until the UI thread has run it, so what it reads next (the
 *   profile list, the current profile's settings) is the new state;
 * - when `must_not_wait` is true (libobs's graphics, audio and destroy
 *   threads, which activating a profile can wait for) the call is queued
 *   and the caller does not wait. */
template<typename Fn> void RunFrontendApiCall(QObject *ui, Fn &&fn, bool must_not_wait)
{
	Qt::ConnectionType type = Qt::DirectConnection;
	if (QThread::currentThread() != ui->thread()) {
		type = must_not_wait ? Qt::QueuedConnection : Qt::BlockingQueuedConnection;
	}

	QMetaObject::invokeMethod(ui, std::forward<Fn>(fn), type);
}
