#pragma once

#include <QDialog>
#include <QString>

class QLabel;
class QPushButton;

/* The camera-busy warning (obs-rust/obs-studio#3724): another program holds
 * a camera OBS uses. It names the device and the holders and waits for a
 * click: Close it, Ignore or Always allow. There is no countdown and nothing
 * happens without a click. The decisions are in rust/obs-camera-busy; this
 * is only the window, and it takes every text from its caller (no App(), no
 * Str()), so it builds without the frontend.
 *
 * It belongs to the OBS main window: `parent` is its transient parent, so it
 * minimizes and comes back with OBS, and it never floats over other apps
 * (no Qt::WindowStaysOnTopHint, no Qt::Tool, not application-modal). Shown
 * with `activate` false (OBS is in the background) it does not take the
 * focus. */
class OBSCameraBusy : public QDialog {
	Q_OBJECT

public:
	/* The values are the OBS_CAMERA_BUSY_* answers of the C ABI
	 * (rust/obs-camera-busy/src/ffi/mod.rs). */
	enum class Answer { CloseIt = 0, Ignore = 1, AlwaysAllow = 2 };

	struct Texts {
		QString title;
		QString message;
		/* Empty: no Quit button (no holder this user may close). */
		QString closeIt;
		/* The question Quit asks before it answers (#3726), and its
		 * buttons. */
		QString confirm;
		QString confirmQuit;
		QString cancel;
		QString ignore;
		/* Empty: no Always allow button (the holder is unknown). */
		QString alwaysAllow;
	};

	OBSCameraBusy(QWidget *parent, const Texts &texts, bool activate);

signals:
	/* Emitted exactly once, before the dialog closes. The window close and
	 * Escape are Ignore. */
	void answered(OBSCameraBusy::Answer answer);

protected:
	void closeEvent(QCloseEvent *event) override;
	void reject() override;

private:
	void finish(Answer answer);

	bool done = false;
};
