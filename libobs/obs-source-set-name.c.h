#include "libobs/build/libobs.pch.h"

/* Keep the file-local canvas predicate available when compiled independently. */
static inline bool source_set_name_requires_canvas(const struct obs_source *source)
{
	return source->info.output_flags & OBS_SOURCE_REQUIRES_CANVAS;
}

void obs_source_set_name(obs_source_t *source, const char *name)
{
	if (!obs_source_valid(source, "obs_source_set_name"))
		return;

	if (!name || !*name || !source->context.name || strcmp(name, source->context.name) != 0) {
		/* A private scene knows its canvas but is not in its list. */
		if (source_set_name_requires_canvas(source) && !source->context.private) {
			obs_canvas_rename_source(source, name);
		} else {
			struct calldata data;
			char *prev_name = bstrdup(source->context.name);

			if (!source->context.private) {
				obs_context_data_setname_ht(&source->context, name, &obs->data.public_sources);
			} else {
				obs_context_data_setname(&source->context, name);
			}

			calldata_init(&data);
			calldata_set_ptr(&data, "source", source);
			calldata_set_string(&data, "new_name", source->context.name);
			calldata_set_string(&data, "prev_name", prev_name);
			if (!source->context.private)
				signal_handler_signal(obs->signals, "source_rename", &data);
			signal_handler_signal(source->context.signals, "rename", &data);
			calldata_free(&data);
			bfree(prev_name);
		}
	}
}
