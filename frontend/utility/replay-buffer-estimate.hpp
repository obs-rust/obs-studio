#pragma once

class QLabel;
class QString;

/* Settings > Output: shows `text` in a Replay Buffer memory estimate label
 * (Simple or Advanced mode), styled as a warning ("text-warning" class) when
 * `warning` is true, that is when the estimate is above the recommended
 * maximum. Settings calls this every time the estimate changes. */
void SetReplayBufferEstimate(QLabel *label, const QString &text, bool warning);
