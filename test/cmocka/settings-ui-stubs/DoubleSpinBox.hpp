/* Test stand-in for the OBS::DoubleSpinBox custom widget in
 * frontend/forms/OBSBasicSettings.ui, for test_settings_track_rows: the
 * generated ui_OBSBasicSettings.h includes <DoubleSpinBox.hpp> and only
 * constructs the widget. A plain QDoubleSpinBox lays out the same. */

#pragma once

#include <QDoubleSpinBox>

namespace OBS {
class DoubleSpinBox : public QDoubleSpinBox {
public:
	using QDoubleSpinBox::QDoubleSpinBox;
};
} // namespace OBS
