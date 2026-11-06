#include "frontend/build/frontend-dialogs-obscamerabusy.pch.hpp"

OBSCameraBusy::OBSCameraBusy(QWidget *parent, const Texts &texts, bool activate) : QDialog(parent)
{
	setAttribute(Qt::WA_DeleteOnClose);
	/* A child of the main window, not of the application: OBS keeps
	 * running, and other apps stay in front of it. */
	setWindowModality(Qt::NonModal);
	/* The window type is exactly Dialog (Qt::Tool is a type, not a flag:
	 * masking it out would also clear Dialog and make this a plain child
	 * widget inside the main window). */
	setWindowFlags(Qt::Dialog | (windowFlags() & ~(Qt::WindowType_Mask | Qt::WindowContextHelpButtonHint |
						       Qt::WindowStaysOnTopHint)));
	if (!activate) {
		setAttribute(Qt::WA_ShowWithoutActivating);
	}
	setWindowTitle(texts.title);

	QVBoxLayout *layout = new QVBoxLayout(this);

	QLabel *message = new QLabel(texts.message, this);
	message->setObjectName("message");
	message->setTextFormat(Qt::PlainText);
	message->setWordWrap(true);
	layout->addWidget(message);

	QDialogButtonBox *buttons = new QDialogButtonBox(this);
	auto add = [this, buttons](const QString &text, const char *name, QDialogButtonBox::ButtonRole role,
				   Answer answer) {
		QPushButton *button = buttons->addButton(text, role);
		button->setObjectName(name);
		/* Enter does nothing: every answer is a deliberate click. */
		button->setAutoDefault(false);
		connect(button, &QPushButton::clicked, this, [this, answer]() {
			finish(answer);
			close();
		});
		return button;
	};
	if (!texts.closeIt.isEmpty()) {
		add(texts.closeIt, "closeItButton", QDialogButtonBox::DestructiveRole, Answer::CloseIt);
	}
	if (!texts.alwaysAllow.isEmpty()) {
		add(texts.alwaysAllow, "alwaysAllowButton", QDialogButtonBox::ActionRole, Answer::AlwaysAllow);
	}
	add(texts.ignore, "ignoreButton", QDialogButtonBox::RejectRole, Answer::Ignore);
	layout->addWidget(buttons);
}

/* QDialog::closeEvent() would call reject() and keep the window open while
 * it is visible, so this does not call it. */
void OBSCameraBusy::closeEvent(QCloseEvent *event)
{
	finish(Answer::Ignore);
	event->accept();
}

/* Escape. */
void OBSCameraBusy::reject()
{
	finish(Answer::Ignore);
	close();
}

/* Reports the first answer only: a click passes here, then the close it
 * causes. */
void OBSCameraBusy::finish(Answer answer)
{
	if (done) {
		return;
	}
	done = true;
	emit answered(answer);
}
