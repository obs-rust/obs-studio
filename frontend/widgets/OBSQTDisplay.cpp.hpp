#include "frontend/build/frontend-widgets-obsqtdisplay.pch.hpp"

static inline long long display_color_to_int(const QColor &color)
{
	auto shift = [&](int val, int shift) {
		return static_cast<long long>(static_cast<unsigned>(val & 0xff)) << shift;
	};

	return shift(color.red(), 0) | shift(color.green(), 8) | shift(color.blue(), 16) | shift(color.alpha(), 24);
}

static inline QColor rgba_to_color(uint32_t rgba)
{
	return QColor::fromRgb(static_cast<int>(rgba & 0xFF), static_cast<int>((rgba >> 8) & 0xFF),
			       static_cast<int>((rgba >> 16) & 0xFF), static_cast<int>((rgba >> 24) & 0xFF));
}

static bool QTToGSWindow(QWindow *window, gs_window &gswindow)
{
	return FillPlatformGraphicsWindow(window, gswindow);
}

OBSQTDisplay::OBSQTDisplay(QWidget *parent, Qt::WindowFlags flags) : QWidget(parent, flags)
{
	setAttribute(Qt::WA_PaintOnScreen);
	setAttribute(Qt::WA_StaticContents);
	setAttribute(Qt::WA_NoSystemBackground);
	setAttribute(Qt::WA_OpaquePaintEvent);
	setAttribute(Qt::WA_DontCreateNativeAncestors);
	setAttribute(Qt::WA_NativeWindow);

	auto windowVisible = [this](bool visible) {
		if (!visible) {
			if (PlatformResetsHiddenDisplay()) {
				display = nullptr;
			}
			return;
		}

		if (!display) {
			CreateDisplay();
		} else {
			QSize size = GetPixelSize(this);
			obs_display_resize(display, static_cast<uint32_t>(size.width()),
					   static_cast<uint32_t>(size.height()));
		}
	};

	auto screenChanged = [this](QScreen *) {
		CreateDisplay();

		QSize size = GetPixelSize(this);
		obs_display_resize(display, static_cast<uint32_t>(size.width()), static_cast<uint32_t>(size.height()));
	};

	connect(windowHandle(), &QWindow::visibleChanged, this, windowVisible);
	connect(windowHandle(), &QWindow::screenChanged, this, screenChanged);

	windowHandle()->installEventFilter(new SurfaceEventFilter(this));
}

QColor OBSQTDisplay::GetDisplayBackgroundColor() const
{
	return rgba_to_color(backgroundColor);
}

void OBSQTDisplay::SetDisplayBackgroundColor(const QColor &color)
{
	uint32_t newBackgroundColor = (uint32_t)display_color_to_int(color);

	if (newBackgroundColor != backgroundColor) {
		backgroundColor = newBackgroundColor;
		UpdateDisplayBackgroundColor();
	}
}

void OBSQTDisplay::UpdateDisplayBackgroundColor()
{
	obs_display_set_background_color(display, backgroundColor);
}

void OBSQTDisplay::CreateDisplay()
{
	if (display) {
		return;
	}

	if (destroying) {
		return;
	}

	if (!windowHandle()->isExposed()) {
		return;
	}

	QSize size = GetPixelSize(this);

	gs_init_data info = {};
	info.cx = static_cast<uint32_t>(size.width());
	info.cy = static_cast<uint32_t>(size.height());
	info.format = GS_BGRA;
	info.zsformat = GS_ZS_NONE;

	if (!QTToGSWindow(windowHandle(), info.window)) {
		return;
	}

	display = obs_display_create(&info, backgroundColor);

	emit DisplayCreated(this);
}

void OBSQTDisplay::paintEvent(QPaintEvent *event)
{
	CreateDisplay();

	QWidget::paintEvent(event);
}

void OBSQTDisplay::moveEvent(QMoveEvent *event)
{
	QWidget::moveEvent(event);

	OnMove();
}

bool OBSQTDisplay::nativeEvent(const QByteArray &, void *message, qintptr *)
{
	HandlePlatformDisplayNativeEvent(this, message);
	return false;
}

void OBSQTDisplay::resizeEvent(QResizeEvent *event)
{
	QWidget::resizeEvent(event);

	CreateDisplay();

	if (isVisible() && display) {
		QSize size = GetPixelSize(this);
		obs_display_resize(display, static_cast<uint32_t>(size.width()), static_cast<uint32_t>(size.height()));
	}

	emit DisplayResized();
}

QPaintEngine *OBSQTDisplay::paintEngine() const
{
	return nullptr;
}

void OBSQTDisplay::OnMove()
{
	if (display) {
		obs_display_update_color_space(display);
	}
}

void OBSQTDisplay::OnDisplayChange()
{
	if (display) {
		obs_display_update_color_space(display);
	}
}
