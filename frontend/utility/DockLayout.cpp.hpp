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
	restored = blob;
	started = true;
	const bool known = obs_dock_layout_start(model, record && *record ? record : nullptr, Hash(blob).constData());
	hasSizes = known;
	blog(LOG_INFO, "Dock layout: restored %lld bytes, %s", static_cast<long long>(blob.size()),
	     known ? "with its sizes" : "no sizes for it yet");
	Queue(OBS_DOCK_LAYOUT_RESTORED);
#else
	UNUSED_PARAMETER(record);
	UNUSED_PARAMETER(blob);
#endif
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
	if (obs_dock_layout_saves_restored_blob(model) && !restored.isEmpty()) {
		return restored;
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
			dragging = true;
		} else if (type == QEvent::MouseButtonRelease &&
			   static_cast<QMouseEvent *>(event)->button() == Qt::LeftButton && dragging) {
			dragging = false;
			Queue(OBS_DOCK_LAYOUT_SEPARATOR_DRAG);
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

/* One snapshot per event loop turn, once Qt has placed every dock: the
 * layout resizes the docks of a row one at a time. */
void DockLayout::Queue(int cause)
{
	queued = std::max(queued, cause);
	if (!observeQueued) {
		observeQueued = true;
		QMetaObject::invokeMethod(this, [this] { Observe(); }, Qt::QueuedConnection);
	}
}

void DockLayout::Observe()
{
	observeQueued = false;
#ifdef OBS_FRONTEND_RUST
	if (!started) {
		queued = -1;
		return;
	}
	if (dragging || queued < 0) {
		/* The release queues the drag's snapshot. */
		return;
	}
	if (!window->isVisible()) {
		/* Minimized to the tray, or not shown yet: nothing to decide.
		 * A restore is looked at once the window shows. */
		if (queued != OBS_DOCK_LAYOUT_RESTORED) {
			queued = -1;
		}
		return;
	}
	const int cause = std::exchange(queued, -1);

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
		window->restoreState(restored);
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
