#include "libobs/build/libobs.pch.h"

/* obs_source_enum_full_tree() and the hidden stop-source wrapper
 * obs_source_active_child_enum_full_tree(), with their static helpers, moved
 * verbatim out of obs-source.c.h (#5833). obs-source.c.h's data_valid() and
 * struct source_enum_data are file-local there, so this file carries its own
 * copies under scoped names; the bodies keep the original names and the file
 * also builds in a translation unit of its own. */
struct source_enum_full_tree_data {
	obs_source_enum_proc_t enum_callback;
	void *param;
	obs_source_t *stop_source;
};

static inline bool source_enum_full_tree_data_valid(const struct obs_source *source, const char *f)
{
	return obs_source_valid(source, f) && source->context.data;
}

#define source_enum_data source_enum_full_tree_data
#define data_valid source_enum_full_tree_data_valid

static void enum_source_full_tree_callback(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct source_enum_data *data = param;
	/* Child admission can hold stop_source's nonrecursive transition mutex.
	 * Report that edge before descending, even if a plugin changed the tree. */
	if (child == data->stop_source) {
		data->enum_callback(parent, child, data->param);
		return;
	}
	bool is_transition = child->info.type == OBS_SOURCE_TYPE_TRANSITION;

	if (is_transition)
		obs_transition_enum_sources(child, enum_source_full_tree_callback, param);
	if (child->info.enum_all_sources) {
		if (child->context.data) {
			child->info.enum_all_sources(child->context.data, enum_source_full_tree_callback, data);
		}
	} else if (child->info.enum_active_sources) {
		if (child->context.data) {
			child->info.enum_active_sources(child->context.data, enum_source_full_tree_callback, data);
		}
	}

	data->enum_callback(parent, child, data->param);
}

static void enum_source_full_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param,
				  obs_source_t *stop_source)
{
	struct source_enum_data data = {enum_callback, param, stop_source};
	bool is_transition;

	if (!data_valid(source, "obs_source_enum_full_tree"))
		return;

	/* Descend through either callback, as enum_source_full_tree_callback
	 * does for a child (#2942). */
	is_transition = source->info.type == OBS_SOURCE_TYPE_TRANSITION;
	if (!is_transition && !source->info.enum_all_sources && !source->info.enum_active_sources)
		return;

	source = obs_source_get_ref(source);
	if (!data_valid(source, "obs_source_enum_full_tree"))
		return;

	if (source->info.type == OBS_SOURCE_TYPE_TRANSITION)
		obs_transition_enum_sources(source, enum_source_full_tree_callback, &data);

	if (source->info.enum_all_sources) {
		source->info.enum_all_sources(source->context.data, enum_source_full_tree_callback, &data);

	} else if (source->info.enum_active_sources) {
		source->info.enum_active_sources(source->context.data, enum_source_full_tree_callback, &data);
	}

	obs_source_release(source);
}

void obs_source_enum_full_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param)
{
	enum_source_full_tree(source, enum_callback, param, NULL);
}

/* The walk with a stop source, for obs_source_add_active_child()
 * (obs-source-active-child.c.h). Hidden: no EXPORT. */
void obs_source_active_child_enum_full_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param,
					    obs_source_t *stop_source);
void obs_source_active_child_enum_full_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param,
					    obs_source_t *stop_source)
{
	enum_source_full_tree(source, enum_callback, param, stop_source);
}

#undef data_valid
#undef source_enum_data
