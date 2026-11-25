#include "libobs/build/libobs.pch.h"

/* obs-source.c.h's full-tree walk with a stop source is static there; this
 * extraction reaches it through its hidden wrapper (no EXPORT), under the
 * original name, scoped to this file. */
void obs_source_active_child_enum_full_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param,
					    obs_source_t *stop_source);
#define enum_source_full_tree obs_source_active_child_enum_full_tree

struct descendant_info {
	bool exists;
	obs_source_t *target;
};

static void check_descendant(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct descendant_info *info = param;
	if (child == info->target || parent == info->target)
		info->exists = true;
}

bool obs_source_add_active_child(obs_source_t *parent, obs_source_t *child)
{
	struct descendant_info info = {false, parent};

	if (!obs_ptr_valid(parent, "obs_source_add_active_child"))
		return false;
	if (!obs_ptr_valid(child, "obs_source_add_active_child"))
		return false;
	if (parent == child) {
		blog(LOG_WARNING, "obs_source_add_active_child: "
				  "parent == child");
		return false;
	}

	enum_source_full_tree(child, check_descendant, &info, parent);
	if (info.exists)
		return false;

	/* Other threads change the counts atomically (#2081). */
	long show_refs = os_atomic_load_long(&parent->show_refs);
	long activate_refs = os_atomic_load_long(&parent->activate_refs);
	for (long i = 0; i < show_refs; i++) {
		enum view_type type;
		type = (i < activate_refs) ? MAIN_VIEW : AUX_VIEW;
		obs_source_activate(child, type);
	}

	return true;
}

void obs_source_remove_active_child(obs_source_t *parent, obs_source_t *child)
{
	if (!obs_ptr_valid(parent, "obs_source_remove_active_child"))
		return;
	if (!obs_ptr_valid(child, "obs_source_remove_active_child"))
		return;

	/* Other threads change the counts atomically (#2081). */
	long show_refs = os_atomic_load_long(&parent->show_refs);
	long activate_refs = os_atomic_load_long(&parent->activate_refs);
	for (long i = 0; i < show_refs; i++) {
		enum view_type type;
		type = (i < activate_refs) ? MAIN_VIEW : AUX_VIEW;
		obs_source_deactivate(child, type);
	}
}

#undef enum_source_full_tree
