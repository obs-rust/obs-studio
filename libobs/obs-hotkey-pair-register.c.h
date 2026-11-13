/******************************************************************************
    Copyright (C) 2014-2015 by Ruwen Hahn <palana@stunned.de>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "libobs/build/libobs.pch.h"

static inline void *weak_encoder_ref(void *ref)
{
	return obs_encoder_get_weak_encoder(ref);
}

obs_hotkey_pair_id obs_hotkey_pair_register_encoder(obs_encoder_t *encoder, const char *name0, const char *description0,
						    const char *name1, const char *description1,
						    obs_hotkey_active_func func0, obs_hotkey_active_func func1,
						    void *data0, void *data1)
{
	if (!encoder)
		return OBS_INVALID_HOTKEY_PAIR_ID;
	return register_hotkey_pair_internal(OBS_HOTKEY_REGISTERER_ENCODER, encoder, weak_encoder_ref,
					     &encoder->context, name0, description0, name1, description1, func0, func1,
					     data0, data1);
}

static inline void *weak_output_ref(void *ref)
{
	return obs_output_get_weak_output(ref);
}

obs_hotkey_pair_id obs_hotkey_pair_register_output(obs_output_t *output, const char *name0, const char *description0,
						   const char *name1, const char *description1,
						   obs_hotkey_active_func func0, obs_hotkey_active_func func1,
						   void *data0, void *data1)
{
	if (!output)
		return OBS_INVALID_HOTKEY_PAIR_ID;
	return register_hotkey_pair_internal(OBS_HOTKEY_REGISTERER_OUTPUT, output, weak_output_ref, &output->context,
					     name0, description0, name1, description1, func0, func1, data0, data1);
}

static inline void *weak_service_ref(void *ref)
{
	return obs_service_get_weak_service(ref);
}

obs_hotkey_pair_id obs_hotkey_pair_register_service(obs_service_t *service, const char *name0, const char *description0,
						    const char *name1, const char *description1,
						    obs_hotkey_active_func func0, obs_hotkey_active_func func1,
						    void *data0, void *data1)
{
	if (!service)
		return OBS_INVALID_HOTKEY_PAIR_ID;
	return register_hotkey_pair_internal(OBS_HOTKEY_REGISTERER_SERVICE, service, weak_service_ref,
					     &service->context, name0, description0, name1, description1, func0, func1,
					     data0, data1);
}

static inline void *weak_source_ref(void *ref)
{
	return obs_source_get_weak_source(ref);
}

obs_hotkey_pair_id obs_hotkey_pair_register_source(obs_source_t *source, const char *name0, const char *description0,
						   const char *name1, const char *description1,
						   obs_hotkey_active_func func0, obs_hotkey_active_func func1,
						   void *data0, void *data1)
{
	if (!source)
		return OBS_INVALID_HOTKEY_PAIR_ID;
	return register_hotkey_pair_internal(OBS_HOTKEY_REGISTERER_SOURCE, source, weak_source_ref, &source->context,
					     name0, description0, name1, description1, func0, func1, data0, data1);
}
