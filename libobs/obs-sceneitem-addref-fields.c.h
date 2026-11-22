#include "libobs/build/libobs.pch.h"

/* C owns the reference counter's layout and the platform's atomic long
 * implementation; the Rust obs_sceneitem_addref() calls this once per item. */
void obs_sceneitem_addref_increment(obs_sceneitem_t *item);

void obs_sceneitem_addref_increment(obs_sceneitem_t *item)
{
	os_atomic_inc_long(&item->ref);
}
