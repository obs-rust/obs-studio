#pragma once

#include <stdbool.h>

#include <obs.h>

#ifdef __cplusplus
extern "C" {
#endif

/* The list of missing files OBSBasic::LoadData() collects while it loads a
 * scene collection, without Qt.
 *
 * missing_files_list_create() makes the list LoadData() passes to
 * obs_load_sources() and LoadTransitions(). Only the Missing Files dialog
 * (OBSMissingFiles) frees that list, so when the check is disabled
 * (--disable-missing-files-check) and the dialog is never shown, no list may
 * be made: it returns NULL then.
 *
 * missing_files_list_add_source() is the load and enumeration callback: it
 * appends the missing files of source and of its filters to the list in data,
 * and does nothing when data is NULL. */
obs_missing_files_t *missing_files_list_create(bool check_disabled);
void missing_files_list_add_source(void *data, obs_source_t *source);

#ifdef __cplusplus
}
#endif
