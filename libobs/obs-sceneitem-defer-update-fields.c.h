#include "libobs/build/libobs.pch.h"

/* C owns the counter layout and the platform's atomic long implementation.
 * Return the RMW result: a separate load could observe another caller. */
long obs_sceneitem_defer_update_increment(obs_sceneitem_t *item);
long obs_sceneitem_defer_update_decrement(obs_sceneitem_t *item);

long obs_sceneitem_defer_update_increment(obs_sceneitem_t *item)
{
	return os_atomic_inc_long(&item->defer_update);
}

long obs_sceneitem_defer_update_decrement(obs_sceneitem_t *item)
{
	return os_atomic_dec_long(&item->defer_update);
}
