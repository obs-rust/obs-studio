#pragma once

#include <atomic>
#include <chrono>
#include <cstdint>
#include <functional>

#include <QEventLoop>
#include <QMetaObject>
#include <QObject>
#include <QTimer>

#include <callback/signal.h>

/* Waits, at most timeout_ms, for an output that was asked to stop to finish
 * stopping: StopRecordingForShutdown()'s wait step in
 * OBSBasic::closeWindow(). Returns whether it stopped. active() reports
 * whether the output is still active; output_signals is the output's signal
 * handler.
 *
 * closeWindow() needs the recording finalized before it goes on to destroy
 * the outputs, so the wait is synchronous for its caller. It used to sleep
 * on the UI thread in 10 ms steps, which froze the window for up to the
 * whole timeout (#1283). Instead it runs a nested QEventLoop that ends on the
 * output's signals or on a single-shot timer at the timeout, so repaints,
 * timers and queued events keep running. User input stays excluded, as the
 * window is closing.
 *
 * A graceful stop of an active output signals "stop" while the output is
 * still flushing its encoders, then "deactivate" when it is done, just
 * before it reports inactive; an output that never began capturing signals
 * only "stop", and is already inactive. So the wait ends on "deactivate", or
 * on "stop" once active() is false. libobs signals on its own threads: the
 * callback only records the signal and queues the check to the UI thread. */
inline bool WaitForOutputToStop(signal_handler_t *output_signals, uint64_t timeout_ms,
				const std::function<bool()> &active)
{
	struct State {
		QEventLoop loop;
		QObject receiver; /* lives on this (the UI) thread */
		std::atomic<bool> deactivated{false};
		const std::function<bool()> *active = nullptr;

		bool Stopped() const { return deactivated.load() || !(*active)(); }
		void Check()
		{
			if (Stopped()) {
				loop.quit();
			}
		}
		void Queue()
		{
			QMetaObject::invokeMethod(&receiver, [this]() { Check(); }, Qt::QueuedConnection);
		}
	};

	auto on_stop = [](void *data, calldata_t *) {
		static_cast<State *>(data)->Queue();
	};
	auto on_deactivate = [](void *data, calldata_t *) {
		State *state = static_cast<State *>(data);
		state->deactivated = true;
		state->Queue();
	};

	State state;
	state.active = &active;

	signal_handler_connect(output_signals, "stop", on_stop, &state);
	signal_handler_connect(output_signals, "deactivate", on_deactivate, &state);

	/* it may have stopped before the handlers were connected */
	if (!state.Stopped()) {
		QTimer timeout;
		timeout.setSingleShot(true);
		timeout.setTimerType(Qt::PreciseTimer);
		QObject::connect(&timeout, &QTimer::timeout, &state.loop, &QEventLoop::quit);
		timeout.start(std::chrono::milliseconds(timeout_ms));
		state.loop.exec(QEventLoop::ExcludeUserInputEvents);
	}

	/* after this no callback runs, and the receiver's pending checks are
	 * dropped with it */
	signal_handler_disconnect(output_signals, "stop", on_stop, &state);
	signal_handler_disconnect(output_signals, "deactivate", on_deactivate, &state);

	return state.Stopped();
}
