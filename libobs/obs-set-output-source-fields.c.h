#include "libobs/build/libobs.pch.h"

/* The canvas setter still owns all locking, signals and source references.
 * Borrow the same field as the original wrapper, never obs_get_main_canvas(). */
struct obs_set_output_source_fields {
	obs_canvas_t *canvas;
	void (*set_channel)(obs_canvas_t *, uint32_t, obs_source_t *);
};

struct obs_set_output_source_fields obs_set_output_source_fields(void);
struct obs_set_output_source_fields obs_set_output_source_fields(void)
{
	return (struct obs_set_output_source_fields){obs->data.main_canvas, obs_canvas_set_channel};
}
