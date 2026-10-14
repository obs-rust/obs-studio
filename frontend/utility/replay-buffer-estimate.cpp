#include "replay-buffer-estimate.hpp"

#include <QLabel>
#include <QString>
#include <QStyle>

/* Stub with the current logic of OBSBasicSettings::SimpleReplayBufferChanged()
 * and AdvReplayBufferChanged(): the label's object name is cleared, which
 * removed the old "warningLabel" style before themes used a class, and the
 * warning class is only ever added. */
void SetReplayBufferEstimate(QLabel *label, const QString &text, bool warning)
{
	label->setObjectName("");
	label->setText(text);
	if (warning) {
		label->setProperty("class", "text-warning");
	}
	label->style()->polish(label);
}
