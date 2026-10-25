#pragma once

#include <QString>

class QWidget;

/* The shared fallback warning (obs-rust/obs-studio#2218), between its callers
 * (obs_frontend_warn_fallback) and the window (OBSFallbackWarning). It logs
 * at error level at once, then on the UI thread shows the dialog or accepts
 * now, as rust/obs-fallback-warning decides, one dialog at a time, and not
 * while another modal dialog is open.
 *
 * Without the frontend's Rust (OBS_FRONTEND_RUST, the C rollback build) it
 * only logs and accepts. It does not use App() or OBSBasic: the main window
 * hands it the window and the translated texts. */
namespace FallbackWarning {

/* Called once per warning, on the UI thread: whether the caller continues in
 * the fallback mode. */
typedef void (*Callback)(void *param, bool accepted);

/* The texts every dialog shares; each warning brings its title and message. */
struct Texts {
	QString accept;
	QString cancel;
	QString futureError;
};

/* UI thread. Dialogs show on `window` from now on, and warnings that came
 * before it run now (the main window calls it when it has loaded). nullptr
 * (shutdown): the open dialog closes, and later and queued warnings accept
 * without a dialog. */
void SetMainWindow(QWidget *window, const Texts &texts);

/* Any thread. Logs now. Then, on the UI thread and never inside this call,
 * shows the dialog or accepts, and calls `cb` (if not NULL) once. NULL
 * strings are empty. */
void Warn(const char *id, const char *title, const char *message, bool can_cancel, Callback cb, void *param);

} // namespace FallbackWarning
