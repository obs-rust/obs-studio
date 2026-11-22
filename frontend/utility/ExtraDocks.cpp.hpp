#include "frontend/build/frontend-dock-layout.pch.hpp"

QDockWidget::DockWidgetFeatures DockFeatures(bool locked, bool closable)
{
	if (locked) {
		return QDockWidget::NoDockWidgetFeatures;
	}

	QDockWidget::DockWidgetFeatures features = QDockWidget::DockWidgetMovable | QDockWidget::DockWidgetFloatable;
	if (closable) {
		features |= QDockWidget::DockWidgetClosable;
	}
	return features;
}

void ExtraDocks::Add(QDockWidget *dock, Kind kind, bool locked)
{
	dock->setFeatures(DockFeatures(locked));

	Entry entry{dock->objectName(), kind, nullptr, dock};
	if (kind == Kind::Plugin) {
		entry.owned.reset(dock);
	}
	entries.push_back(std::move(entry));
}

bool ExtraDocks::Remove(QMainWindow *window, const QString &name)
{
	for (qsizetype i = 0; i < entries.size(); i++) {
		if (entries[i].name != name) {
			continue;
		}

		Entry entry = entries.takeAt(i);
		if (entry.kind == Kind::Plugin) {
			entry.owned.reset();
		} else {
			window->removeDockWidget(entry.dock);
		}
		return true;
	}
	return false;
}

bool ExtraDocks::Contains(const QString &name) const
{
	return std::any_of(entries.begin(), entries.end(), [&](const Entry &entry) { return entry.name == name; });
}

bool ExtraDocks::IsEmpty() const
{
	return entries.isEmpty();
}

void ExtraDocks::SetLocked(bool locked)
{
	for (const Entry &entry : std::as_const(entries)) {
		if (entry.dock) {
			entry.dock->setFeatures(DockFeatures(locked));
		}
	}
}

QList<QDockWidget *> ExtraDocks::Docks(Kind kind) const
{
	QList<QDockWidget *> docks;
	for (const Entry &entry : entries) {
		if (entry.kind == kind && entry.dock) {
			docks.push_back(entry.dock);
		}
	}
	return docks;
}

std::optional<QString> ExtraDocks::CustomName(const QDockWidget *dock) const
{
	for (const Entry &entry : entries) {
		if (entry.kind == Kind::Custom && entry.dock == dock) {
			return entry.name;
		}
	}
	return std::nullopt;
}
