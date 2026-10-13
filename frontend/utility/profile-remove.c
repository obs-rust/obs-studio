#include "profile-remove.h"

void profile_remove_current(const struct profile_remove_ops *ops, bool ask)
{
	if (ask && !ops->confirm(ops->param)) {
		return;
	}

	ops->profile_changing(ops->param);
	ops->switch_and_remove_current(ops->param);

	/* The list lost a profile, as when any other profile is removed. */
	ops->profile_list_changed(ops->param);
}

void profile_remove_by_api(const struct profile_remove_ops *ops, bool is_current)
{
	if (is_current) {
		/* The caller asked for this profile to go. Asking someone at
		 * the OBS machine would leave a remote request waiting on a
		 * dialog nobody may see. */
		profile_remove_current(ops, false);
		return;
	}

	ops->remove_other(ops->param);
	ops->profile_list_changed(ops->param);
}
