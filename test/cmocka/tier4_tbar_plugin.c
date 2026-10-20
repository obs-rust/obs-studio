/* Test plugin for Tier 4's "tbar" phase (run_tier4.py --only tbar): calls the
 * frontend's T-Bar API the way a plugin does, on request over obs-websocket
 * (vendor "tier4-tbar"):
 * - "get_position": obs_frontend_get_tbar_position(), returned as
 *   {"position": N};
 * - "release": obs_frontend_release_tbar().
 * Not installed; the phase copies it into a fresh config's plugin folder. */

#include <obs-module.h>
#include <obs-frontend-api.h>

#include "obs-websocket-api.h"

OBS_DECLARE_MODULE()

static obs_websocket_vendor vendor;

static void get_position(obs_data_t *request, obs_data_t *response, void *param)
{
	(void)request;
	(void)param;
	obs_data_set_int(response, "position", obs_frontend_get_tbar_position());
}

static void release(obs_data_t *request, obs_data_t *response, void *param)
{
	(void)request;
	(void)param;
	obs_frontend_release_tbar();
	obs_data_set_bool(response, "released", true);
}

bool obs_module_load(void)
{
	return true;
}

void obs_module_post_load(void)
{
	vendor = obs_websocket_register_vendor("tier4-tbar");
	if (!vendor) {
		blog(LOG_WARNING, "[tier4-tbar] obs-websocket is not loaded");
		return;
	}
	obs_websocket_vendor_register_request(vendor, "get_position", get_position, NULL);
	obs_websocket_vendor_register_request(vendor, "release", release, NULL);
	blog(LOG_INFO, "[tier4-tbar] vendor requests registered");
}
