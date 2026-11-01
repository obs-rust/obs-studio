#include "frontend/build/frontend-dialogs.pch.hpp"

OBSBasicVCamConfig::OBSBasicVCamConfig(const VCamConfig &_config, bool _vcamActive, QWidget *parent)
	: config(_config),
	  vcamActive(_vcamActive),
	  activeType(_config.type),
	  QDialog(parent),
	  ui(new Ui::OBSBasicVCamConfig)
{
	setWindowFlags(windowFlags() & ~Qt::WindowContextHelpButtonHint);

	ui->setupUi(this);

	ui->outputType->addItem(QTStr("Basic.VCam.OutputType.Program"), (int)VCamOutputType::ProgramView);
	ui->outputType->addItem(QTStr("StudioMode.Preview"), (int)VCamOutputType::PreviewOutput);
	ui->outputType->addItem(QTStr("Basic.Scene"), (int)VCamOutputType::SceneOutput);
	ui->outputType->addItem(QTStr("Basic.Main.Source"), (int)VCamOutputType::SourceOutput);

	ui->outputType->setCurrentIndex(ui->outputType->findData((int)config.type));
	OutputTypeChanged();
	connect(ui->outputType, &QComboBox::currentIndexChanged, this, &OBSBasicVCamConfig::OutputTypeChanged);

	/* Only an output that sends audio has a Virtual Microphone (#3182). */
	bool hasAudio = (obs_get_output_flags(VIRTUAL_CAM_ID) & OBS_OUTPUT_AUDIO) != 0;
	ui->audioEnabled->setVisible(hasAudio);
	ui->audioTrackLabel->setVisible(hasAudio);
	ui->audioTrack->setVisible(hasAudio);
	for (int track = 1; track <= VCAM_AUDIO_TRACKS; track++) {
		ui->audioTrack->addItem(QString::number(track), track);
	}
	ui->audioEnabled->setChecked(config.audio.enabled);
	ui->audioTrack->setCurrentIndex(ui->audioTrack->findData(VCamAudioTrackFromSaved(config.audio.track)));
	ui->audioTrack->setEnabled(config.audio.enabled);
	connect(ui->audioEnabled, &QCheckBox::toggled, ui->audioTrack, &QWidget::setEnabled);
	connect(ui->audioEnabled, &QCheckBox::toggled, this, &OBSBasicVCamConfig::AudioChanged);
	connect(ui->audioTrack, &QComboBox::currentIndexChanged, this, &OBSBasicVCamConfig::AudioChanged);

	/* An output that renders to a device lists them (Windows, #3183). */
	bool hasDevices = false;
	if (obs_properties_t *props = obs_get_output_properties(VIRTUAL_CAM_ID)) {
		if (obs_property_t *p = obs_properties_get(props, "audio_device")) {
			for (size_t i = 0; i < obs_property_list_item_count(p); i++) {
				ui->audioDevice->addItem(QT_UTF8(obs_property_list_item_name(p, i)),
							 QT_UTF8(obs_property_list_item_string(p, i)));
			}
			hasDevices = true;
		}
		obs_properties_destroy(props);
	}
	int saved = ui->audioDevice->findData(QT_UTF8(config.audio.device.c_str()));
	ui->audioDevice->setCurrentIndex(saved >= 0 ? saved : 0);
	ui->audioDeviceLabel->setVisible(hasAudio && hasDevices);
	ui->audioDevice->setVisible(hasAudio && hasDevices);
	ui->audioDevice->setEnabled(config.audio.enabled);
	connect(ui->audioEnabled, &QCheckBox::toggled, ui->audioDevice, &QWidget::setEnabled);
	connect(ui->audioDevice, &QComboBox::currentIndexChanged, this, &OBSBasicVCamConfig::AudioChanged);

	connect(ui->buttonBox, &QDialogButtonBox::accepted, this, &OBSBasicVCamConfig::UpdateConfig);
}

/* The output reads its audio settings when it starts, so a running Virtual
 * Camera restarts to apply a change. */
void OBSBasicVCamConfig::AudioChanged()
{
	if (!vcamActive) {
		return;
	}

	VCamAudioConfig chosen{ui->audioEnabled->isChecked(), ui->audioTrack->currentData().toInt(),
			       ui->audioDevice->currentData().toString().toStdString()};
	audioChanged = chosen.enabled != config.audio.enabled ||
		       (chosen.enabled && (chosen.track != VCamAudioTrackFromSaved(config.audio.track) ||
					   chosen.device != config.audio.device));
	ui->warningLabel->setVisible(requireRestart || audioChanged);
}

void OBSBasicVCamConfig::OutputTypeChanged()
{
	VCamOutputType type = (VCamOutputType)ui->outputType->currentData().toInt();
	ui->outputSelection->setDisabled(false);

	auto list = ui->outputSelection;
	list->clear();

	switch (type) {
	case VCamOutputType::Invalid:
	case VCamOutputType::ProgramView:
	case VCamOutputType::PreviewOutput:
		ui->outputSelection->setDisabled(true);
		list->addItem(QTStr("Basic.VCam.OutputSelection.NoSelection"));
		break;
	case VCamOutputType::SceneOutput: {
		// Scenes in default order
		BPtr<char *> scenes = obs_frontend_get_scene_names();
		for (char **temp = scenes; *temp; temp++) {
			list->addItem(*temp);

			if (config.scene.compare(*temp) == 0) {
				list->setCurrentIndex(list->count() - 1);
			}
		}
		break;
	}
	case VCamOutputType::SourceOutput: {
		// Sources in alphabetical order
		std::vector<std::string> sources;
		auto AddSource = [&](obs_source_t *source) {
			auto name = obs_source_get_name(source);

			if (!(obs_source_get_output_flags(source) & OBS_SOURCE_VIDEO)) {
				return;
			}

			sources.push_back(name);
		};
		using AddSource_t = decltype(AddSource);

		obs_enum_sources(
			[](void *handler_data, obs_source_t *source) {
				auto &AddSource = *static_cast<AddSource_t *>(handler_data);
				if (!obs_source_removed(source)) {
					AddSource(source);
				}
				return true;
			},
			static_cast<void *>(&AddSource));

		// Sort and select current item
		sort(sources.begin(), sources.end());
		for (auto &&source : sources) {
			list->addItem(source.c_str());

			if (config.source == source) {
				list->setCurrentIndex(list->count() - 1);
			}
		}
		break;
	}
	}

	if (!vcamActive) {
		return;
	}

	requireRestart = (activeType == VCamOutputType::ProgramView && type != VCamOutputType::ProgramView) ||
			 (activeType != VCamOutputType::ProgramView && type == VCamOutputType::ProgramView);

	ui->warningLabel->setVisible(requireRestart || audioChanged);
}

void OBSBasicVCamConfig::UpdateConfig()
{
	VCamOutputType type = (VCamOutputType)ui->outputType->currentData().toInt();
	switch (type) {
	case VCamOutputType::ProgramView:
	case VCamOutputType::PreviewOutput:
		break;
	case VCamOutputType::SceneOutput:
		config.scene = ui->outputSelection->currentText().toStdString();
		break;
	case VCamOutputType::SourceOutput:
		config.source = ui->outputSelection->currentText().toStdString();
		break;
	default:
		// unknown value, don't save type
		return;
	}

	config.type = type;
	config.audio.enabled = ui->audioEnabled->isChecked();
	config.audio.track = ui->audioTrack->currentData().toInt();
	if (ui->audioDevice->count() > 0) {
		config.audio.device = ui->audioDevice->currentData().toString().toStdString();
	}

	if (requireRestart || audioChanged) {
		emit AcceptedAndRestart(config);
	} else {
		emit Accepted(config);
	}
}
