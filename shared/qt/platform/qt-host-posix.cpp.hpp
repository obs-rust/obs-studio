#include "shared/qt/wrappers/build/qt-wrappers.pch.hpp"

namespace qt_platform {
void tweak_checkbox(QCheckBox *checkbox)
{
	(void)checkbox;
}

void raise_after_dialog(QWidget *widget)
{
	(void)widget;
}

QColorDialog::ColorDialogOptions color_dialog_options()
{
#ifdef __linux__
	// TODO: Revisit hang on Ubuntu with native dialog
	return QColorDialog::DontUseNativeDialog;
#else
	return {};
#endif
}

QFontDialog::FontDialogOptions font_dialog_options()
{
	return QFontDialog::DontUseNativeDialog;
}

uint32_t translate_modifiers(Qt::KeyboardModifiers mods)
{
	uint32_t obsModifiers = INTERACT_NONE;

	if (mods.testFlag(Qt::ShiftModifier)) {
		obsModifiers |= INTERACT_SHIFT_KEY;
	}
	if (mods.testFlag(Qt::AltModifier)) {
		obsModifiers |= INTERACT_ALT_KEY;
	}
	// Handle windows key? Can a browser even trap that key?
	if (mods.testFlag(Qt::ControlModifier)) {
		obsModifiers |= INTERACT_CONTROL_KEY;
	}
	if (mods.testFlag(Qt::MetaModifier)) {
		obsModifiers |= INTERACT_COMMAND_KEY;
	}

	return static_cast<uint32_t>(obsModifiers);
}
} // namespace qt_platform
