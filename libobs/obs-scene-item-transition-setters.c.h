#include "libobs/build/libobs.pch.h"

void obs_sceneitem_set_transition(obs_sceneitem_t *item, bool show, obs_source_t *transition)
{
	if (!item)
		return;

	obs_source_t **target = show ? &item->show_transition : &item->hide_transition;
	if (*target)
		obs_source_release(*target);
	*target = obs_source_get_ref(transition);
}

void obs_sceneitem_set_transition_duration(obs_sceneitem_t *item, bool show, uint32_t duration_ms)
{
	if (!item)
		return;
	if (show)
		item->show_transition_duration = duration_ms;
	else
		item->hide_transition_duration = duration_ms;
}
