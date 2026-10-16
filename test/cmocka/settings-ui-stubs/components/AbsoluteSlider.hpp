/* Test stand-in for the AbsoluteSlider custom widget in
 * frontend/forms/OBSBasicSettings.ui, for test_settings_track_rows: the
 * generated ui_OBSBasicSettings.h includes <components/AbsoluteSlider.hpp> and
 * only constructs the widget. A plain QSlider lays out the same. */

#pragma once

#include <QSlider>

class AbsoluteSlider : public QSlider {
public:
	using QSlider::QSlider;
};
