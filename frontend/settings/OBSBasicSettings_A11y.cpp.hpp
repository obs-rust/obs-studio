#include "frontend/build/frontend-settings.pch.hpp"

enum ColorPreset {
	COLOR_PRESET_DEFAULT,
	COLOR_PRESET_COLOR_BLIND_1,
	COLOR_PRESET_CUSTOM = 99,
};

static inline QColor a11y_color_from_int(long long val)
{
	return QColor(val & 0xff, (val >> 8) & 0xff, (val >> 16) & 0xff, (val >> 24) & 0xff);
}

static inline long long a11y_color_to_int(QColor color)
{
	auto shift = [&](int val, int shift) {
		return static_cast<long long>(static_cast<unsigned>(val & 0xff)) << shift;
	};

	return shift(color.red(), 0) | shift(color.green(), 8) | shift(color.blue(), 16) | shift(color.alpha(), 24);
}

QColor OBSBasicSettings::GetColor(uint32_t colorVal, QString label)
{
	QColorDialog::ColorDialogOptions options;

	if (PlatformNeedsNonNativeColorDialog()) {
		options |= QColorDialog::DontUseNativeDialog;
	}

	QColor color = a11y_color_from_int(colorVal);

	return QColorDialog::getColor(color, this, label, options);
}

void OBSBasicSettings::LoadA11ySettings(bool presetChange)
{
	config_t *config = App()->GetUserConfig();

	loading = true;
	if (!presetChange) {
		preset = static_cast<int>(config_get_int(config, "Accessibility", "ColorPreset"));

		bool block = ui->colorPreset->blockSignals(true);
		ui->colorPreset->setCurrentIndex(std::min(preset, ui->colorPreset->count() - 1));
		ui->colorPreset->blockSignals(block);

		bool checked = config_get_bool(config, "Accessibility", "OverrideColors");

		ui->colorsGroupBox->setChecked(checked);
	}

	if (preset == COLOR_PRESET_DEFAULT) {
		ResetDefaultColors();
		SetDefaultColors();
	} else if (preset == COLOR_PRESET_COLOR_BLIND_1) {
		ResetDefaultColors();

		mixerGreenActive = 0x742E94;
		mixerGreen = 0x4A1A60;
		mixerYellowActive = 0x3349F9;
		mixerYellow = 0x1F2C97;
		mixerRedActive = 0xBEAC63;
		mixerRed = 0x675B28;

		selectRed = 0x3349F9;
		selectGreen = 0xFF56C9;
		selectBlue = 0xB09B44;

		SetDefaultColors();
	} else if (preset == COLOR_PRESET_CUSTOM) {
		SetDefaultColors();

		selectRed = static_cast<uint32_t>(config_get_int(config, "Accessibility", "SelectRed"));
		selectGreen = static_cast<uint32_t>(config_get_int(config, "Accessibility", "SelectGreen"));
		selectBlue = static_cast<uint32_t>(config_get_int(config, "Accessibility", "SelectBlue"));

		mixerGreen = static_cast<uint32_t>(config_get_int(config, "Accessibility", "MixerGreen"));
		mixerYellow = static_cast<uint32_t>(config_get_int(config, "Accessibility", "MixerYellow"));
		mixerRed = static_cast<uint32_t>(config_get_int(config, "Accessibility", "MixerRed"));

		mixerGreenActive = static_cast<uint32_t>(config_get_int(config, "Accessibility", "MixerGreenActive"));
		mixerYellowActive = static_cast<uint32_t>(config_get_int(config, "Accessibility", "MixerYellowActive"));
		mixerRedActive = static_cast<uint32_t>(config_get_int(config, "Accessibility", "MixerRedActive"));
	}

	UpdateA11yColors();

	loading = false;
}

void OBSBasicSettings::SaveA11ySettings()
{
	config_t *config = App()->GetUserConfig();

	config_set_bool(config, "Accessibility", "OverrideColors", ui->colorsGroupBox->isChecked());
	config_set_int(config, "Accessibility", "ColorPreset", preset);

	config_set_int(config, "Accessibility", "SelectRed", selectRed);
	config_set_int(config, "Accessibility", "SelectGreen", selectGreen);
	config_set_int(config, "Accessibility", "SelectBlue", selectBlue);
	config_set_int(config, "Accessibility", "MixerGreen", mixerGreen);
	config_set_int(config, "Accessibility", "MixerYellow", mixerYellow);
	config_set_int(config, "Accessibility", "MixerRed", mixerRed);
	config_set_int(config, "Accessibility", "MixerGreenActive", mixerGreenActive);
	config_set_int(config, "Accessibility", "MixerYellowActive", mixerYellowActive);
	config_set_int(config, "Accessibility", "MixerRedActive", mixerRedActive);

	emit main->userSettingChanged("Accessibility", "SettingsChanged");
}

static void SetStyle(QLabel *label, uint32_t colorVal)
{
	QColor color = a11y_color_from_int(colorVal);
	color.setAlpha(255);
	QPalette palette = QPalette(color);
	label->setFrameStyle(QFrame::Sunken | QFrame::Panel);
	label->setText(color.name(QColor::HexRgb));
	label->setPalette(palette);
	label->setStyleSheet(QString("background-color: %1; color: %2;")
				     .arg(palette.color(QPalette::Window).name(QColor::HexRgb))
				     .arg(palette.color(QPalette::WindowText).name(QColor::HexRgb)));
	label->setAutoFillBackground(true);
	label->setAlignment(Qt::AlignCenter);
}

void OBSBasicSettings::UpdateA11yColors()
{
	SetStyle(ui->color1, selectRed);
	SetStyle(ui->color2, selectGreen);
	SetStyle(ui->color3, selectBlue);
	SetStyle(ui->color4, mixerGreen);
	SetStyle(ui->color5, mixerYellow);
	SetStyle(ui->color6, mixerRed);
	SetStyle(ui->color7, mixerGreenActive);
	SetStyle(ui->color8, mixerYellowActive);
	SetStyle(ui->color9, mixerRedActive);
}

void OBSBasicSettings::SetDefaultColors()
{
	config_t *config = App()->GetUserConfig();
	config_set_default_int(config, "Accessibility", "SelectRed", selectRed);
	config_set_default_int(config, "Accessibility", "SelectGreen", selectGreen);
	config_set_default_int(config, "Accessibility", "SelectBlue", selectBlue);

	config_set_default_int(config, "Accessibility", "MixerGreen", mixerGreen);
	config_set_default_int(config, "Accessibility", "MixerYellow", mixerYellow);
	config_set_default_int(config, "Accessibility", "MixerRed", mixerRed);

	config_set_default_int(config, "Accessibility", "MixerGreenActive", mixerGreenActive);
	config_set_default_int(config, "Accessibility", "MixerYellowActive", mixerYellowActive);
	config_set_default_int(config, "Accessibility", "MixerRedActive", mixerRedActive);
}

void OBSBasicSettings::ResetDefaultColors()
{
	selectRed = 0x0000FF;
	selectGreen = 0x00FF00;
	selectBlue = 0xFF7F00;
	mixerGreen = 0x267f26;
	mixerYellow = 0x267f7f;
	mixerRed = 0x26267f;
	mixerGreenActive = 0x4cff4c;
	mixerYellowActive = 0x4cffff;
	mixerRedActive = 0x4c4cff;
}

void OBSBasicSettings::on_colorPreset_currentIndexChanged(int idx)
{
	preset = idx == ui->colorPreset->count() - 1 ? COLOR_PRESET_CUSTOM : idx;
	LoadA11ySettings(true);
}

void OBSBasicSettings::on_choose1_clicked()
{
	QColor color = GetColor(selectRed, QTStr("Basic.Settings.Accessibility.ColorOverrides.SelectRed"));

	if (!color.isValid()) {
		return;
	}

	selectRed = static_cast<uint32_t>(a11y_color_to_int(color));

	preset = COLOR_PRESET_CUSTOM;
	bool block = ui->colorPreset->blockSignals(true);
	ui->colorPreset->setCurrentIndex(ui->colorPreset->count() - 1);
	ui->colorPreset->blockSignals(block);

	A11yChanged();

	UpdateA11yColors();
}

void OBSBasicSettings::on_choose2_clicked()
{
	QColor color = GetColor(selectGreen, QTStr("Basic.Settings.Accessibility.ColorOverrides.SelectGreen"));

	if (!color.isValid()) {
		return;
	}

	selectGreen = static_cast<uint32_t>(a11y_color_to_int(color));

	preset = COLOR_PRESET_CUSTOM;
	bool block = ui->colorPreset->blockSignals(true);
	ui->colorPreset->setCurrentIndex(ui->colorPreset->count() - 1);
	ui->colorPreset->blockSignals(block);

	A11yChanged();

	UpdateA11yColors();
}

void OBSBasicSettings::on_choose3_clicked()
{
	QColor color = GetColor(selectBlue, QTStr("Basic.Settings.Accessibility.ColorOverrides.SelectBlue"));

	if (!color.isValid()) {
		return;
	}

	selectBlue = static_cast<uint32_t>(a11y_color_to_int(color));

	preset = COLOR_PRESET_CUSTOM;
	bool block = ui->colorPreset->blockSignals(true);
	ui->colorPreset->setCurrentIndex(ui->colorPreset->count() - 1);
	ui->colorPreset->blockSignals(block);

	A11yChanged();

	UpdateA11yColors();
}

void OBSBasicSettings::on_choose4_clicked()
{
	QColor color = GetColor(mixerGreen, QTStr("Basic.Settings.Accessibility.ColorOverrides.MixerGreen"));

	if (!color.isValid()) {
		return;
	}

	mixerGreen = static_cast<uint32_t>(a11y_color_to_int(color));

	preset = COLOR_PRESET_CUSTOM;
	bool block = ui->colorPreset->blockSignals(true);
	ui->colorPreset->setCurrentIndex(ui->colorPreset->count() - 1);
	ui->colorPreset->blockSignals(block);

	A11yChanged();

	UpdateA11yColors();
}

void OBSBasicSettings::on_choose5_clicked()
{
	QColor color = GetColor(mixerYellow, QTStr("Basic.Settings.Accessibility.ColorOverrides.MixerYellow"));

	if (!color.isValid()) {
		return;
	}

	mixerYellow = static_cast<uint32_t>(a11y_color_to_int(color));

	preset = COLOR_PRESET_CUSTOM;
	bool block = ui->colorPreset->blockSignals(true);
	ui->colorPreset->setCurrentIndex(ui->colorPreset->count() - 1);
	ui->colorPreset->blockSignals(block);

	A11yChanged();

	UpdateA11yColors();
}

void OBSBasicSettings::on_choose6_clicked()
{
	QColor color = GetColor(mixerRed, QTStr("Basic.Settings.Accessibility.ColorOverrides.MixerRed"));

	if (!color.isValid()) {
		return;
	}

	mixerRed = static_cast<uint32_t>(a11y_color_to_int(color));

	preset = COLOR_PRESET_CUSTOM;
	bool block = ui->colorPreset->blockSignals(true);
	ui->colorPreset->setCurrentIndex(ui->colorPreset->count() - 1);
	ui->colorPreset->blockSignals(block);

	A11yChanged();

	UpdateA11yColors();
}

void OBSBasicSettings::on_choose7_clicked()
{
	QColor color =
		GetColor(mixerGreenActive, QTStr("Basic.Settings.Accessibility.ColorOverrides.MixerGreenActive"));

	if (!color.isValid()) {
		return;
	}

	mixerGreenActive = static_cast<uint32_t>(a11y_color_to_int(color));

	preset = COLOR_PRESET_CUSTOM;
	bool block = ui->colorPreset->blockSignals(true);
	ui->colorPreset->setCurrentIndex(ui->colorPreset->count() - 1);
	ui->colorPreset->blockSignals(block);

	A11yChanged();

	UpdateA11yColors();
}

void OBSBasicSettings::on_choose8_clicked()
{
	QColor color =
		GetColor(mixerYellowActive, QTStr("Basic.Settings.Accessibility.ColorOverrides.MixerYellowActive"));

	if (!color.isValid()) {
		return;
	}

	mixerYellowActive = static_cast<uint32_t>(a11y_color_to_int(color));

	preset = COLOR_PRESET_CUSTOM;
	bool block = ui->colorPreset->blockSignals(true);
	ui->colorPreset->setCurrentIndex(ui->colorPreset->count() - 1);
	ui->colorPreset->blockSignals(block);

	A11yChanged();

	UpdateA11yColors();
}

void OBSBasicSettings::on_choose9_clicked()
{
	QColor color = GetColor(mixerRedActive, QTStr("Basic.Settings.Accessibility.ColorOverrides.MixerRedActive"));

	if (!color.isValid()) {
		return;
	}

	mixerRedActive = static_cast<uint32_t>(a11y_color_to_int(color));

	preset = COLOR_PRESET_CUSTOM;
	bool block = ui->colorPreset->blockSignals(true);
	ui->colorPreset->setCurrentIndex(ui->colorPreset->count() - 1);
	ui->colorPreset->blockSignals(block);

	A11yChanged();

	UpdateA11yColors();
}
