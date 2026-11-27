#pragma once

/* Narrow stand-in for the frontend components PCH (test_volume_control_uuid):
 * what VolumeControl.cpp.hpp and VolumeName.cpp.hpp need, with OBSBasic,
 * OBSApp, the meter, the name dialog and message boxes replaced. */

#include "frontend/components/VolumeControl.hpp"

#include "volume-control-main.hpp"

#include <QBoxLayout>
#include <QCheckBox>
#include <QGuiApplication>
#include <QLabel>
#include <QMenu>
#include <QMessageBox>
#include <QObjectCleanupHandler>
#include <QResizeEvent>
#include <QScreen>
#include <QStyle>
#include <QStyleOption>
#include <QStylePainter>
#include <QTimer>

Q_DECLARE_METATYPE(OBSSource);

#define QT_UTF8(str) QString::fromUtf8(str, -1)

/* From frontend/components/VolumeMeter.hpp. */
#define FADER_PRECISION 4096.0

class VolumeMeter : public QWidget {
public:
	explicit VolumeMeter(QWidget *parent = nullptr, obs_source_t * = nullptr) : QWidget(parent) {}

	void setLevels(const float[MAX_AUDIO_CHANNELS], const float[MAX_AUDIO_CHANNELS],
		       const float[MAX_AUDIO_CHANNELS])
	{
	}
	void setVertical(bool = true) {}
	void setUseDisabledColors(bool) {}
	void setMuted(bool) {}
	void refreshColors() {}
	void setPeakDecayRate(qreal) {}
	void setPeakMeterType(enum obs_peak_meter_type) {}
	void updateBackgroundCache(bool = false) {}
};

class NameDialog {
public:
	static bool AskForName(QWidget *, const QString &, const QString &, std::string &, const QString & = QString())
	{
		return false;
	}
};

class OBSMessageBox {
public:
	static void warning(QWidget *, const QString &, const QString &, bool = false) {}
};
