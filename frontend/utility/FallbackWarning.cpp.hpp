#include "frontend/build/frontend-fallback-warning.pch.hpp"

namespace FallbackWarning {

struct FwRequest {
	std::string id;
	std::string title;
	std::string message;
	bool can_cancel;
	Callback cb;
	void *param;
};

/* UI thread only. */
static struct {
	QPointer<QWidget> window;
	Texts texts;
	/* SetMainWindow() was called: warnings run instead of waiting. */
	bool ready = false;
	std::deque<FwRequest> pending;
	QPointer<OBSFallbackWarning> open;
} fw_state;

static const char *fw_text(const char *s)
{
	return s ? s : "";
}

static void fw_answer(const FwRequest &r, bool accepted)
{
	if (r.cb) {
		r.cb(r.param, accepted);
	}
}

static void fw_log(const char *id, const char *message)
{
#ifdef OBS_FRONTEND_RUST
	char *line = obs_fallback_warning_log_line(id, message);
	if (line) {
		blog(LOG_ERROR, "%s", line);
		obs_fallback_warning_free(line);
	}
#else
	/* The C rollback build: the line of rust/obs-fallback-warning's log_line(). */
	const char *future_error = "This may become an error in a future version of OBS.";
	if (*message) {
		blog(LOG_ERROR, "[fallback] %s: %s %s", id, message, future_error);
	} else {
		blog(LOG_ERROR, "[fallback] %s: %s", id, future_error);
	}
#endif
}

static void fw_pump();

#ifdef OBS_FRONTEND_RUST
static void fw_open(const FwRequest &r)
{
	OBSFallbackWarning::Texts t;
	t.title = QString::fromStdString(r.title);
	t.message = QString::fromStdString(r.message);
	t.futureError = fw_state.texts.futureError;
	t.accept = fw_state.texts.accept;
	t.cancel = r.can_cancel ? fw_state.texts.cancel : QString();

	OBSFallbackWarning *dialog =
		new OBSFallbackWarning(fw_state.window, t, (qint64)obs_fallback_warning_countdown_ms());
	auto answered = std::make_shared<bool>(false);
	QObject::connect(dialog, &OBSFallbackWarning::ended, qApp, [r, answered](OBSFallbackWarning::End end) {
		*answered = true;
		fw_state.open = nullptr;
		fw_answer(r, obs_fallback_warning_accepted((int)end, r.can_cancel));
		/* The next one after this dialog has closed. */
		QTimer::singleShot(0, qApp, fw_pump);
	});
	/* Deleted without an end (its main window went away first): it closed. */
	QObject::connect(dialog, &QObject::destroyed, qApp, [r, answered]() {
		if (*answered) {
			return;
		}
		*answered = true;
		fw_answer(r, obs_fallback_warning_accepted(OBS_FALLBACK_WARNING_END_CLOSED, r.can_cancel));
		QTimer::singleShot(0, qApp, fw_pump);
	});
	fw_state.open = dialog;
	dialog->show();
}
#endif

#ifdef OBS_FRONTEND_RUST
/* How often the queue looks again while another modal dialog is open. */
static constexpr int fw_modal_retry_ms = 250;
static bool fw_retry_queued = false;

static void fw_retry_pump()
{
	fw_retry_queued = false;
	fw_pump();
}
#endif

/* Runs the queued warnings until one shows a dialog. */
static void fw_pump()
{
	if (!fw_state.ready || fw_state.open) {
		return;
	}
	while (!fw_state.pending.empty()) {
#ifdef OBS_FRONTEND_RUST
		bool visible = fw_state.window && fw_state.window->isVisible();
		/* Another modal dialog (the first-run wizard opens right after
		 * the main window has loaded) would hide the warning or take its
		 * input, and it would accept unseen: wait until it has closed. The
		 * warning is not counted as shown before it shows. */
		if (visible && QApplication::activeModalWidget()) {
			if (!fw_retry_queued) {
				fw_retry_queued = true;
				QTimer::singleShot(fw_modal_retry_ms, qApp, fw_retry_pump);
			}
			return;
		}
#endif
		FwRequest r = std::move(fw_state.pending.front());
		fw_state.pending.pop_front();
#ifdef OBS_FRONTEND_RUST
		if (obs_fallback_warning_begin(r.id.c_str(), visible) == OBS_FALLBACK_WARNING_DIALOG) {
			fw_open(r);
			return;
		}
#endif
		fw_answer(r, true);
	}
}

void SetMainWindow(QWidget *window, const Texts &texts)
{
	fw_state.window = window;
	fw_state.texts = texts;
	fw_state.ready = true;
	if (!window && fw_state.open) {
		fw_state.open->close();
	}
	fw_pump();
}

void Warn(const char *id, const char *title, const char *message, bool can_cancel, Callback cb, void *param)
{
	id = fw_text(id);
	message = fw_text(message);
	fw_log(id, message);

	FwRequest r{id, fw_text(title), message, can_cancel, cb, param};
	QMetaObject::invokeMethod(
		qApp,
		[r]() {
			fw_state.pending.push_back(r);
			fw_pump();
		},
		Qt::QueuedConnection);
}

} // namespace FallbackWarning
