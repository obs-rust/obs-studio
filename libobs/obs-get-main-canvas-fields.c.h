#include "libobs/build/libobs.pch.h"

/* Borrow the current field; the existing getter owns reference acquisition. */
struct obs_get_main_canvas_fields {
	obs_canvas_t *canvas;
	obs_canvas_t *(*get_ref)(obs_canvas_t *);
};

struct obs_get_main_canvas_fields obs_get_main_canvas_fields(void);
struct obs_get_main_canvas_fields obs_get_main_canvas_fields(void)
{
	return (struct obs_get_main_canvas_fields){obs->data.main_canvas, obs_canvas_get_ref};
}
