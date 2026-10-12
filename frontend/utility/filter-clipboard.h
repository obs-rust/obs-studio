#pragma once

#include <obs.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Paste Filters (source list > Paste Filters), without Qt.
 *
 * filter_clipboard_paste() copies the filters of src onto the source of every
 * selected item of scene, including the selected items inside its groups.
 * Each source gets the filters once, however many selected items use it (an
 * item and its references, for example), and src itself is skipped.
 *
 * For undo, undo and redo (when not NULL) receive a "sources" array with one
 * entry per pasted-to source: its "uuid" and its filters ("array", as from
 * obs_source_backup_filters()) before and after the paste. Returns the number
 * of sources the filters were pasted to.
 *
 * filter_clipboard_restore() puts back the filters saved in undo or redo and
 * calls restored(source, param), when set, for each restored source. */
size_t filter_clipboard_paste(obs_scene_t *scene, obs_source_t *src, obs_data_t *undo, obs_data_t *redo);
void filter_clipboard_restore(obs_data_t *data, void (*restored)(obs_source_t *source, void *param), void *param);

#ifdef __cplusplus
}
#endif
