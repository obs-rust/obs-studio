#pragma once

#include <QDockWidget>
#include <QList>
#include <QPointer>
#include <QString>

#include <memory>
#include <optional>

class QMainWindow;

/* The features of a dock (obs-rust/obs-studio#5471): none while the docks are
 * locked, else movable and floatable, and closable unless `closable` is false
 * (the five docks of the default layout). */
QDockWidget::DockWidgetFeatures DockFeatures(bool locked, bool closable = true);

/* The plugin and custom docks of the main window, outside the default layout
 * and Stats (#5471). The browser docks have their own lists (title, URL). */
class ExtraDocks {
public:
	enum class Kind {
		/* obs_frontend_add_dock_by_id(): OBS owns the dock and deletes it
		 * when it is removed and at the end. */
		Plugin,
		/* obs_frontend_add_custom_qdock(): the plugin owns the dock;
		 * OBS only takes it out of the window. */
		Custom,
	};

	/* Keeps `dock` by its object name now and gives it the features for
	 * `locked`. */
	void Add(QDockWidget *dock, Kind kind, bool locked);

	/* Removes the dock added as `name`: a plugin dock is deleted, a custom
	 * dock is taken out of `window`. False when no dock has that name. */
	bool Remove(QMainWindow *window, const QString &name);

	/* A dock was added as `name`. */
	bool Contains(const QString &name) const;

	bool IsEmpty() const;

	/* Gives every dock the features for `locked`. */
	void SetLocked(bool locked);

	/* The docks of `kind`, in the order they were added. */
	QList<QDockWidget *> Docks(Kind kind) const;

	/* The name `dock` was added with when it is a custom dock, else none:
	 * a custom dock gets that name back when the plugin changes it. */
	std::optional<QString> CustomName(const QDockWidget *dock) const;

private:
	struct Entry {
		QString name;
		Kind kind;
		/* Set for a plugin dock only. */
		std::shared_ptr<QDockWidget> owned;
		QPointer<QDockWidget> dock;
	};

	QList<Entry> entries;
};
