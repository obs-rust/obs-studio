/* Test stand-in for the OBS::SpinBox custom widget in
 * frontend/forms/OBSBasicSettings.ui, for test_settings_track_rows: the
 * generated ui_OBSBasicSettings.h includes <SpinBox.hpp> and only constructs
 * the widget. A plain QSpinBox lays out the same. */

#pragma once

#include <QSpinBox>

namespace OBS {
class SpinBox : public QSpinBox {
public:
	using QSpinBox::QSpinBox;
};
} // namespace OBS
