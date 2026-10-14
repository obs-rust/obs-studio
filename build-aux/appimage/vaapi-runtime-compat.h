/* Runtime VA buffer mapping used by the AppImage FFmpeg build. */
#pragma once

#include <va/va.h>

static inline VAStatus obs_va_map_buffer(VADisplay display, VABufferID buffer, void **address, unsigned int flags)
{
	return vaMapBuffer2(display, buffer, address, flags);
}
