#include "frontend/build/frontend-dock-layout.pch.hpp"

/* RED (obs-rust/obs-studio#5471): stubs. */

QDockWidget::DockWidgetFeatures DockFeatures(bool, bool)
{
	return QDockWidget::NoDockWidgetFeatures;
}

void ExtraDocks::Add(QDockWidget *, Kind, bool) {}

bool ExtraDocks::Remove(QMainWindow *, const QString &)
{
	return false;
}

bool ExtraDocks::Contains(const QString &) const
{
	return false;
}

bool ExtraDocks::IsEmpty() const
{
	return true;
}

void ExtraDocks::SetLocked(bool) {}

QList<QDockWidget *> ExtraDocks::Docks(Kind) const
{
	return {};
}

std::optional<QString> ExtraDocks::CustomName(const QDockWidget *) const
{
	return std::nullopt;
}
