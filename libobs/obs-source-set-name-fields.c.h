#include "libobs/build/libobs.pch.h"

/* Live adapters for the Rust source rename. The caller rejects NULL and keeps
 * the source alive; each call is one read or one call of the original setter. */
bool obs_source_set_name_requires_canvas(const obs_source_t *source);
const char *obs_source_set_name_current(const obs_source_t *source);
bool obs_source_set_name_private(const obs_source_t *source);
void obs_source_set_name_canvas(obs_source_t *source, const char *name);
void obs_source_set_name_public(obs_source_t *source, const char *name);
void obs_source_set_name_unlisted(obs_source_t *source, const char *name);
void obs_source_set_name_global(obs_source_t *source, calldata_t *data);
void obs_source_set_name_local(obs_source_t *source, calldata_t *data);

bool obs_source_set_name_requires_canvas(const obs_source_t *source)
{
	return source->info.output_flags & OBS_SOURCE_REQUIRES_CANVAS;
}

const char *obs_source_set_name_current(const obs_source_t *source)
{
	return source->context.name;
}

bool obs_source_set_name_private(const obs_source_t *source)
{
	return source->context.private;
}

void obs_source_set_name_canvas(obs_source_t *source, const char *name)
{
	obs_canvas_rename_source(source, name);
}

void obs_source_set_name_public(obs_source_t *source, const char *name)
{
	obs_context_data_setname_ht(&source->context, name, &obs->data.public_sources);
}

void obs_source_set_name_unlisted(obs_source_t *source, const char *name)
{
	obs_context_data_setname(&source->context, name);
}

void obs_source_set_name_global(obs_source_t *source, calldata_t *data)
{
	UNUSED_PARAMETER(source);
	signal_handler_signal(obs->signals, "source_rename", data);
}

void obs_source_set_name_local(obs_source_t *source, calldata_t *data)
{
	signal_handler_signal(source->context.signals, "rename", data);
}
