#include "frontend/build/frontend-camera-busy.pch.hpp"

namespace CameraBusy {

#ifdef OBS_FRONTEND_RUST

/* A list from the C ABI, freed with it. */
struct HoldersDeleter {
	void operator()(obs_camera_busy_holders_t *h) const { obs_camera_busy_holders_free(h); }
};
using HolderList = std::shared_ptr<obs_camera_busy_holders_t>;

static HolderList cb_list(obs_camera_busy_holders_t *h)
{
	return HolderList(h, HoldersDeleter());
}

/* A string from the C ABI as a std::string, freed. */
static std::string cb_take(char *s)
{
	std::string out = s ? s : "";
	obs_camera_busy_free(s);
	return out;
}

struct Report {
	int cause;
	std::string device;
	HolderList holders;
};

struct Watch {
	obs_camera_busy_watch_t *watch = nullptr;
	/* The callback's param: lives as long as the watch. */
	std::unique_ptr<std::string> device;
	int refs = 0;
};

class WindowFilter;

/* UI thread only. */
static struct {
	QPointer<QWidget> window;
	Texts texts;
	SaveAllowlist save = nullptr;
	void *saveParam = nullptr;
	bool ready = false;
	/* Waiting for the main window to be shown or restored. */
	std::deque<Report> pending;
	std::map<std::string, Watch> watches;
	WindowFilter *filter = nullptr;
} cb_state;

/* Any thread: reports after shutdown do nothing. */
static std::atomic<bool> cb_shut_down{false};

static void cb_pump();

/* Shows the waiting warnings when the main window comes back. */
class WindowFilter : public QObject {
public:
	using QObject::QObject;

protected:
	bool eventFilter(QObject *watched, QEvent *event) override
	{
		switch (event->type()) {
		case QEvent::Show:
		case QEvent::WindowStateChange:
		case QEvent::WindowActivate:
			QTimer::singleShot(0, qApp, cb_pump);
			break;
		default:
			break;
		}
		return QObject::eventFilter(watched, event);
	}
};

static uint64_t cb_now_ms()
{
	return os_gettime_ns() / 1000000;
}

static bool cb_window_shown()
{
	QWidget *w = cb_state.window;
	return w && w->isVisible() && !w->isMinimized();
}

static void cb_answered(const Report &r, const HolderList &shown, OBSCameraBusy::Answer answer)
{
	const char *device = r.device.c_str();
	int code = (int)answer;
	blog(LOG_INFO, "%s", cb_take(obs_camera_busy_answer_log_line(device, shown.get(), code)).c_str());
	if (obs_camera_busy_end(device, shown.get(), code, cb_now_ms()) && cb_state.save) {
		cb_state.save(cb_state.saveParam, cb_take(obs_camera_busy_allowlist()).c_str());
	}
	if (answer != OBSCameraBusy::Answer::CloseIt) {
		return;
	}
	/* SIGTERM, then SIGKILL after the grace: seconds, so not here. The
	 * source's busy check (#800) opens the camera once the holder is gone. */
	std::thread([shown]() {
		size_t n = obs_camera_busy_holders_count(shown.get());
		for (size_t i = 0; i < n; i++) {
			if (!obs_camera_busy_holder_can_close(shown.get(), i)) {
				continue;
			}
			const char *name = obs_camera_busy_holder_name(shown.get(), i);
			uint32_t pid = obs_camera_busy_holder_pid(shown.get(), i);
			int closed = obs_camera_busy_close(pid);
			blog(closed == OBS_CAMERA_BUSY_FAILED ? LOG_WARNING : LOG_INFO, "%s",
			     cb_take(obs_camera_busy_close_log_line(name, pid, closed)).c_str());
		}
	}).detach();
}

static void cb_open(const Report &r, const HolderList &shown)
{
	size_t n = obs_camera_busy_holders_count(shown.get());
	bool closable = false;
	for (size_t i = 0; i < n; i++) {
		closable = closable || obs_camera_busy_holder_can_close(shown.get(), i);
	}
	QString device = QString::fromStdString(r.device);
	QString list = QString::fromStdString(cb_take(obs_camera_busy_holder_list(shown.get())));

	OBSCameraBusy::Texts t;
	t.title = cb_state.texts.title;
	if (n == 0) {
		t.message = cb_state.texts.busyUnknown.arg(device);
	} else if (r.cause == OBS_CAMERA_BUSY_CAUSE_OPENED) {
		t.message = cb_state.texts.opened.arg(list, device);
	} else {
		t.message = cb_state.texts.busy.arg(device, list);
	}
	t.closeIt = closable ? cb_state.texts.closeIt : QString();
	t.ignore = cb_state.texts.ignore;
	t.alwaysAllow = n ? cb_state.texts.alwaysAllow : QString();

	/* In the background: do not take the focus from the user's app. */
	bool active = cb_state.window->isActiveWindow();
	OBSCameraBusy *dialog = new OBSCameraBusy(cb_state.window, t, active);
	auto answered = std::make_shared<bool>(false);
	QObject::connect(dialog, &OBSCameraBusy::answered, qApp, [r, shown, answered](OBSCameraBusy::Answer answer) {
		*answered = true;
		cb_answered(r, shown, answer);
	});
	/* Deleted without an answer (its main window went away first): Ignore,
	 * so the device and holder do not stay open for good. */
	QObject::connect(dialog, &QObject::destroyed, qApp, [r, shown, answered]() {
		if (!*answered) {
			obs_camera_busy_end(r.device.c_str(), shown.get(), OBS_CAMERA_BUSY_IGNORE, cb_now_ms());
		}
	});
	dialog->show();
	if (!active) {
		QApplication::alert(cb_state.window);
	}
}

/* UI thread: decide, log, and show or wait. */
static void cb_handle(const Report &r)
{
	int show = OBS_CAMERA_BUSY_NO_HOLDER;
	HolderList shown =
		cb_list(obs_camera_busy_begin(r.cause, r.device.c_str(), r.holders.get(), cb_now_ms(), &show));
	/* OBS reopening its own camera, or a program that only probed it. */
	if (r.cause == OBS_CAMERA_BUSY_CAUSE_OPENED && show == OBS_CAMERA_BUSY_NO_HOLDER) {
		return;
	}
	blog(show == OBS_CAMERA_BUSY_DIALOG ? LOG_WARNING : LOG_INFO, "%s",
	     cb_take(obs_camera_busy_log_line(r.cause, r.device.c_str(), shown.get(), show)).c_str());
	if (show == OBS_CAMERA_BUSY_DIALOG) {
		cb_open(r, shown);
	}
}

static void cb_pump()
{
	if (!cb_state.ready || !cb_window_shown()) {
		return;
	}
	while (!cb_state.pending.empty()) {
		Report r = std::move(cb_state.pending.front());
		cb_state.pending.pop_front();
		cb_handle(r);
	}
}

/* Any thread. */
static void cb_report(int cause, std::string device, HolderList holders)
{
	if (cb_shut_down) {
		return;
	}
	QMetaObject::invokeMethod(
		qApp,
		[cause, device = std::move(device), holders = std::move(holders)]() {
			if (cb_shut_down) {
				return;
			}
			cb_state.pending.push_back({cause, device, holders});
			cb_pump();
		},
		Qt::QueuedConnection);
}

/* The watcher thread. */
static void cb_watch_cb(void *param, obs_camera_busy_holders_t *holders)
{
	cb_report(OBS_CAMERA_BUSY_CAUSE_OPENED, *static_cast<std::string *>(param), cb_list(holders));
}

static void cb_unwatch_all()
{
	for (auto &[device, w] : cb_state.watches) {
		obs_camera_busy_unwatch(w.watch);
	}
	cb_state.watches.clear();
}

void SetMainWindow(QWidget *window, const Texts &texts, const char *allowlist, SaveAllowlist save, void *param)
{
	if (cb_state.window && cb_state.filter) {
		cb_state.window->removeEventFilter(cb_state.filter);
	}
	delete cb_state.filter;
	cb_state.filter = nullptr;

	cb_shut_down = !window;
	cb_state.window = window;
	cb_state.texts = texts;
	cb_state.save = save;
	cb_state.saveParam = param;
	cb_state.ready = window != nullptr;
	if (!window) {
		cb_unwatch_all();
		cb_state.pending.clear();
		return;
	}
	obs_camera_busy_set_allowlist(allowlist ? allowlist : "");
	cb_state.filter = new WindowFilter(qApp);
	window->installEventFilter(cb_state.filter);
	cb_pump();
}

bool Busy(const char *device)
{
	if (cb_shut_down) {
		return true;
	}
	std::string dev = device ? device : "";
	/* /proc is scanned off the thread that reported it (often the UI). */
	std::thread([dev]() {
		cb_report(OBS_CAMERA_BUSY_CAUSE_BUSY, dev, cb_list(obs_camera_busy_scan(dev.c_str())));
	}).detach();
	return true;
}

void DeviceOpened(const char *device)
{
	if (cb_shut_down || !device || !*device) {
		return;
	}
	Watch &w = cb_state.watches[device];
	if (w.refs++ == 0) {
		w.device = std::make_unique<std::string>(device);
		w.watch = obs_camera_busy_watch(device, cb_watch_cb, w.device.get());
		if (!w.watch) {
			blog(LOG_INFO, "[camera-busy] cannot watch %s for other programs opening it", device);
		}
	}
}

void DeviceClosed(const char *device)
{
	auto it = device ? cb_state.watches.find(device) : cb_state.watches.end();
	if (it == cb_state.watches.end()) {
		return;
	}
	if (--it->second.refs <= 0) {
		obs_camera_busy_unwatch(it->second.watch);
		cb_state.watches.erase(it);
	}
}

#else

/* The C rollback build: no Rust, so the caller keeps its own popup. */
void SetMainWindow(QWidget *, const Texts &, const char *, SaveAllowlist, void *) {}

bool Busy(const char *)
{
	return false;
}

void DeviceOpened(const char *) {}

void DeviceClosed(const char *) {}

#endif

} // namespace CameraBusy
