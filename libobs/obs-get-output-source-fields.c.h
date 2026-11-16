#include "libobs/build/libobs.pch.h"

/* Borrow the same main-canvas field as the original wrapper. The existing
 * getter owns all locking and returns the acquired source reference. */
struct obs_get_output_source_fields {
	obs_canvas_t *canvas;
	obs_source_t *(*get_channel)(obs_canvas_t *, uint32_t);
};

struct obs_get_output_source_fields obs_get_output_source_fields(void);
struct obs_get_output_source_fields obs_get_output_source_fields(void)
{
	return (struct obs_get_output_source_fields){obs->data.main_canvas, obs_canvas_get_channel};
}
