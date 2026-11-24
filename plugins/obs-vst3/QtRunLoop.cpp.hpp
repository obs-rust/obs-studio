// SPDX-License-Identifier: GPL-3.0-or-later
#include "plugins/obs-vst3/build/obs-vst3.linux.pch.hpp"

#ifdef OBS_VST3_RUST_HOST
DEF_CLASS_IID(Steinberg::Linux::IRunLoop)

class QtRunLoop final : public QObject {
public:
	QtRunLoop() : rust(vst3_rust_run_loop_create(reinterpret_cast<uintptr_t>(this), watchFd, watchTimer, unwatch))
	{
	}

	~QtRunLoop() override
	{
		for (auto &[token, object] : sources) {
			object->disconnect(this);
		}
		vst3_rust_run_loop_destroy(rust);
	}

	Steinberg::Linux::IRunLoop *interface() const { return vst3_rust_run_loop_interface(rust); }

private:
	static QtRunLoop &fromContext(uintptr_t context) { return *reinterpret_cast<QtRunLoop *>(context); }

	static void watchFd(uintptr_t context, uint64_t token, int fd)
	{
		auto &self = fromContext(context);
		auto *notifier = new QSocketNotifier(fd, QSocketNotifier::Read, &self);
		self.sources.emplace(token, notifier);
		QObject::connect(notifier, &QSocketNotifier::activated, &self,
				 [&self, token, fd] { vst3_rust_run_loop_dispatch(self.rust, token, fd); });
	}

	static void watchTimer(uintptr_t context, uint64_t token, int interval)
	{
		auto &self = fromContext(context);
		auto *timer = new QTimer(&self);
		timer->setInterval(interval);
		self.sources.emplace(token, timer);
		QObject::connect(timer, &QTimer::timeout, &self,
				 [&self, token] { vst3_rust_run_loop_dispatch(self.rust, token, -1); });
		timer->start();
	}

	static void unwatch(uintptr_t context, uint64_t token)
	{
		auto &self = fromContext(context);
		auto at = self.sources.find(token);
		if (at == self.sources.end()) {
			return;
		}
		auto *source = at->second;
		self.sources.erase(at);
		// Disconnect immediately. Rust also rejects token deliveries already queued.
		source->disconnect(&self);
		if (auto *timer = qobject_cast<QTimer *>(source)) {
			timer->stop();
		} else if (auto *notifier = qobject_cast<QSocketNotifier *>(source)) {
			notifier->setEnabled(false);
		}
		source->deleteLater();
	}

	void *rust;
	std::map<uint64_t, QObject *> sources;
};
#endif
