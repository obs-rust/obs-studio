#pragma once

#include <QDialog>
#include <QElapsedTimer>
#include <QString>
#include <QTimer>

class QProgressBar;
class QPushButton;

/* Shown when a stream that rtmp-services upgraded to RTMPS fails to connect.
 * "Proceed anyway" connects to the same server over plain RTMP; it fills a
 * progress bar for CountdownMs and then accepts on its own, so a stream started
 * with nobody at the screen still goes live. Cancel stops the start. */
class OBSRtmpsFallback : public QDialog {
	Q_OBJECT

public:
	static constexpr int CountdownMs = 3000;

	struct Texts {
		QString title;
		QString message;
		QString proceed;
		QString cancel;
	};

	OBSRtmpsFallback(QWidget *parent, const Texts &texts);

	/* Writes the fallback to stderr and the OBS log. */
	static void LogFallback(const char *plain_url);

private:
	QProgressBar *progress = nullptr;
	QPushButton *proceedButton = nullptr;
	QTimer tick;
	QElapsedTimer elapsed;

	void Tick();
};
