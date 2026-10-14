/* Runtime VA buffer mapping used by the AppImage FFmpeg build. */
#pragma once

#include <dlfcn.h>
#include <va/va.h>

static inline VAStatus obs_va_map_buffer(VADisplay display, VABufferID buffer, void **address, unsigned int flags)
{
	/* Keep the newer mapping flags when the host implements this API. Older
	 * libva versions use vaMapBuffer, as libva itself does for older drivers.
	 * An optional lookup avoids a loader requirement for vaMapBuffer2. */
	typedef VAStatus (*map_buffer2_fn)(VADisplay, VABufferID, void **, unsigned int);
	map_buffer2_fn map_buffer2 = (map_buffer2_fn)dlsym(RTLD_DEFAULT, "vaMapBuffer2");
	if (map_buffer2) {
		return map_buffer2(display, buffer, address, flags);
	}
	return vaMapBuffer(display, buffer, address);
}
