/* Test stand-in for the UrlPushButton custom widget in
 * frontend/forms/OBSBasicSettings.ui, for test_settings_track_rows: the
 * generated ui_OBSBasicSettings.h includes <components/UrlPushButton.hpp> and
 * only constructs the widget. A plain QPushButton lays out the same. */

#pragma once

#include <QPushButton>

class UrlPushButton : public QPushButton {
public:
	using QPushButton::QPushButton;
};
