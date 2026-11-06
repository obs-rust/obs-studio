#pragma once

#include <QString>

class QWidget;

/* The camera-busy warning (obs-rust/obs-studio#3724), between the camera
 * sources' signals and the window (OBSCameraBusy). A source reports its
 * device busy (EBUSY) or opened by OBS; OBS then watches the device for
 * other processes opening it. Who holds it is looked up off the UI thread;
 * rust/obs-camera-busy decides whether a dialog shows (allowlist, one per
 * device and holder, rate limit) and what an answer does, and every warning
 * is logged.
 *
 * The dialog is a child of the main window. While the main window is
 * hidden or minimized a warning waits, and shows when it comes back; while
 * OBS is in the background the dialog shows without taking the focus and
 * the taskbar entry asks for attention (QApplication::alert), so it never
 * raises itself over the app the user is in.
 *
 * Without the frontend's Rust (OBS_FRONTEND_RUST, the C rollback build)
 * Busy() returns false and does nothing; the caller keeps its own popup. */
namespace CameraBusy {

struct Texts {
	QString title;
	/* %1 device, %2 the holder list */
	QString busy;
	/* %1 device */
	QString busyUnknown;
	/* %1 the holder list, %2 device */
	QString opened;
	/* %1 the programs Quit closes ("Quit zoom", #3726) */
	QString closeIt;
	/* %1 the holder list: asked before Quit closes them */
	QString confirm;
	QString confirmQuit;
	QString cancel;
	QString ignore;
	QString alwaysAllow;
};

/* Saves the "Always allow" list (a config string) to OBS's user config. */
typedef void (*SaveAllowlist)(void *param, const char *config);

/* UI thread. Dialogs show on `window` from now on; `allowlist` is the saved
 * list. nullptr (shutdown): open dialogs close, watches stop, later reports
 * do nothing. */
void SetMainWindow(QWidget *window, const Texts &texts, const char *allowlist, SaveAllowlist save, void *param);

/* Any thread. `device` could not be opened or streamed: it is busy. Returns
 * false without the frontend's Rust. */
bool Busy(const char *device);

/* UI thread. A source opened / closed `device`: watch it for other
 * processes opening it while OBS has it. Counted per device. */
void DeviceOpened(const char *device);
void DeviceClosed(const char *device);

} // namespace CameraBusy
