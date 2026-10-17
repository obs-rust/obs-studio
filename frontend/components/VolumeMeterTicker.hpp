#pragma once

/* Repaint scheduling for the volume meters (obs-rust/obs-studio#1281). The
 * meters share one 16 ms timer on the UI thread. It used to repaint every
 * meter 60 times a second forever, also while the mixer was hidden, OBS was
 * minimized or every source was silent. VolumeMeterTicker repaints only the
 * meters that want it and stops when none does; wake() starts it again. */

#include <QMetaObject>
#include <QPointer>
#include <QTimer>

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstddef>
#include <functional>
#include <map>

/* True when another repaint would draw the same meter: every displayed level
 * has caught up with its input (levels below the meter's floor `minimum`
 * count as equal), so no decay, integration or peak hold is still moving. */
template<std::size_t N>
bool volumeMeterSettled(const float (&currentMagnitude)[N], const float (&currentPeak)[N],
			const float (&currentInputPeak)[N], const float (&displayMagnitude)[N],
			const float (&displayPeak)[N], const float (&displayPeakHold)[N],
			const float (&displayInputPeakHold)[N], float minimum)
{
	// Within a tenth of a dB the meter draws the same pixels.
	const float epsilon = 0.1f;
	auto same = [&](float display, float current) {
		if (std::isnan(display) || std::isnan(current)) {
			return false;
		}
		if (display <= minimum && current <= minimum) {
			return true;
		}
		return std::fabs(display - std::max(current, minimum)) < epsilon;
	};
	for (std::size_t i = 0; i < N; i++) {
		if (!same(displayMagnitude[i], currentMagnitude[i]) || !same(displayPeak[i], currentPeak[i]) ||
		    !same(displayPeakHold[i], currentPeak[i]) || !same(displayInputPeakHold[i], currentInputPeak[i])) {
			return false;
		}
	}
	return true;
}

class VolumeMeterTicker {
public:
	struct Client {
		/* Visible and not settled. Called on the UI thread. */
		std::function<bool()> wantsRepaint;
		std::function<void()> repaint;
	};

	/* `parent` owns the timer; the ticker must outlive it. */
	explicit VolumeMeterTicker(QObject *parent, int intervalMs = 16) : timer(new QTimer(parent))
	{
		timer->setTimerType(Qt::PreciseTimer);
		timer->setInterval(intervalMs);
		QObject::connect(timer, &QTimer::timeout, timer, [this]() { tick(); });
		timer->start();
	}

	/* A timer whose parent already deleted it is not deleted again. */
	~VolumeMeterTicker() { delete timer; }

	VolumeMeterTicker(const VolumeMeterTicker &) = delete;
	VolumeMeterTicker &operator=(const VolumeMeterTicker &) = delete;

	int add(Client client)
	{
		int id = nextId++;
		clients[id] = std::move(client);
		return id;
	}

	void remove(int id) { clients.erase(id); }

	/* Start the timer if it stopped. Safe from any thread: the start is
	 * queued to the timer's thread, and only when the timer was stopped. */
	void wake()
	{
		if (running.exchange(true)) {
			return;
		}
		QMetaObject::invokeMethod(
			timer,
			[t = timer]() {
				if (t && !t->isActive()) {
					t->start();
				}
			},
			Qt::QueuedConnection);
	}

	bool active() const { return timer->isActive(); }

private:
	void tick()
	{
		// Clear the flag before asking the meters: a wake() that comes
		// while they are asked either sees false and queues a start, or
		// set its new level before the meter is asked and is seen here.
		running = false;
		bool any = false;
		for (auto &[id, client] : clients) {
			if (client.wantsRepaint()) {
				client.repaint();
				any = true;
			}
		}
		if (any) {
			running = true;
		} else {
			timer->stop();
		}
	}

	QPointer<QTimer> timer;
	std::map<int, Client> clients;
	int nextId = 0;
	std::atomic<bool> running{true};
};
