#pragma once

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Removing a profile, kept free of Qt so it can be unit tested. Profiles are
 * opaque to the helper: the frontend knows which profile is being removed.
 * OBS_FRONTEND_EVENT_PROFILE_LIST_CHANGED listeners (obs-websocket's
 * ProfileListChanged, ...) are told about the change. */

struct profile_remove_ops {
	void *param;

	/* Asks the user to confirm removing the current profile. Returns false
	 * if they said no. */
	bool (*confirm)(void *param);

	/* Emits OBS_FRONTEND_EVENT_PROFILE_CHANGING. */
	void (*profile_changing)(void *param);

	/* Takes the current profile out of the profile list, switches to
	 * another profile (OBSBasic::ActivateProfile, which emits
	 * OBS_FRONTEND_EVENT_PROFILE_CHANGED) and deletes the removed profile's
	 * files. */
	void (*switch_and_remove_current)(void *param);

	/* Deletes a profile that is not the current one and takes it out of
	 * the profile list and the Profile menu. */
	void (*remove_other)(void *param);

	/* Emits OBS_FRONTEND_EVENT_PROFILE_LIST_CHANGED. */
	void (*profile_list_changed)(void *param);
};

/* The user chose Profile -> Remove (OBSBasic::on_actionRemoveProfile_triggered)
 * for the current profile. ask: show the confirmation first. */
void profile_remove_current(const struct profile_remove_ops *ops, bool ask);

/* obs_frontend_delete_profile() (obs-websocket's RemoveProfile, scripts,
 * plugins) removes a profile by name (OBSBasic::DeleteProfile). is_current:
 * it is the profile in use, which is removed without asking. */
void profile_remove_by_api(const struct profile_remove_ops *ops, bool is_current);

#ifdef __cplusplus
}
#endif
