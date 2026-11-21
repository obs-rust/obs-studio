#include "frontend/build/frontend-dock-layout.pch.hpp"

/* RED (#5230): the adapter's interface with upstream behavior. The model is
 * not asked yet: the live layout is saved and no sizes are kept. */

DockLayout::DockLayout(QMainWindow *window) : QObject(window), window(window) {}

DockLayout::~DockLayout() {}

QByteArray DockLayout::Hash(const QByteArray &blob)
{
	return QCryptographicHash::hash(blob, QCryptographicHash::Sha256).toHex().left(16);
}

void DockLayout::Start(const char *record, const QByteArray &blob)
{
	UNUSED_PARAMETER(record);
	restored = blob;
	started = true;
}

QByteArray DockLayout::StateToSave() const
{
	return window->saveState();
}

std::string DockLayout::SizesRecord(const QByteArray &blob) const
{
	UNUSED_PARAMETER(blob);
	return {};
}

bool DockLayout::eventFilter(QObject *obj, QEvent *event)
{
	UNUSED_PARAMETER(obj);
	UNUSED_PARAMETER(event);
	return false;
}

void DockLayout::Queue(int cause)
{
	queued = std::max(queued, cause);
}

void DockLayout::Observe()
{
	observeQueued = false;
}

void DockLayout::Apply() {}
