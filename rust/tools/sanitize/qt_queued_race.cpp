/* Negative control for #3809. This deliberately races on application state
 * inside the real WaitForOutputToStop predicate. It is never a normal test.
 * With a configured HYBRID TSan tree in $SAN_BUILD, compile from the repo root:
 * c++ -std=c++17 -O2 -g1 -fsanitize=thread -I. -Ilibobs -I"$SAN_BUILD/config" \
 *   $(pkg-config --cflags Qt6Core) rust/tools/sanitize/qt_queued_race.cpp \
 *   -L"$SAN_BUILD/libobs" -Wl,-rpath,"$SAN_BUILD/libobs" -lobs \
 *   $(pkg-config --libs Qt6Core) -pthread -o "$SAN_BUILD/qt_queued_race"
 * Run with TSAN_OPTIONS=exitcode=66:suppressions=$PWD/rust/tools/sanitize/tsan.supp.
 * Expect exit 66 and a report pairing two accesses to ApplicationState::value,
 * even with the queued-functor allocation suppression enabled. The relaxed
 * flags deliberately do not publish this field; ready only starts the worker.
 */
#include <QCoreApplication>
#include <QTimer>
#include <atomic>
#include <thread>
#include <cstdio>
#include "frontend/widgets/RecordingShutdownWait.hpp"
struct ApplicationState {
	int value = 0;
};
int main(int argc, char **argv)
{
	QCoreApplication app(argc, argv);
	auto *handler = signal_handler_create();
	signal_handler_add(handler, "void stop()");
	signal_handler_add(handler, "void deactivate()");
	ApplicationState state;
	std::atomic<bool> ready{false}, notified{false}, done{false};
	std::thread worker([&] {
		while (!ready.load(std::memory_order_acquire)) {
		}
		notified.store(true, std::memory_order_relaxed);
		calldata_t data;
		calldata_init(&data);
		signal_handler_signal(handler, "stop", &data);
		while (!done.load(std::memory_order_relaxed)) {
			state.value = 1;
		}
		calldata_free(&data);
	});
	QObject receiver;
	QTimer::singleShot(0, &receiver, [&] { ready.store(true, std::memory_order_release); });
	bool stopped = WaitForOutputToStop(handler, 2000, [&] {
		if (!notified.load(std::memory_order_relaxed)) {
			return true;
		}
		state.value = 2;
		std::printf("application field %d\n", state.value);
		done.store(true, std::memory_order_relaxed);
		return false;
	});
	done.store(true, std::memory_order_relaxed);
	worker.join();
	signal_handler_destroy(handler);
	return stopped ? 0 : 1;
}
