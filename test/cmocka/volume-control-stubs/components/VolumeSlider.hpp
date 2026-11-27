#pragma once

/* Stand-in for frontend/components/VolumeSlider.hpp (test_volume_control_uuid):
 * a plain QSlider, so VolumeControl builds without AbsoluteSlider. */

#include <obs.hpp>

#include <QSlider>

class VolumeSlider : public QSlider {
public:
	obs_fader_t *fad;

	VolumeSlider(obs_fader_t *fader, Qt::Orientation orientation, QWidget *parent = nullptr)
		: QSlider(orientation, parent),
		  fad(fader)
	{
	}

	void setDisplayTicks(bool) {}
};
