#include "libobs/build/libobs.pch.h"

enum obs_scale_type obs_sceneitem_get_scale_filter(obs_sceneitem_t *item)
{
	return obs_ptr_valid(item, "obs_sceneitem_get_scale_filter") ? item->scale_filter : OBS_SCALE_DISABLE;
}

enum obs_blending_method obs_sceneitem_get_blending_method(obs_sceneitem_t *item)
{
	return obs_ptr_valid(item, "obs_sceneitem_get_blending_method") ? item->blend_method : OBS_BLEND_METHOD_DEFAULT;
}

enum obs_blending_type obs_sceneitem_get_blending_mode(obs_sceneitem_t *item)
{
	return obs_ptr_valid(item, "obs_sceneitem_get_blending_mode") ? item->blend_type : OBS_BLEND_NORMAL;
}
