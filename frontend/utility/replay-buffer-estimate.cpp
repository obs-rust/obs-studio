#include "replay-buffer-estimate.hpp"

#include <QLabel>
#include <QString>
#include <QStyle>

void SetReplayBufferEstimate(QLabel *label, const QString &text, bool warning)
{
	label->setText(text);
	label->setProperty("class", warning ? "text-warning" : "");
	label->style()->polish(label);
}
