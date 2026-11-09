#include "libobs/build/libobs.pch.h"

void obs_source_set_async_rotation(obs_source_t *source, long rotation)
{
	/* In 0-359, so rotate_async_video() moves every multiple of 90 (-180,
	 * 450, ...) back into the item's box as its equivalent (#2121). The
	 * remainder is in (-360, 360), so adding 360 cannot overflow, even for
	 * LONG_MIN. A rotation that is not a multiple of 90 keeps its axis
	 * swap, as 360 is a multiple of 180. */
	if (source) {
		rotation %= 360;
		source->async_rotation = rotation < 0 ? rotation + 360 : rotation;
	}
}
