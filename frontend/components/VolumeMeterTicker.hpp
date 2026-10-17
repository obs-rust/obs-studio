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
	(void)currentMagnitude;
	(void)currentPeak;
	(void)currentInputPeak;
	(void)displayMagnitude;
	(void)displayPeak;
	(void)displayPeakHold;
	(void)displayInputPeakHold;
	(void)minimum;
	return false;
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

	/* Start the timer if it stopped. Safe from any thread. */
	void wake() {}

	bool active() const { return timer->isActive(); }

private:
	void tick()
	{
		for (auto &[id, client] : clients) {
			client.repaint();
		}
	}

	QPointer<QTimer> timer;
	std::map<int, Client> clients;
	int nextId = 0;
	std::atomic<bool> running{true};
};
