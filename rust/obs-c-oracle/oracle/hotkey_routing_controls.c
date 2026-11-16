/* Original routing setters and the production private-field adapter. */
#include "libobs/build/libobs.pch.h"
#include <assert.h>
#include <string.h>

static _Thread_local struct obs_core routing_core;
static _Thread_local bool routing_present;
static _Thread_local size_t routing_depth;
static _Thread_local size_t routing_counts[2];

static struct obs_core *routing_read_core(void)
{
	assert(routing_depth > 0);
	return &routing_core;
}

static bool routing_lock(void)
{
	routing_counts[0]++;
	if (routing_present)
		routing_depth++;
	return routing_present;
}

static void routing_unlock(void)
{
	assert(routing_depth > 0);
	routing_counts[1]++;
	routing_depth--;
}

void oracle_obs_hotkey_set_callback_routing_func(obs_hotkey_callback_router_func func, void *data);
void oracle_obs_hotkey_enable_callback_rerouting(bool enable);
#define obs routing_read_core()
#define obs_hotkey_seam_lock routing_lock
#define obs_hotkey_seam_unlock routing_unlock
#define obs_hotkey_set_callback_routing_func oracle_obs_hotkey_set_callback_routing_func
#define obs_hotkey_enable_callback_rerouting oracle_obs_hotkey_enable_callback_rerouting
#include "obs-hotkey-routing-controls.c.h"
#undef obs_hotkey_enable_callback_rerouting
#undef obs_hotkey_set_callback_routing_func
#include "hotkey_routing_controls_fields.inc"
#undef obs_hotkey_seam_unlock
#undef obs_hotkey_seam_lock
#undef obs

void oracle_hotkey_routing_reset(bool present, obs_hotkey_callback_router_func callback, void *data, bool enabled);
void oracle_hotkey_routing_reset(bool present, obs_hotkey_callback_router_func callback, void *data, bool enabled)
{
	assert(routing_depth == 0);
	routing_present = present;
	routing_core.hotkeys.router_func = callback;
	routing_core.hotkeys.router_func_data = data;
	routing_core.hotkeys.reroute_hotkeys = enabled;
	memset(routing_counts, 0, sizeof(routing_counts));
}

void oracle_hotkey_routing_snapshot(obs_hotkey_callback_router_func *callback, void **data, bool *enabled,
				    size_t *counts);
void oracle_hotkey_routing_snapshot(obs_hotkey_callback_router_func *callback, void **data, bool *enabled,
				    size_t *counts)
{
	*callback = routing_core.hotkeys.router_func;
	*data = routing_core.hotkeys.router_func_data;
	*enabled = routing_core.hotkeys.reroute_hotkeys;
	counts[0] = routing_counts[0];
	counts[1] = routing_counts[1];
	counts[2] = routing_depth;
}
