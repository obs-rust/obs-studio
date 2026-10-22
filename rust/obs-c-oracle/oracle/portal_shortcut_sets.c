/* Test-only oracle: the original libobs/obs-nix-portal-shortcut-sets.c.h,
 * unmodified, with its entry points renamed to oracle_* so they can link
 * next to the Rust shims. It calls nothing outside the file but strcmp and
 * the header's static inline same_combo(). */
#include <stddef.h>
#include <stdint.h>

#define obs_nix_portal_sets_has oracle_obs_nix_portal_sets_has
#define obs_nix_portal_sets_unchanged oracle_obs_nix_portal_sets_unchanged
#define obs_nix_portal_sets_request oracle_obs_nix_portal_sets_request
#define obs_nix_portal_sets_partition oracle_obs_nix_portal_sets_partition

#include "obs-nix-portal-shortcut-sets.c.h"

/* The layouts the entry points take, in the order of
 * rust/obs-c-oracle/src/portal_shortcut_sets.rs. */
enum {
	ORACLE_PORTAL_SETS_LAYOUT_COUNT = 10,
};

size_t oracle_portal_sets_layout(size_t which);

size_t oracle_portal_sets_layout(size_t which)
{
	const size_t values[ORACLE_PORTAL_SETS_LAYOUT_COUNT] = {
		sizeof(struct obs_nix_portal_combos),
		_Alignof(struct obs_nix_portal_combos),
		offsetof(struct obs_nix_portal_combos, array),
		offsetof(struct obs_nix_portal_combos, num),
		sizeof(struct obs_nix_portal_descriptions),
		_Alignof(struct obs_nix_portal_descriptions),
		offsetof(struct obs_nix_portal_descriptions, array),
		offsetof(struct obs_nix_portal_descriptions, num),
		sizeof(size_t),
		sizeof(bool),
	};
	return which < ORACLE_PORTAL_SETS_LAYOUT_COUNT ? values[which] : 0;
}
