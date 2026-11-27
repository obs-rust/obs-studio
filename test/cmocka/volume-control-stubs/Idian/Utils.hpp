#pragma once

/* Stand-in for shared/qt/idian's Idian/Utils.hpp (test_volume_control_uuid):
 * the style-class helpers VolumeControl calls, doing nothing. */

#include <QString>

class QWidget;

namespace idian {
class Utils {
public:
	static void addClass(QWidget *, const QString &) {}
	static void toggleClass(QWidget *, const QString &, bool) {}
	static void applyStateStylingEventFilter(QWidget *) {}
};
} // namespace idian
