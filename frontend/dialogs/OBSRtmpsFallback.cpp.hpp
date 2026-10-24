#include "frontend/build/frontend-dialogs-obsrtmpsfallback.pch.hpp"

OBSRtmpsFallback::OBSRtmpsFallback(QWidget *parent, const Texts &texts) : QDialog(parent)
{
	setWindowTitle(texts.title);

	QVBoxLayout *layout = new QVBoxLayout(this);
	QLabel *label = new QLabel(texts.message, this);
	label->setWordWrap(true);
	layout->addWidget(label);

	progress = new QProgressBar(this);
	progress->setRange(0, CountdownMs);
	progress->setTextVisible(false);
	layout->addWidget(progress);

	QDialogButtonBox *buttons = new QDialogButtonBox(this);
	proceedButton = buttons->addButton(texts.proceed, QDialogButtonBox::AcceptRole);
	buttons->addButton(texts.cancel, QDialogButtonBox::RejectRole);
	connect(buttons, &QDialogButtonBox::accepted, this, &QDialog::accept);
	connect(buttons, &QDialogButtonBox::rejected, this, &QDialog::reject);
	layout->addWidget(buttons);

	tick.setInterval(50);
	connect(&tick, &QTimer::timeout, this, &OBSRtmpsFallback::Tick);
}

void OBSRtmpsFallback::showEvent(QShowEvent *event)
{
	QDialog::showEvent(event);
	elapsed.start();
	progress->setValue(0);
	tick.start();
}

void OBSRtmpsFallback::Tick()
{
	/* Cancel or a click closed the dialog: the countdown ends with it. */
	if (!isVisible()) {
		tick.stop();
		return;
	}

	qint64 ms = elapsed.elapsed();
	progress->setValue((int)std::min<qint64>(ms, CountdownMs));
	if (ms >= CountdownMs) {
		tick.stop();
		accept();
	}
}

void OBSRtmpsFallback::LogFallback(const char *plain_url)
{
	fprintf(stderr, "RTMPS connection failed; streaming without encryption to %s\n", plain_url);
	fflush(stderr);
	blog(LOG_WARNING, "RTMPS connection failed; streaming without encryption to %s", plain_url);
}
