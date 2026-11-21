#pragma once

#include <QByteArray>
#include <QObject>

#include <string>

class QMainWindow;
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

	/* The window restored `blob`: `record` is the saved
	 * BasicWindow/DockSizes for it (nullptr or "": none). */
	void Start(const char *record, const QByteArray &blob);

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
	void Observe();
	void Apply();

	QMainWindow *window;
	obs_dock_layout *model = nullptr;
	/* The blob of the last Start(): what a restore again restores. */
	QByteArray restored;
	/* Start() was called: before it there is no layout of the user's to
	 * keep (OBSBasic sets its default layout up first). */
	bool started = false;
	/* The strongest cause queued since the last snapshot (RESTORED over
	 * SEPARATOR_DRAG over CHANGE), -1 when none is queued. */
	int queued = -1;
	bool observeQueued = false;
	/* The left button is down on the window: a separator drag runs. Qt
	 * resizes the docks while it moves; those are the user's sizes, so
	 * no snapshot is taken until the release. */
	bool dragging = false;
};
