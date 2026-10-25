#include "frontend/build/frontend-dialogs-obsfallbackwarning.pch.hpp"

/* RED (#2218): the window does nothing yet. */

OBSFallbackWarning::OBSFallbackWarning(QWidget *parent, const Texts &, qint64 countdownMs_)
	: QDialog(parent),
	  countdownMs(countdownMs_)
{
}

qint64 OBSFallbackWarning::elapsedShown() const
{
	return 0;
}

void OBSFallbackWarning::closeEvent(QCloseEvent *event)
{
	QDialog::closeEvent(event);
}

void OBSFallbackWarning::reject()
{
	QDialog::reject();
}

void OBSFallbackWarning::tick() {}

void OBSFallbackWarning::finish(End) {}
