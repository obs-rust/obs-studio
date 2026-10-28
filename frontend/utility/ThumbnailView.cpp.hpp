/******************************************************************************
    Copyright (C) 2025 by Taylor Giampaolo <warchamp7@obsproject.com>
                          Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "frontend/build/frontend-utility.pch.hpp"

ThumbnailView::ThumbnailView(QObject *parent, const QPointer<ThumbnailItem> &item) : QObject(parent)
{
	if (!parent) {
		deleteLater();
		return;
	}

	if (!item) {
		return;
	}

	uuid = item->getUuid();
	setPixmap(item->getPixmap());

	connect(item, &ThumbnailItem::pixmapUpdated, this, &ThumbnailView::setPixmap);

	if (!item->isValid()) {
		setEnabled(false);
	}
}

void ThumbnailView::setEnabled(bool newEnabled)
{
	if (this->enabled != newEnabled) {
		this->enabled = newEnabled;

		emit enabledChanged(newEnabled);
	}
}

void ThumbnailView::requestUpdate()
{
	if (!this->enabled) {
		return;
	}

	emit updateRequested(uuid);
}

void ThumbnailView::setPixmap(QPixmap newPixmap)
{
	this->pixmap = std::move(newPixmap);

	emit updated(this->pixmap);
}
