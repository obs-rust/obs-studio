#include "libobs/build/libobs-graphics-ffmpeg-host-wic-checks.pch.h"

bool gs_image_host_fallback_refuses(const char *file, size_t len)
{
	return len <= 4 && astrcmpi(file + len - 4, ".jxr") != 0;
}

enum gs_image_wic_kind gs_image_wic_kind(bool scrgb, bool pq10)
{
	if (scrgb || pq10)
		return pq10 ? GS_IMAGE_WIC_PQ10 : GS_IMAGE_WIC_COPY;

	return GS_IMAGE_WIC_UNSUPPORTED;
}
