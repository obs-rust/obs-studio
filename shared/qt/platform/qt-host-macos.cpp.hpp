#include "shared/qt/wrappers/build/qt-wrappers.pch.hpp"

namespace qt_platform {
void tweak_checkbox(QCheckBox *checkbox)
{
	checkbox->setAttribute(Qt::WA_LayoutUsesWidgetRect);
}

void raise_after_dialog(QWidget *widget)
{
	// TODO: Revisit when QTBUG-42661 is fixed
	widget->window()->raise();
}

QColorDialog::ColorDialogOptions color_dialog_options()
{
	return {};
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
	// Mac: Meta = Control, Control = Command
	if (mods.testFlag(Qt::ControlModifier)) {
		obsModifiers |= INTERACT_COMMAND_KEY;
	}
	if (mods.testFlag(Qt::MetaModifier)) {
		obsModifiers |= INTERACT_CONTROL_KEY;
	}

	return obsModifiers;
}
} // namespace qt_platform
