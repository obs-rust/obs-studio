/* libobs/graphics' Linux and BSD entry points (obs-rust/obs-studio#872). Included inside
 * graphics.c after its helpers; guarded by its own OS, no include guard. */

#if defined(__linux__) || defined(__FreeBSD__) || defined(__DragonFly__)
gs_texture_t *gs_texture_create_from_dmabuf(unsigned int width, unsigned int height, uint32_t drm_format,
					    enum gs_color_format color_format, uint32_t n_planes, const int *fds,
					    const uint32_t *strides, const uint32_t *offsets, const uint64_t *modifiers)
{
	graphics_t *graphics = thread_graphics;

	return graphics->exports.device_texture_create_from_dmabuf(
		graphics->device, width, height, drm_format, color_format, n_planes, fds, strides, offsets, modifiers);
}

bool gs_query_dmabuf_capabilities(enum gs_dmabuf_flags *dmabuf_flags, uint32_t **drm_formats, size_t *n_formats)
{
	graphics_t *graphics = thread_graphics;

	return graphics->exports.device_query_dmabuf_capabilities(graphics->device, dmabuf_flags, drm_formats,
								  n_formats);
}

bool gs_query_dmabuf_modifiers_for_format(uint32_t drm_format, uint64_t **modifiers, size_t *n_modifiers)
{
	graphics_t *graphics = thread_graphics;

	return graphics->exports.device_query_dmabuf_modifiers_for_format(graphics->device, drm_format, modifiers,
									  n_modifiers);
}

gs_texture_t *gs_texture_create_from_pixmap(uint32_t width, uint32_t height, enum gs_color_format color_format,
					    uint32_t target, void *pixmap)
{
	graphics_t *graphics = thread_graphics;

	return graphics->exports.device_texture_create_from_pixmap(graphics->device, width, height, color_format,
								   target, pixmap);
}

bool gs_query_sync_capabilities(void)
{
	graphics_t *graphics = thread_graphics;

	return graphics->exports.device_query_sync_capabilities(graphics->device);
}

gs_sync_t *gs_sync_create(void)
{
	graphics_t *graphics = thread_graphics;

	return graphics->exports.device_sync_create(graphics->device);
}

gs_sync_t *gs_sync_create_from_syncobj_timeline_point(int syncobj_fd, uint64_t timeline_point)
{
	graphics_t *graphics = thread_graphics;

	return graphics->exports.device_sync_create_from_syncobj_timeline_point(graphics->device, syncobj_fd,
										timeline_point);
}

void gs_sync_destroy(gs_sync_t *sync)
{
	graphics_t *graphics = thread_graphics;

	graphics->exports.device_sync_destroy(graphics->device, sync);
}

bool gs_sync_export_syncobj_timeline_point(gs_sync_t *sync, int syncobj_fd, uint64_t timeline_point)
{
	graphics_t *graphics = thread_graphics;

	return graphics->exports.device_sync_export_syncobj_timeline_point(graphics->device, sync, syncobj_fd,
									   timeline_point);
}

bool gs_sync_signal_syncobj_timeline_point(int syncobj_fd, uint64_t timeline_point)
{
	graphics_t *graphics = thread_graphics;

	return graphics->exports.device_sync_signal_syncobj_timeline_point(graphics->device, syncobj_fd,
									   timeline_point);
}

bool gs_sync_wait(gs_sync_t *sync)
{
	graphics_t *graphics = thread_graphics;

	return graphics->exports.device_sync_wait(graphics->device, sync);
}

#endif
