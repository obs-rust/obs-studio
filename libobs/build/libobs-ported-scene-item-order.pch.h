/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-scene-item-order. obs-scene-item-order.c.h needs only its
 * declarations (obs-scene-item-order.h, with obs.h for obs_sceneitem_t,
 * struct obs_sceneitem_order_info and enum obs_order_movement), without the
 * core, so the C oracle and the C test can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include "libobs/obs-scene-item-order.h"
