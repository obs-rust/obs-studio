#include "frontend/build/frontend-dock-layout.pch.hpp"

DockLayout::DockLayout(QMainWindow *window) : QObject(window), window(window)
{
#ifdef OBS_FRONTEND_RUST
	model = obs_dock_layout_create();
	window->installEventFilter(this);
	for (QDockWidget *dock : window->findChildren<QDockWidget *>(Qt::FindDirectChildrenOnly)) {
		dock->installEventFilter(this);
	}
#endif
}

DockLayout::~DockLayout()
{
#ifdef OBS_FRONTEND_RUST
	obs_dock_layout_destroy(model);
#endif
}

QByteArray DockLayout::Hash(const QByteArray &blob)
{
	return QCryptographicHash::hash(blob, QCryptographicHash::Sha256).toHex().left(16);
}

void DockLayout::Start(const char *record, const QByteArray &blob)
{
#ifdef OBS_FRONTEND_RUST
	const bool known = obs_dock_layout_start(model, record && *record ? record : nullptr, Hash(blob).constData(),
						 blob.constData(), static_cast<size_t>(blob.size()));
	hasSizes = known;
	blog(LOG_INFO, "Dock layout: restored %lld bytes, %s", static_cast<long long>(blob.size()),
	     known ? "with its sizes" : "no sizes for it yet");
	Queue(OBS_DOCK_LAYOUT_RESTORED);
#else
	UNUSED_PARAMETER(record);
	UNUSED_PARAMETER(blob);
#endif
}

void DockLayout::SetDefault(const DefaultDocks &docks)
{
	defaultDocks = docks;
	defaultLayout = window->saveState();
}

void DockLayout::Reset()
{
	const DefaultDocks &d = defaultDocks;
	window->restoreState(defaultLayout);
	for (QDockWidget *dock : {d.scenes, d.sources, d.mixer, d.transitions, d.controls}) {
		dock->setVisible(true);
	}

	/* The default sizes: the bottom docks get 22.5 % of the height and
	 * share the width 45/14/16, the side docks get 30 % of the width, at
	 * most 280 px. */
	const int cx = window->width();
	const int bottom = window->height() * 225 / 1000;
	const QList<QDockWidget *> bottomDocks{d.mixer, d.transitions, d.controls};
	window->resizeDocks(bottomDocks, {bottom, bottom, bottom}, Qt::Vertical);
	window->resizeDocks(bottomDocks, {cx * 45 / 100, cx * 14 / 100, cx * 16 / 100}, Qt::Horizontal);
	const int side = std::min(cx * 30 / 100, 280);
	window->resizeDocks({d.scenes, d.sources}, {side, side}, Qt::Horizontal);
	/* resizeDocks() only sets the sizes for the next layout pass. Start()
	 * takes the layout as it is now, so lay it out first: otherwise the
	 * model keeps the docks as they were before the sizes (the side docks
	 * at their minimum, #5462). */
	window->layout()->activate();

	/* The reset layout is the user's now, with no known sizes: the layout
	 * this session started with must not come back on a window change or
	 * be saved at quit (obs-rust/obs-studio#5230). */
	Start(nullptr, window->saveState());
}

bool DockLayout::Load(config_t *config, const char *section)
{
	const char *text = config_get_string(config, section, "DockState");
	if (!text || !*text) {
		return false;
	}
	const QByteArray blob = QByteArray::fromBase64(QByteArray(text));
	if (!window->restoreState(blob)) {
		return false;
	}
	Start(config_get_string(config, section, "DockSizes"), blob);
	return true;
}

void DockLayout::Save(config_t *config, const char *section) const
{
	const QByteArray blob = StateToSave();
	config_set_string(config, section, "DockState", blob.toBase64().constData());
	const std::string sizes = SizesRecord(blob);
	if (sizes.empty()) {
		config_remove_value(config, section, "DockSizes");
	} else {
		config_set_string(config, section, "DockSizes", sizes.c_str());
	}
}

QByteArray DockLayout::StateToSave() const
{
#ifdef OBS_FRONTEND_RUST
	size_t size = 0;
	if (const char *blob = obs_dock_layout_blob_to_save(model, &size)) {
		return QByteArray(blob, static_cast<qsizetype>(size));
	}
#endif
	return window->saveState();
}

std::string DockLayout::SizesRecord(const QByteArray &blob) const
{
#ifdef OBS_FRONTEND_RUST
	char *text = obs_dock_layout_record(model, Hash(blob).constData());
	std::string record = text ? text : "";
	obs_dock_layout_free(text);
	return record;
#else
	UNUSED_PARAMETER(blob);
	return {};
#endif
}

bool DockLayout::eventFilter(QObject *obj, QEvent *event)
{
#ifdef OBS_FRONTEND_RUST
	const QEvent::Type type = event->type();
	if (obj == window) {
		if (type == QEvent::ChildAdded) {
			/* A dock added later: a plugin, a browser dock. */
			if (auto *dock = qobject_cast<QDockWidget *>(static_cast<QChildEvent *>(event)->child())) {
				dock->installEventFilter(this);
				Queue(OBS_DOCK_LAYOUT_CHANGE);
			}
		} else if (type == QEvent::MouseButtonPress &&
			   static_cast<QMouseEvent *>(event)->button() == Qt::LeftButton) {
			/* QMainWindow handles its dock separators itself. */
			obs_dock_layout_press(model);
		} else if (type == QEvent::MouseButtonRelease &&
			   static_cast<QMouseEvent *>(event)->button() == Qt::LeftButton) {
			Post(obs_dock_layout_release(model));
		} else if (type == QEvent::Resize || type == QEvent::Show) {
			Queue(OBS_DOCK_LAYOUT_CHANGE);
		}
	} else if (type == QEvent::Show || type == QEvent::Hide || type == QEvent::Resize || type == QEvent::Move) {
		Queue(OBS_DOCK_LAYOUT_CHANGE);
	}
#else
	UNUSED_PARAMETER(obj);
	UNUSED_PARAMETER(event);
#endif
	return false;
}

void DockLayout::Queue(int cause)
{
#ifdef OBS_FRONTEND_RUST
	Post(obs_dock_layout_queue(model, cause));
#else
	UNUSED_PARAMETER(cause);
#endif
}

/* One snapshot per event loop turn, once Qt has placed every dock: the
 * layout resizes the docks of a row one at a time. The model says when a
 * turn is due and what it takes (obs-rust/obs-studio#5475). */
void DockLayout::Post(bool due)
{
	if (due) {
		QMetaObject::invokeMethod(this, [this] { Observe(); }, Qt::QueuedConnection);
	}
}

void DockLayout::Observe()
{
#ifdef OBS_FRONTEND_RUST
	const int cause = obs_dock_layout_turn(model, window->isVisible());
	if (cause < 0) {
		return;
	}

	const QList<QDockWidget *> docks = window->findChildren<QDockWidget *>(Qt::FindDirectChildrenOnly);
	const size_t count = static_cast<size_t>(docks.size());
	std::vector<QByteArray> names;
	std::vector<std::vector<QByteArray>> tabNames;
	std::vector<std::vector<const char *>> tabs;
	names.reserve(count);
	tabNames.reserve(count);
	tabs.reserve(count);
	std::vector<obs_dock_layout_dock> raw;
	raw.reserve(count);
	for (QDockWidget *dock : docks) {
		names.push_back(dock->objectName().toUtf8());
		tabNames.emplace_back();
		for (QDockWidget *tab : window->tabifiedDockWidgets(dock)) {
			tabNames.back().push_back(tab->objectName().toUtf8());
		}
		tabs.emplace_back();
		for (const QByteArray &tab : tabNames.back()) {
			tabs.back().push_back(tab.constData());
		}
		const QSize min = dock->minimumSizeHint().expandedTo(dock->minimumSize());
		const QRect rect = dock->geometry();
		raw.push_back({names.back().constData(), int(window->dockWidgetArea(dock)), dock->isFloating(),
			       dock->isVisible(), rect.x(), rect.y(), rect.width(), rect.height(), min.width(),
			       min.height(), tabs.back().empty() ? nullptr : tabs.back().data(), tabs.back().size()});
	}

	obs_dock_layout_window snap = {window->width(), window->height(), true, 0, 0, 0, 0, 0, 0};
	if (QWidget *central = window->centralWidget()) {
		const QRect rect = central->geometry();
		const QSize min = central->minimumSizeHint().expandedTo(central->minimumSize());
		snap.central_x = rect.x();
		snap.central_y = rect.y();
		snap.central_width = rect.width();
		snap.central_height = rect.height();
		snap.central_min_width = min.width();
		snap.central_min_height = min.height();
	}

	if (obs_dock_layout_observe(model, &snap, raw.data(), raw.size(), cause)) {
		/* No sizes for this layout yet: show it as it was saved, at
		 * the new window size. */
		size_t size = 0;
		const char *blob = obs_dock_layout_restored_blob(model, &size);
		window->restoreState(QByteArray(blob, static_cast<qsizetype>(size)));
		Queue(OBS_DOCK_LAYOUT_RESTORED);
		return;
	}
	Apply();
#endif
}

void DockLayout::Apply()
{
#ifdef OBS_FRONTEND_RUST
	const size_t count = obs_dock_layout_resize_count(model);
	for (size_t i = 0; i < count; ++i) {
		QList<QDockWidget *> docks;
		QList<int> sizes;
		const size_t n = obs_dock_layout_resize_docks(model, i);
		for (size_t j = 0; j < n; ++j) {
			const char *name = obs_dock_layout_resize_name(model, i, j);
			auto *dock = window->findChild<QDockWidget *>(QString::fromUtf8(name ? name : ""),
								      Qt::FindDirectChildrenOnly);
			if (dock) {
				docks.append(dock);
				sizes.append(obs_dock_layout_resize_size(model, i, j));
			}
		}
		const Qt::Orientation orientation = obs_dock_layout_resize_axis(model, i) == OBS_DOCK_LAYOUT_VERTICAL
							    ? Qt::Vertical
							    : Qt::Horizontal;
		if (!docks.isEmpty()) {
			window->resizeDocks(docks, sizes, orientation);
		}
	}
#endif
}
