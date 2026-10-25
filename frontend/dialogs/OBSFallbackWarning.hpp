#pragma once

#include <QDialog>
#include <QElapsedTimer>
#include <QString>

class QLabel;
class QProgressBar;
class QPushButton;
class QTimer;

/* The shared fallback warning (obs-rust/obs-studio#2218): a feature fell back
 * to a less safe mode, and the user sees why, with a countdown that accepts
 * the fallback by itself. The decisions (whether to show it, what an end
 * means) are in rust/obs-fallback-warning; this is only the window. It takes
 * every text from its caller (no App(), no Str()), so it builds without the
 * frontend. It is not modal: OBS keeps running while it counts down. The
 * countdown pauses while another modal dialog is open. */
class OBSFallbackWarning : public QDialog {
	Q_OBJECT

public:
	/* How the dialog ended. The values are the OBS_FALLBACK_WARNING_END_*
	 * values of the C ABI (rust/obs-fallback-warning/src/ffi/mod.rs). */
	enum class End { CountdownDone = 0, AcceptClicked = 1, CancelClicked = 2, Closed = 3 };

	struct Texts {
		QString title;
		QString message;
		/* The sentence below the message ("This may become an error ..."). */
		QString futureError;
		QString accept;
		/* Empty: no Cancel button (the caller cannot stop). */
		QString cancel;
	};

	OBSFallbackWarning(QWidget *parent, const Texts &texts, qint64 countdownMs);

	/* The countdown progress shown, 0 to countdownMs. */
	qint64 elapsedShown() const;

signals:
	/* Emitted exactly once, before the dialog closes. */
	void ended(OBSFallbackWarning::End end);

protected:
	void closeEvent(QCloseEvent *event) override;
	void reject() override;

private:
	void tick();
	void finish(End end);

	qint64 countdownMs;
	bool done = false;
	/* Time counted down, only while no other modal dialog is open. */
	qint64 elapsed = 0;
	bool paused = false;
	QElapsedTimer clock;
	QTimer *timer = nullptr;
	QLabel *messageLabel = nullptr;
	QLabel *futureErrorLabel = nullptr;
	QProgressBar *progress = nullptr;
	QPushButton *acceptButton = nullptr;
	QPushButton *cancelButton = nullptr;
};
