#include "libobs/build/libobs.pch.h"

obs_source_t *obs_sceneitem_get_transition(obs_sceneitem_t *item, bool show)
{
	if (!item)
		return NULL;

	return show ? item->show_transition : item->hide_transition;
}

uint32_t obs_sceneitem_get_transition_duration(obs_sceneitem_t *item, bool show)
{
	if (!item)
		return 0;
	return show ? item->show_transition_duration : item->hide_transition_duration;
}
