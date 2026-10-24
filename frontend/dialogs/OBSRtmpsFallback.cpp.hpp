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
}

void OBSRtmpsFallback::Tick() {}

void OBSRtmpsFallback::LogFallback(const char *plain_url)
{
	(void)plain_url;
}
