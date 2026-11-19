#include "libobs/build/libobs-graphics.pch.h"

/* Shared by the real graphics lifecycle and CPU image loading. Keep the
 * atomic state with its callback so standalone consumers need no runtime. */
static volatile long image_max_texture_size;

void gs_image_set_max_texture_size(uint32_t size);

uint32_t gs_image_max_texture_size(void)
{
	return (uint32_t)os_atomic_load_long(&image_max_texture_size);
}

void gs_image_set_max_texture_size(uint32_t size)
{
	os_atomic_set_long(&image_max_texture_size, (long)size);
}
