/* Test stand-in for the OBSHotkeyEdit custom widget in
 * frontend/forms/OBSBasicSettings.ui, for test_settings_track_rows: the
 * generated ui_OBSBasicSettings.h includes <settings/OBSHotkeyEdit.hpp> and
 * only constructs the widget. A plain QLineEdit lays out the same. */

#pragma once

#include <QLineEdit>

class OBSHotkeyEdit : public QLineEdit {
public:
	using QLineEdit::QLineEdit;
};
