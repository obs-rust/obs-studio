/* Test-only oracle: the original libobs/obs-nix-portal-shortcut-status.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims. It calls nothing outside the file. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define obs_nix_portal_status_action oracle_obs_nix_portal_status_action
#define obs_nix_portal_status_after_bind oracle_obs_nix_portal_status_after_bind
#define obs_nix_portal_status_bound oracle_obs_nix_portal_status_bound
#define obs_nix_portal_status_can_configure oracle_obs_nix_portal_status_can_configure
#define obs_nix_portal_status_declined oracle_obs_nix_portal_status_declined
#define obs_nix_portal_status_desktop_triggers oracle_obs_nix_portal_status_desktop_triggers
#define obs_nix_portal_status_has_description oracle_obs_nix_portal_status_has_description
#define obs_nix_portal_status_keep_approved oracle_obs_nix_portal_status_keep_approved
#define obs_nix_portal_status_more oracle_obs_nix_portal_status_more
#define obs_nix_portal_status_rebind oracle_obs_nix_portal_status_rebind

#include "obs-nix-portal-shortcut-status.c.h"

/* The values and layouts the C uses, in the order of
 * rust/obs-c-oracle/src/portal_shortcut_status.rs. */
enum {
	ORACLE_PORTAL_STATUS_VALUE_COUNT = 16,
};

int64_t oracle_portal_status_value(size_t which);

int64_t oracle_portal_status_value(size_t which)
{
	const int64_t values[ORACLE_PORTAL_STATUS_VALUE_COUNT] = {
		OBS_NIX_PORTAL_ACTION_FIRST,
		OBS_NIX_PORTAL_ACTION_NEXT,
		OBS_NIX_PORTAL_ACTION_UNNAMED,
		OBS_NIX_PORTAL_REBIND_NONE,
		OBS_NIX_PORTAL_REBIND_KEYS_DECLINED,
		OBS_NIX_PORTAL_REBIND_DESCRIPTION_DECLINED,
		OBS_NIX_PORTAL_CHANGE_KEEP,
		OBS_NIX_PORTAL_CHANGE_SET,
		OBS_NIX_PORTAL_CHANGE_CLEAR,
		sizeof(enum obs_nix_portal_action),
		sizeof(struct obs_nix_portal_outcome),
		_Alignof(struct obs_nix_portal_outcome),
		offsetof(struct obs_nix_portal_outcome, mark_declined),
		offsetof(struct obs_nix_portal_outcome, clear_declined),
		offsetof(struct obs_nix_portal_outcome, set_partly),
		offsetof(struct obs_nix_portal_outcome, partly),
	};
	return which < ORACLE_PORTAL_STATUS_VALUE_COUNT ? values[which] : -1;
}
