#include "libobs/build/libobs.pch.h"

/* Borrow the same canvas field as the original wrapper. Canvas enumeration
 * continues to own traversal, callback stopping, locking and source references. */
struct obs_enum_scenes_fields {
	obs_canvas_t *canvas;
	void (*enumerate)(obs_canvas_t *, bool (*)(void *, obs_source_t *), void *);
};

struct obs_enum_scenes_fields obs_enum_scenes_fields(void);
struct obs_enum_scenes_fields obs_enum_scenes_fields(void)
{
	return (struct obs_enum_scenes_fields){obs->data.main_canvas, obs_canvas_enum_scenes};
}
