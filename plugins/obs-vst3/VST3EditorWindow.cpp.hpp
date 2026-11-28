/******************************************************************************
    Copyright (C) 2025-2026 pkv <pkv@obsproject.com>
    This file is part of obs-vst3.
    It uses the Steinberg VST3 SDK, which is licensed under MIT license.
    See https://github.com/steinbergmedia/vst3sdk for details.
    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.
    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.
    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "plugins/obs-vst3/build/obs-vst3.editor.pch.hpp"

using namespace Steinberg;

namespace {

void reportError(const char *message)
{
	std::fprintf(stderr, "VST3 editor error: %s\n", message);
}

} // namespace

#ifndef OBS_VST3_RUST_HOST
class VST3EditorWindow::PlugFrameImpl final : public IPlugFrame {
public:
	explicit PlugFrameImpl(VST3EditorWindow *window) : window_(window) {}
	tresult PLUGIN_API resizeView(IPlugView *view, ViewRect *newSize) override
	{
		if (!window_ || !view || view != window_->view_ || !newSize || newSize->getWidth() <= 0 ||
		    newSize->getHeight() <= 0) {
			return kInvalidArgument;
		}

		return window_->resizeFromPlugin(*newSize);
	}
	tresult PLUGIN_API queryInterface(const TUID _iid, void **obj) override
	{
		if (FUnknownPrivate::iidEqual(_iid, FUnknown::iid)) {
			*obj = static_cast<FUnknown *>(static_cast<IPlugFrame *>(this));
			return kResultOk;
		}
		if (FUnknownPrivate::iidEqual(_iid, IPlugFrame::iid)) {
			*obj = static_cast<IPlugFrame *>(this);
			return kResultOk;
		}
		return queryPlatformFrameInterface(_iid, obj);
	}
	// refcounting does not matter here, cf SDK
	uint32_t PLUGIN_API addRef() override { return 1; }
	uint32_t PLUGIN_API release() override { return 1; }

private:
	VST3EditorWindow *window_;
};

#endif

VST3EditorWindow::VST3EditorWindow(VST3EditorView *view, const std::string &title) : QWidget(nullptr), view_(view)
{
	viewContainer_ = new QWidget(this);

	resizable_ = canResizeView();

	Qt::WindowFlags flags = Qt::Tool | Qt::WindowTitleHint | Qt::WindowCloseButtonHint |
				Qt::WindowMinimizeButtonHint;

	if (resizable_) {
		flags |= Qt::WindowMaximizeButtonHint;
	}

	auto *main = static_cast<QMainWindow *>(obs_frontend_get_main_window());
	QWidget::setParent(main, flags);

	setWindowTitle(QString::fromUtf8(title.data(), static_cast<qsizetype>(title.size())));

	initializePlatformWindow();
}

#ifdef OBS_VST3_RUST_HOST
VST3EditorWindow::~VST3EditorWindow()
{
	detach();
}

void VST3EditorWindow::detach()
{
	if (std::exchange(attached_, false)) {
		vst3_rust_editor_detach(view_);
	}
}
#else
VST3EditorWindow::~VST3EditorWindow() = default;
#endif

bool VST3EditorWindow::create(int width, int height, [[maybe_unused]] FUnknown *hostContext)
{
	if (!view_ || attached_ || width <= 0 || height <= 0) {
		return false;
	}

	ViewRect initialSize(0, 0, width, height);
	constrainView(&initialSize);
	const QSize qtInitialSize = vst3ToQtSize(initialSize);
	if (resizable_) {
		resize(qtInitialSize);
	} else {
		setFixedSize(qtInitialSize);
	}

	viewContainer_->setGeometry(0, 0, qtInitialSize.width(), qtInitialSize.height());

	QWidget::create();
	if (resizable_) {
		preparePlatformView(qtInitialSize);
	}
	const WId nativeParentId = viewContainer_->winId();
	if (!windowHandle() || !nativeParentId) {
		reportError("failed to create the native window");
		return false;
	}

	connect(windowHandle(), &QWindow::screenChanged, this, [this] { handleScaleChange(); });

	void *nativeParent = reinterpret_cast<void *>(nativeParentId);
#ifdef OBS_VST3_RUST_HOST
	const auto resize = [](void *context, const ViewRect *rect) -> tresult {
		return static_cast<VST3EditorWindow *>(context)->resizeFromPlugin(*rect);
	};
	const bool attached =
		vst3_rust_editor_attach(view_, nativeParent, platformViewType(), this, resize, hostContext);
	if (!attached) {
		reportError("failed to attach the plug-in view");
		return false;
	}
#else
	frame_ = std::make_unique<PlugFrameImpl>(this);
	view_->setFrame(frame_.get());
	if (view_->attached(nativeParent, platformViewType()) != kResultOk) {
		view_->setFrame(nullptr);
		frame_.reset();
		reportError("failed to attach the plug-in view");

		return false;
	}

#endif
	attached_ = true;

	ViewRect windowSize = qtToVst3Rect(viewContainer_->size());
	notifyViewSize(&windowSize);

	(void)updateContentScaleFactor();

	return true;
}

void VST3EditorWindow::show()
{
	if (!attached_) {
		return;
	}

	QWidget::show();
	raise();
	activateWindow();

	wasClosed_ = false;

	handleScaleChange();
}

void VST3EditorWindow::close()
{
	hide();
}

bool VST3EditorWindow::getClosedState() const
{
	return wasClosed_;
}

bool VST3EditorWindow::event(QEvent *event)
{
#if QT_VERSION >= QT_VERSION_CHECK(6, 6, 0)
	if (event->type() == QEvent::DevicePixelRatioChange && usesContentScaleFactor()) {
		handlingDpiChange_ = true;

		const bool handled = QWidget::event(event);
		const bool scaleChanged = updateContentScaleFactor();

		handlingDpiChange_ = false;

		if (scaleChanged && attached_) {
			ViewRect actualRect = qtToVst3Rect(viewContainer_->size());
			notifyViewSize(&actualRect);
		}

		return handled;
	}
#endif
	return QWidget::event(event);
}

void VST3EditorWindow::closeEvent(QCloseEvent *event)
{
	wasClosed_ = true;
	hide();
	event->ignore();
}

void VST3EditorWindow::resizeEvent(QResizeEvent *event)
{
	QWidget::resizeEvent(event);
	viewContainer_->setGeometry(0, 0, event->size().width(), event->size().height());

	if (!view_ || !attached_ || resizingFromPlugin_ || correctingHostResize_ || handlingDpiChange_) {
		return;
	}

	handlePlatformResize(event->size());
}

bool VST3EditorWindow::sameSize(const ViewRect &lhs, const ViewRect &rhs)
{
	return lhs.getWidth() == rhs.getWidth() && lhs.getHeight() == rhs.getHeight();
}

QSize VST3EditorWindow::vst3ToQtSize(const ViewRect &rect) const
{
	const qreal scale = vst3CoordinateScaleFactor();
	const int width = std::max(1, static_cast<int>(std::lround(rect.getWidth() / scale)));
	const int height = std::max(1, static_cast<int>(std::lround(rect.getHeight() / scale)));

	return {width, height};
}

ViewRect VST3EditorWindow::qtToVst3Rect(const QSize &size) const
{
	const qreal scale = vst3CoordinateScaleFactor();
	const auto width = static_cast<int32>(std::lround(size.width() * scale));
	const auto height = static_cast<int32>(std::lround(size.height() * scale));

	return {0, 0, std::max<int32>(1, width), std::max<int32>(1, height)};
}

tresult PLUGIN_API VST3EditorWindow::resizeFromPlugin(const ViewRect &rect)
{
#ifndef OBS_VST3_RUST_HOST
	if (!view_ || resizeViewRecursionGuard_ || rect.getWidth() <= 0 || rect.getHeight() <= 0) {
		return kResultFalse;
	}
#endif

	const QSize targetSize = vst3ToQtSize(rect);

	if (viewContainer_->size() == targetSize) {
		return kResultTrue;
	}

#ifndef OBS_VST3_RUST_HOST
	resizeViewRecursionGuard_ = true;
#endif
	resizingFromPlugin_ = true;

	if (resizable_) {
		resize(targetSize);
	} else {
		setFixedSize(targetSize);
	}

	resizingFromPlugin_ = false;

	ViewRect actualRect = qtToVst3Rect(viewContainer_->size());
	notifyViewSize(&actualRect);

#ifndef OBS_VST3_RUST_HOST
	resizeViewRecursionGuard_ = false;
#endif

	return kResultTrue;
}

bool VST3EditorWindow::updateContentScaleFactor()
{
	if (!usesContentScaleFactor()) {
		return false;
	}

	const qreal scale = devicePixelRatioF();

	if (contentScaleFactor_ == scale) {
		return false;
	}

	contentScaleFactor_ = scale;

#ifdef OBS_VST3_RUST_HOST
	vst3_rust_editor_scale(view_, static_cast<float>(scale));
#else
	FUnknownPtr<IPlugViewContentScaleSupport> scaleSupport(view_);

	if (scaleSupport) {
		scaleSupport->setContentScaleFactor(static_cast<IPlugViewContentScaleSupport::ScaleFactor>(scale));
	}
#endif

	return true;
}

void VST3EditorWindow::handleScaleChange()
{
	if (updateContentScaleFactor() && attached_) {
		ViewRect currentSize = qtToVst3Rect(viewContainer_->size());
		notifyViewSize(&currentSize);
	}
}

bool VST3EditorWindow::canResizeView() const
{
#ifdef OBS_VST3_RUST_HOST
	return vst3_rust_editor_can_resize(view_);
#else
	return view_->canResize() == kResultTrue;
#endif
}

tresult VST3EditorWindow::constrainView(ViewRect *rect) const
{
#ifdef OBS_VST3_RUST_HOST
	return vst3_rust_editor_constrain(view_, rect);
#else
	return view_->checkSizeConstraint(rect);
#endif
}

void VST3EditorWindow::notifyViewSize(ViewRect *rect) const
{
#ifdef OBS_VST3_RUST_HOST
	(void)vst3_rust_editor_on_size(view_, rect);
#else
	view_->onSize(rect);
#endif
}
