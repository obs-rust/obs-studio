#pragma once

/* Keeps the relative sizes of docks that share a row in the main window
 * when the window shrinks below the docks' minimum sizes and grows again
 * (obs-rust/obs-studio#2225). Header-only, Qt Widgets only, so it can be
 * unit tested without the frontend.
 *
 * Qt scales docks in proportion while the window resizes, until a dock
 * reaches its minimum size. From then on the docks at their minimum stay
 * there, and when the window grows again Qt shares the new space out
 * equally: a layout the user sized by hand comes back with every dock the
 * same size. This helper remembers each row's shares while no dock in it is
 * at its minimum and puts them back when the window grows.
 *
 * A row whose docks change size but whose total stays the same was resized
 * from inside (a dragged separator): its shares are recorded. A row whose
 * total changed was resized with the window: its saved shares are put back
 * once they fit.
 *
 * The same goes for a whole dock area against the window: a column of docks
 * on the left or right (Scenes above Sources by default) shrinks with a
 * narrowing window, and Qt gives all of the width back to the preview when
 * the window widens again. Each area's share of the window is recorded when
 * the area changes size on its own (a dragged edge) and put back when the
 * window changes size. */

#include <QByteArray>
#include <QDockWidget>
#include <QEvent>
#include <QList>
#include <QMainWindow>
#include <QObject>

#include <algorithm>
#include <cmath>
#include <map>
#include <utility>

class DockProportions : public QObject {
public:
	explicit DockProportions(QMainWindow *window) : QObject(window), window(window)
	{
		window->installEventFilter(this);
		for (QDockWidget *dock : window->findChildren<QDockWidget *>(Qt::FindDirectChildrenOnly)) {
			dock->installEventFilter(this);
		}
	}

	/* The dock state to write to the user's ini (#2225). A layout that is
	 * clamped by a narrow window serializes with every dock at its
	 * minimum, and the next start restores that clamped layout at full
	 * size. The state saved is therefore the widest layout this window
	 * last showed unclamped; a window that is currently unclamped saves
	 * itself. */
	QByteArray StateToSave() const
	{
		if (unclamped.isEmpty() || !AnyDockAtMinimum()) {
			return window->saveState();
		}
		return unclamped;
	}

protected:
	bool eventFilter(QObject *obj, QEvent *event) override
	{
		if (obj == window && event->type() == QEvent::ChildAdded) {
			auto *child = static_cast<QChildEvent *>(event)->child();
			if (auto *dock = qobject_cast<QDockWidget *>(child)) {
				dock->installEventFilter(this);
			}
		} else if (event->type() == QEvent::Resize && !updateQueued) {
			/* The layout resizes the docks of a row one at a time:
			 * look at the row once it has placed all of them. The
			 * window's own resize counts too: a widening window can
			 * leave every dock as it was. */
			updateQueued = true;
			QMetaObject::invokeMethod(this, [this] { Update(); }, Qt::QueuedConnection);
		}
		return false;
	}

private:
	struct Row {
		Qt::Orientation orientation;
		QList<QDockWidget *> docks;
	};

	struct Saved {
		int total;
		QList<double> shares;
	};

	/* A dock area across its depth: the width of a left or right area,
	 * the height of a top or bottom one. */
	struct Area {
		Qt::DockWidgetArea area;
		Qt::Orientation orientation;
		QList<QDockWidget *> docks;
	};

	struct SavedArea {
		int window;
		int size;
		double share;
	};

	QMainWindow *window;
	/* The layout as it last was with no dock at its minimum: what a save
	 * from a narrow window writes instead of the clamp. */
	QByteArray unclamped;
	std::map<QList<QDockWidget *>, Saved> saved;
	std::map<int, SavedArea> savedAreas;
	bool updateQueued = false;

	/* Docks side by side in the same area: along the top or bottom they
	 * share a top edge and split the width; on the left or right they share
	 * a left edge and split the height. */
	QList<Row> Rows() const
	{
		std::map<std::pair<int, int>, Row> rows;
		for (QDockWidget *dock : window->findChildren<QDockWidget *>(Qt::FindDirectChildrenOnly)) {
			if (dock->isFloating() || !dock->isVisible()) {
				continue;
			}
			Qt::DockWidgetArea area = window->dockWidgetArea(dock);
			bool horizontal = area == Qt::TopDockWidgetArea || area == Qt::BottomDockWidgetArea;
			if (!horizontal && area != Qt::LeftDockWidgetArea && area != Qt::RightDockWidgetArea) {
				continue;
			}
			int edge = horizontal ? dock->y() : dock->x();
			Row &row = rows[{int(area), edge}];
			row.orientation = horizontal ? Qt::Horizontal : Qt::Vertical;
			row.docks.append(dock);
		}

		QList<Row> result;
		for (auto &entry : rows) {
			Row &row = entry.second;
			if (row.docks.size() < 2) {
				continue;
			}
			const bool horizontal = row.orientation == Qt::Horizontal;
			std::sort(row.docks.begin(), row.docks.end(), [horizontal](QDockWidget *a, QDockWidget *b) {
				return horizontal ? a->x() < b->x() : a->y() < b->y();
			});
			result.append(row);
		}
		return result;
	}

	static int Size(const QDockWidget *dock, Qt::Orientation orientation)
	{
		return orientation == Qt::Horizontal ? dock->width() : dock->height();
	}

	static int Minimum(const QDockWidget *dock, Qt::Orientation orientation)
	{
		QSize min = dock->minimumSizeHint().expandedTo(dock->minimumSize());
		return orientation == Qt::Horizontal ? min.width() : min.height();
	}

	/* Areas whose docks all start at the same edge, so the first dock's
	 * depth is the area's. An area with docks side by side across its
	 * depth is left to Qt. */
	QList<Area> Areas() const
	{
		std::map<int, Area> areas;
		for (QDockWidget *dock : window->findChildren<QDockWidget *>(Qt::FindDirectChildrenOnly)) {
			if (dock->isFloating() || !dock->isVisible()) {
				continue;
			}
			Qt::DockWidgetArea area = window->dockWidgetArea(dock);
			bool vertical = area == Qt::TopDockWidgetArea || area == Qt::BottomDockWidgetArea;
			if (!vertical && area != Qt::LeftDockWidgetArea && area != Qt::RightDockWidgetArea) {
				continue;
			}
			Area &entry = areas[int(area)];
			entry.area = area;
			entry.orientation = vertical ? Qt::Vertical : Qt::Horizontal;
			entry.docks.append(dock);
		}

		QList<Area> result;
		for (auto &entry : areas) {
			const Area &area = entry.second;
			const bool horizontal = area.orientation == Qt::Horizontal;
			const QDockWidget *first = area.docks.first();
			bool aligned = std::all_of(area.docks.begin(), area.docks.end(), [&](const QDockWidget *dock) {
				return horizontal ? dock->x() == first->x() && dock->width() == first->width()
						  : dock->y() == first->y() && dock->height() == first->height();
			});
			if (aligned) {
				result.append(area);
			}
		}
		return result;
	}

	void Update()
	{
		updateQueued = false;
		UpdateRows();
		UpdateAreas();
		/* A dock pushed to its minimum means the window cannot show the
		 * layout the user sized, so saveState() now is that clamp. */
		if (!AnyDockAtMinimum()) {
			unclamped = window->saveState();
		}
	}

	bool AnyDockAtMinimum() const
	{
		for (QDockWidget *dock : window->findChildren<QDockWidget *>(Qt::FindDirectChildrenOnly)) {
			if (dock->isFloating() || !dock->isVisible()) {
				continue;
			}
			Qt::DockWidgetArea area = window->dockWidgetArea(dock);
			const bool horizontal = area == Qt::TopDockWidgetArea || area == Qt::BottomDockWidgetArea;
			const Qt::Orientation orientation = horizontal ? Qt::Horizontal : Qt::Vertical;
			if (Size(dock, orientation) <= Minimum(dock, orientation) + 1) {
				return true;
			}
		}
		return false;
	}

	void UpdateAreas()
	{
		for (const Area &area : Areas()) {
			const int extent = area.orientation == Qt::Horizontal ? window->width() : window->height();
			const int size = Size(area.docks.first(), area.orientation);
			int minimum = 0;
			for (QDockWidget *dock : area.docks) {
				minimum = std::max(minimum, Minimum(dock, area.orientation));
			}
			if (extent <= 0 || size <= 0) {
				continue;
			}

			auto it = savedAreas.find(int(area.area));
			if (it == savedAreas.end()) {
				savedAreas[int(area.area)] = {extent, size, double(size) / extent};
				continue;
			}
			SavedArea &s = it->second;
			if (s.window == extent) {
				/* Same window, new size: a dragged edge. */
				if (s.size != size) {
					s = {extent, size, double(size) / extent};
				}
				continue;
			}

			/* Resized with the window: put the share back once it
			 * fits, and otherwise wait for the window to grow. */
			s.window = extent;
			s.size = size;
			const int target = int(std::lround(s.share * extent));
			if (std::abs(target - size) > 1 && target > minimum + 1) {
				s.size = target;
				window->resizeDocks({area.docks.first()}, {target}, area.orientation);
			}
		}
	}

	void UpdateRows()
	{
		for (const Row &row : Rows()) {
			int total = 0;
			bool clamped = false;
			for (QDockWidget *dock : row.docks) {
				total += Size(dock, row.orientation);
				clamped = clamped || Size(dock, row.orientation) <= Minimum(dock, row.orientation) + 1;
			}
			if (total <= 0) {
				continue;
			}

			auto it = saved.find(row.docks);
			if (it == saved.end() || it->second.total == total) {
				/* New row, or resized from inside. A dock pushed to
				 * its minimum says nothing about the shares wanted. */
				if (!clamped) {
					saved[row.docks] = {total, Shares(row, total)};
				}
				continue;
			}

			/* Resized with the window. Keep the shares; the next
			 * change at this total comes from inside the row. */
			it->second.total = total;
			const QList<double> &shares = it->second.shares;
			bool differs = false, fits = true;
			QList<int> sizes;
			for (qsizetype i = 0; i < row.docks.size(); i++) {
				double share = double(Size(row.docks[i], row.orientation)) / total;
				differs = differs || std::abs(share - shares[i]) > 0.01;
				sizes.append(int(std::lround(shares[i] * total)));
				fits = fits && sizes.last() > Minimum(row.docks[i], row.orientation) + 1;
			}
			/* Too small for the saved shares: wait for the window to
			 * grow again. */
			if (differs && fits) {
				window->resizeDocks(row.docks, sizes, row.orientation);
			}
		}
	}

	static QList<double> Shares(const Row &row, int total)
	{
		QList<double> shares;
		for (QDockWidget *dock : row.docks) {
			shares.append(double(Size(dock, row.orientation)) / total);
		}
		return shares;
	}
};
