#pragma once

/* C ABI of rust/obs-dock-layout (src/ffi/mod.rs), linked into the frontend
 * through rust/obs-frontend when OBS_FRONTEND_RUST is defined
 * (obs-rust/obs-studio#5230). One model per main window, used from the UI
 * thread. The model keeps the dock sizes the user wants and says how to put
 * them back after a window change; DockLayout.hpp is the Qt side. */

#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Dock areas, the values of Qt::DockWidgetArea; anything else is none. */
#define OBS_DOCK_LAYOUT_LEFT 0x1
#define OBS_DOCK_LAYOUT_RIGHT 0x2
#define OBS_DOCK_LAYOUT_TOP 0x4
#define OBS_DOCK_LAYOUT_BOTTOM 0x8

/* Axes, the values of Qt::Orientation. */
#define OBS_DOCK_LAYOUT_HORIZONTAL 0x1
#define OBS_DOCK_LAYOUT_VERTICAL 0x2

/* Why a snapshot is taken. */
#define OBS_DOCK_LAYOUT_CHANGE 0
#define OBS_DOCK_LAYOUT_SEPARATOR_DRAG 1
#define OBS_DOCK_LAYOUT_RESTORED 2

struct obs_dock_layout;

struct obs_dock_layout_dock {
	const char *name;
	int area;
	bool floating;
	bool shown;
	int x;
	int y;
	int width;
	int height;
	int min_width;
	int min_height;
	/* The other docks of its tab group; NULL when tab_count is 0. */
	const char *const *tabs;
	size_t tab_count;
};

struct obs_dock_layout_window {
	int width;
	int height;
	bool visible;
	/* The central widget's geometry and minimum size. */
	int central_x;
	int central_y;
	int central_width;
	int central_height;
	int central_min_width;
	int central_min_height;
};

/* A new model. Free it with obs_dock_layout_destroy. */
struct obs_dock_layout *obs_dock_layout_create(void);
void obs_dock_layout_destroy(struct obs_dock_layout *layout);

/* A restored layout starts: `record` is the saved BasicWindow/DockSizes
 * (NULL: none), `blob_hash` the hash of the DockState blob that was
 * restored. Whether the record belongs to that blob. */
bool obs_dock_layout_start(struct obs_dock_layout *layout, const char *record, const char *blob_hash);

/* The window after a change, with its `count` docks and the cause
 * (OBS_DOCK_LAYOUT_*). Returns whether to restore the started blob again
 * (no known sizes yet). The resizes to apply are read with
 * obs_dock_layout_resize_* until the next call. */
bool obs_dock_layout_observe(struct obs_dock_layout *layout, const struct obs_dock_layout_window *window,
			     const struct obs_dock_layout_dock *docks, size_t count, int cause);

/* Resize i: its axis (0 when there is none), its docks by object name (valid
 * until the next call that changes `layout`) and their sizes. */
size_t obs_dock_layout_resize_count(const struct obs_dock_layout *layout);
int obs_dock_layout_resize_axis(const struct obs_dock_layout *layout, size_t i);
size_t obs_dock_layout_resize_docks(const struct obs_dock_layout *layout, size_t i);
const char *obs_dock_layout_resize_name(const struct obs_dock_layout *layout, size_t i, size_t j);
int obs_dock_layout_resize_size(const struct obs_dock_layout *layout, size_t i, size_t j);

/* Whether a save now writes the started blob, not the live layout: no sizes
 * are known yet and the user changed nothing. */
bool obs_dock_layout_saves_restored_blob(const struct obs_dock_layout *layout);

/* The BasicWindow/DockSizes record for the blob with hash `blob_hash`. Free
 * it with obs_dock_layout_free. */
char *obs_dock_layout_record(const struct obs_dock_layout *layout, const char *blob_hash);
void obs_dock_layout_free(char *s);

#ifdef __cplusplus
}
#endif
