/* Standalone Rust-static fixtures pull the real image session adapter through
 * unrelated archive members, but never initialize graphics or load an image.
 * This private callback is not exported by libobs. Abort if that contract changes;
 * image-loading characterizations provide their own recording callback instead. */
#include <stdint.h>
#include <stdlib.h>

uint32_t gs_image_max_texture_size(void);

uint32_t gs_image_max_texture_size(void)
{
	abort();
}
