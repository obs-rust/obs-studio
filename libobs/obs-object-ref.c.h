#include "libobs/build/libobs.pch.h"

obs_object_t *obs_object_get_ref(obs_object_t *object)
{
	if (!object)
		return NULL;

	return obs_weak_object_get_object(object->control);
}

void obs_object_release(obs_object_t *object)
{
	if (!obs) {
		blog(LOG_WARNING, "Tried to release an object when the OBS "
				  "core is shut down!");
		return;
	}

	if (!object)
		return;

	obs_weak_object_t *control = object->control;
	if (obs_ref_release(&control->ref)) {
		object->destroy(object);
		obs_weak_object_release(control);
	}
}
