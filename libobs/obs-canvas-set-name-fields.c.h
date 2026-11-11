#include "libobs/build/libobs.pch.h"

uint32_t obs_canvas_rename_flags(const obs_canvas_t *canvas);
const char *obs_canvas_rename_name(const obs_canvas_t *canvas);
bool obs_canvas_rename_private(const obs_canvas_t *canvas);
void obs_canvas_rename_set_private(obs_canvas_t *canvas, const char *name);
void obs_canvas_rename_set_public(obs_canvas_t *canvas, const char *name);
void obs_canvas_rename_local(obs_canvas_t *canvas, calldata_t *data);
void obs_canvas_rename_global(obs_canvas_t *canvas, calldata_t *data);

uint32_t obs_canvas_rename_flags(const obs_canvas_t *canvas)
{
	return canvas->flags;
}

const char *obs_canvas_rename_name(const obs_canvas_t *canvas)
{
	return canvas->context.name;
}

bool obs_canvas_rename_private(const obs_canvas_t *canvas)
{
	return canvas->context.private;
}

void obs_canvas_rename_set_private(obs_canvas_t *canvas, const char *name)
{
	obs_context_data_setname(&canvas->context, name);
}

void obs_canvas_rename_set_public(obs_canvas_t *canvas, const char *name)
{
	obs_context_data_setname_ht(&canvas->context, name, &obs->data.named_canvases);
}

void obs_canvas_rename_local(obs_canvas_t *canvas, calldata_t *data)
{
	signal_handler_signal(canvas->context.signals, "rename", data);
}

void obs_canvas_rename_global(obs_canvas_t *canvas, calldata_t *data)
{
	UNUSED_PARAMETER(canvas);
	signal_handler_signal(obs->signals, "canvas_rename", data);
}
