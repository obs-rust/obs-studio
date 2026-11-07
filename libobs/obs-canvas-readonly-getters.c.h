#include "libobs/build/libobs-ported-service-getters.pch.h"

const char *obs_canvas_get_name(const obs_canvas_t *canvas)
{
	return canvas->context.name;
}

const char *obs_canvas_get_uuid(const obs_canvas_t *canvas)
{
	return canvas->context.uuid;
}

signal_handler_t *obs_canvas_get_signal_handler(obs_canvas_t *canvas)
{
	return canvas->context.signals;
}

bool obs_canvas_has_video(obs_canvas_t *canvas)
{
	return canvas->mix != NULL;
}
