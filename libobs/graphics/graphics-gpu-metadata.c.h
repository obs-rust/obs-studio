#include "libobs/build/libobs-graphics.pch.h"

/* Private dependencies defined in graphics.c.h, later in the unity group. */
static THREAD_LOCAL graphics_t *thread_graphics;
static inline bool gs_valid(const char *f);

const char *gs_get_driver_version(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_driver_version"))
		return NULL;

	if (graphics->exports.gpu_get_driver_version)
		return (graphics->exports.gpu_get_driver_version());
	else
		return NULL;
}

const char *gs_get_renderer(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_renderer"))
		return NULL;

	if (graphics->exports.gpu_get_renderer)
		return (graphics->exports.gpu_get_renderer());
	else
		return NULL;
}

uint64_t gs_get_gpu_dmem(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_gpu_dmem"))
		return 0;

	if (graphics->exports.gpu_get_dmem)
		return (graphics->exports.gpu_get_dmem());
	else
		return 0;
}

uint64_t gs_get_gpu_smem(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_get_gpu_smem"))
		return 0;

	if (graphics->exports.gpu_get_smem)
		return (graphics->exports.gpu_get_smem());
	else
		return 0;
}
