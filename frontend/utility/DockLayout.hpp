#pragma once

#include <QByteArray>
#include <QObject>

#include <string>

class QDockWidget;
class QMainWindow;
struct config_data;
struct obs_dock_layout;

/* The dock layout of the main window (obs-rust/obs-studio#5230), between Qt
 * and rust/obs-dock-layout. The model keeps the sizes the user wants: the
 * depth of each dock area in pixels and the length of each dock in its row.
 * Only the user changes them, with a separator drag or a dock placement
 * change; a window change (a resize, a tiling window manager's retile) only
 * puts them back with resizeDocks(). The layout itself (which docks, where,
 * floating, hidden, tabs) is Qt's saveState() blob, saved live.
 *
 * A start that has no sizes for the restored blob (the first start after the
 * upgrade, a layout upstream OBS wrote, Reset Docks, a service layout)
 * restores that blob again at each window change, until the window shows it
 * unclamped or the user changes something; until then a save writes that
 * blob, not the squeezed live layout.
 *
 * Without the frontend's Rust (OBS_FRONTEND_RUST, the C rollback build) it is
 * upstream OBS: the live saveState() is saved and no sizes are kept. */
class DockLayout : public QObject {
public:
	explicit DockLayout(QMainWindow *window);
	~DockLayout() override;

	/* The five docks of the default layout. */
	struct DefaultDocks {
		QDockWidget *scenes = nullptr;
		QDockWidget *sources = nullptr;
		QDockWidget *mixer = nullptr;
		QDockWidget *transitions = nullptr;
		QDockWidget *controls = nullptr;
	};

	/* The window shows the default layout now (OBSBasic arranges it
	 * before it restores the user's): Reset() restores this one. */
	void SetDefault(const DefaultDocks &docks);

	/* Reset Docks (obs-rust/obs-studio#5462): restores the default
	 * layout, shows its docks, sizes them for the window and starts the
	 * model with no sizes. The only place the default sizes are set.
	 * Docks outside the default layout keep their state. */
	void Reset();

	/* Restores the layout saved in `section` of `config` (`DockState`,
	 * with its `DockSizes`) and starts the model with it: the start uses
	 * BasicWindow, a service layout its service's section
	 * (obs-rust/obs-studio#5455). False when the section has no layout or
	 * Qt does not accept it; the window is then unchanged. */
	bool Load(struct config_data *config, const char *section);

	/* Writes the layout to save now to `section` of `config`: `DockState`,
	 * and `DockSizes` when the model has sizes for it (removed otherwise). */
	void Save(struct config_data *config, const char *section) const;

	/* The window restored `blob`: `record` is the saved
	 * BasicWindow/DockSizes for it (nullptr or "": none). Load() and
	 * Reset() call it. */
	void Start(const char *record, const QByteArray &blob);

	/* The last Start() got sizes that belong to its blob. */
	bool HasSizes() const { return hasSizes; }

	/* The BasicWindow/DockState to save now. */
	QByteArray StateToSave() const;

	/* The BasicWindow/DockSizes to save with `blob` (from StateToSave()),
	 * empty without the frontend's Rust. */
	std::string SizesRecord(const QByteArray &blob) const;

	/* The hash a DockSizes record names its blob by. */
	static QByteArray Hash(const QByteArray &blob);

protected:
	bool eventFilter(QObject *obj, QEvent *event) override;

private:
	void Queue(int cause);
	void Post(bool due);
	void Observe();
	void Apply();

	QMainWindow *window;
	DefaultDocks defaultDocks;
	/* The default layout, from SetDefault(). */
	QByteArray defaultLayout;
	/* The decisions (rust/obs-dock-layout Session, #5475): the sizes, the
	 * started blob, when to take a snapshot. */
	obs_dock_layout *model = nullptr;
	bool hasSizes = false;
};
