#include "frontend/build/frontend-dialogs-obsfallbackwarning.pch.hpp"

/* How often the countdown progress moves. */
static constexpr int fallback_warning_tick_ms = 50;

OBSFallbackWarning::OBSFallbackWarning(QWidget *parent, const Texts &texts, qint64 countdownMs_)
	: QDialog(parent),
	  countdownMs(std::clamp<qint64>(countdownMs_, 0, std::numeric_limits<int>::max()))
{
	setAttribute(Qt::WA_DeleteOnClose);
	setModal(false);
	setWindowFlags(windowFlags() & ~Qt::WindowContextHelpButtonHint);
	setWindowTitle(texts.title);

	QVBoxLayout *layout = new QVBoxLayout(this);

	messageLabel = new QLabel(texts.message, this);
	messageLabel->setObjectName("message");
	messageLabel->setTextFormat(Qt::PlainText);
	messageLabel->setWordWrap(true);
	layout->addWidget(messageLabel);

	futureErrorLabel = new QLabel(texts.futureError, this);
	futureErrorLabel->setObjectName("futureError");
	futureErrorLabel->setTextFormat(Qt::PlainText);
	futureErrorLabel->setWordWrap(true);
	layout->addWidget(futureErrorLabel);

	progress = new QProgressBar(this);
	progress->setObjectName("progress");
	progress->setRange(0, (int)countdownMs);
	progress->setValue(0);
	progress->setTextVisible(false);
	layout->addWidget(progress);

	QDialogButtonBox *buttons = new QDialogButtonBox(this);
	acceptButton = buttons->addButton(texts.accept, QDialogButtonBox::AcceptRole);
	acceptButton->setObjectName("acceptButton");
	connect(acceptButton, &QPushButton::clicked, this, [this]() {
		finish(End::AcceptClicked);
		close();
	});
	if (!texts.cancel.isEmpty()) {
		/* The safe choice is the default one; the countdown accepts anyway. */
		cancelButton = buttons->addButton(texts.cancel, QDialogButtonBox::RejectRole);
		cancelButton->setObjectName("cancelButton");
		cancelButton->setDefault(true);
		connect(cancelButton, &QPushButton::clicked, this, [this]() {
			finish(End::CancelClicked);
			close();
		});
	}
	layout->addWidget(buttons);

	timer = new QTimer(this);
	timer->setInterval(fallback_warning_tick_ms);
	connect(timer, &QTimer::timeout, this, &OBSFallbackWarning::tick);
	clock.start();
	timer->start();
}

qint64 OBSFallbackWarning::elapsedShown() const
{
	return progress->value();
}

/* QDialog::closeEvent() would call reject() and keep the window open while
 * it is visible, so this does not call it. */
void OBSFallbackWarning::closeEvent(QCloseEvent *event)
{
	finish(End::Closed);
	event->accept();
}

/* Escape. */
void OBSFallbackWarning::reject()
{
	finish(End::Closed);
	close();
}

void OBSFallbackWarning::tick()
{
	if (done) {
		return;
	}
	qint64 step = clock.restart();
	/* Another modal dialog covers this one or takes its input (the
	 * first-run wizard opens right after the main window has loaded): do
	 * not accept unseen. Go on, in front, once it has closed. */
	QWidget *modal = QApplication::activeModalWidget();
	if (modal && modal != this) {
		paused = true;
		return;
	}
	if (paused) {
		paused = false;
		raise();
		activateWindow();
	}
	elapsed = std::min(elapsed + step, countdownMs);
	progress->setValue((int)elapsed);
	if (elapsed >= countdownMs) {
		finish(End::CountdownDone);
		close();
	}
}

/* Reports the first end only: every way out passes here, some twice (a
 * button click, then the close it causes). */
void OBSFallbackWarning::finish(End end)
{
	if (done) {
		return;
	}
	done = true;
	timer->stop();
	emit ended(end);
}
