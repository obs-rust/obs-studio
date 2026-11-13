#include "libobs/build/libobs-graphics.pch.h"

/* Preserve graphics.c.h's context/validation seams in this standalone group. */
#define thread_graphics gs_get_context()
static inline bool vertexbuffer_create_valid(const char *function)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
		return false;
	}
	return true;
}
#define gs_valid vertexbuffer_create_valid

gs_vertbuffer_t *gs_vertexbuffer_create(struct gs_vb_data *data, uint32_t flags)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_vertexbuffer_create"))
		return NULL;

	if (data && data->num && (flags & GS_DUP_BUFFER) != 0) {
		struct gs_vb_data *new_data = gs_vbdata_create();

		new_data->num = data->num;

#define DUP_VAL(val)                                                                        \
	do {                                                                                \
		if (data->val)                                                              \
			new_data->val = bmemdup(data->val, sizeof(*data->val) * data->num); \
	} while (false)

		DUP_VAL(points);
		DUP_VAL(normals);
		DUP_VAL(tangents);
		DUP_VAL(colors);
#undef DUP_VAL

		if (data->tvarray && data->num_tex) {
			new_data->num_tex = data->num_tex;
			new_data->tvarray = bzalloc(sizeof(struct gs_tvertarray) * data->num_tex);

			for (size_t i = 0; i < data->num_tex; i++) {
				struct gs_tvertarray *tv = &data->tvarray[i];
				struct gs_tvertarray *new_tv = &new_data->tvarray[i];
				size_t size = tv->width * sizeof(float);

				new_tv->width = tv->width;
				new_tv->array = bmemdup(tv->array, size * data->num);
			}
		}

		data = new_data;
	}

	return graphics->exports.device_vertexbuffer_create(graphics->device, data, flags);
}

#undef gs_valid
#undef thread_graphics
