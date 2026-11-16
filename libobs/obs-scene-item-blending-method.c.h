#include "libobs/build/libobs.pch.h"

void obs_sceneitem_set_blending_method(obs_sceneitem_t *item, enum obs_blending_method method)
{
	if (!obs_ptr_valid(item, "obs_sceneitem_set_blending_method"))
		return;

	item->blend_method = method;
}
