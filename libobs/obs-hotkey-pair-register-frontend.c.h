#include "libobs/build/libobs.pch.h"

static inline void *obs_id_(void *id_)
{
	return id_;
}

obs_hotkey_pair_id obs_hotkey_pair_register_frontend(const char *name0, const char *description0, const char *name1,
						     const char *description1, obs_hotkey_active_func func0,
						     obs_hotkey_active_func func1, void *data0, void *data1)
{
	return register_hotkey_pair_internal(OBS_HOTKEY_REGISTERER_FRONTEND, NULL, obs_id_, NULL, name0, description0,
					     name1, description1, func0, func1, data0, data1);
}
