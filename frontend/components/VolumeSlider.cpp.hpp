#include "frontend/build/frontend-components.pch.hpp"

VolumeSlider::VolumeSlider(obs_fader_t *fader, QWidget *parent) : AbsoluteSlider(parent)
{
	fad = fader;
}

VolumeSlider::VolumeSlider(obs_fader_t *fader, Qt::Orientation orientation, QWidget *parent)
	: AbsoluteSlider(orientation, parent)
{
	fad = fader;
}

bool VolumeSlider::getDisplayTicks() const
{
	return displayTicks;
}

void VolumeSlider::setDisplayTicks(bool display)
{
	displayTicks = display;
}

void VolumeSlider::paintEvent(QPaintEvent *event)
{
	if (!getDisplayTicks()) {
		QSlider::paintEvent(event);
		return;
	}

	QPainter painter(this);
	QColor tickMarkColor(91, 98, 115, 255);

	obs_fader_conversion_t fader_db_to_def = obs_fader_db_to_def(fad);

	QStyleOptionSlider opt;
	initStyleOption(&opt);

	QRect groove = style()->subControlRect(QStyle::CC_Slider, &opt, QStyle::SC_SliderGroove, this);
	QRect handle = style()->subControlRect(QStyle::CC_Slider, &opt, QStyle::SC_SliderHandle, this);

	if (orientation() == Qt::Horizontal) {
		const int sliderWidth = groove.width() - handle.width();

		float tickLength = static_cast<float>(groove.height()) * 1.5f;
		tickLength = static_cast<float>(
			std::max(static_cast<int>(tickLength) + groove.height(), 8 + groove.height()));

		float yPos = static_cast<float>(groove.center().y()) - (tickLength / 2.0f) + 1.0f;

		for (int db = -10; db >= -90; db -= 10) {
			float tickValue = fader_db_to_def(static_cast<float>(db));

			float xPos = static_cast<float>(groove.left()) + (tickValue * static_cast<float>(sliderWidth)) +
				     (static_cast<float>(handle.width()) / 2.0f);
			painter.fillRect(QRectF(xPos, yPos, 1.0f, tickLength), tickMarkColor);
		}
	}

	if (orientation() == Qt::Vertical) {
		const int sliderHeight = groove.height() - handle.height();

		float tickLength = static_cast<float>(groove.width()) * 1.5f;
		tickLength =
			static_cast<float>(std::max(static_cast<int>(tickLength) + groove.width(), 8 + groove.width()));

		float xPos = static_cast<float>(groove.center().x()) - (tickLength / 2.0f) + 1.0f;

		for (int db = -10; db >= -96; db -= 10) {
			float tickValue = fader_db_to_def(static_cast<float>(db));

			float yPos = static_cast<float>(groove.height()) + static_cast<float>(groove.top()) -
				     (tickValue * static_cast<float>(sliderHeight)) -
				     (static_cast<float>(handle.height()) / 2.0f);
			painter.fillRect(QRectF(xPos, yPos, tickLength, 1.0f), tickMarkColor);
		}
	}

	QSlider::paintEvent(event);
}
