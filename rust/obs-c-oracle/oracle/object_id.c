/* Original C, real private layouts, and the exact production field adapters. */
#include "libobs/build/libobs.pch.h"
#include <stdlib.h>
#include <string.h>

const char *oracle_obs_obj_get_id(void *obj);
#define obs_obj_get_id oracle_obs_obj_get_id
#include "libobs/obs-object-id.c.h"
#undef obs_obj_get_id
#include "object_id_fields.inc"

typedef const char *(*object_id_query)(void *);
bool oracle_object_id_check(int kind, bool present, const char *id, const char *original_id, object_id_query query,
			    const char *safe_id);
bool oracle_object_id_check(int kind, bool present, const char *id, const char *original_id, object_id_query query,
			    const char *safe_id)
{
	if (OBS_OBJ_TYPE_INVALID != 0 || OBS_OBJ_TYPE_SOURCE != 1 || OBS_OBJ_TYPE_OUTPUT != 2 ||
	    OBS_OBJ_TYPE_ENCODER != 3 || OBS_OBJ_TYPE_SERVICE != 4 || OBS_OBJ_TYPE_CANVAS != 5)
		return false;
	if (!present)
		return !query(NULL) && !oracle_obs_obj_get_id(NULL) && !safe_id;

	size_t size = sizeof(struct obs_context_data);
	switch (kind) {
	case OBS_OBJ_TYPE_SOURCE:
		size = sizeof(obs_source_t);
		break;
	case OBS_OBJ_TYPE_OUTPUT:
		size = sizeof(obs_output_t);
		break;
	case OBS_OBJ_TYPE_ENCODER:
		size = sizeof(obs_encoder_t);
		break;
	case OBS_OBJ_TYPE_SERVICE:
		size = sizeof(obs_service_t);
		break;
	}
	void *obj = calloc(1, size), *before = malloc(size);
	if (!obj || !before) {
		free(obj);
		free(before);
		return false;
	}
	struct obs_context_data *context = obj;
	context->type = (enum obs_obj_type)kind;
	context->name = "different display name";
	const char *const *field = NULL, *expected = NULL;
	switch (kind) {
	case OBS_OBJ_TYPE_SOURCE:
		((obs_source_t *)obj)->info.id = id;
		field = &((obs_source_t *)obj)->info.id;
		break;
	case OBS_OBJ_TYPE_OUTPUT:
		((obs_output_t *)obj)->info.id = id;
		field = &((obs_output_t *)obj)->info.id;
		break;
	case OBS_OBJ_TYPE_ENCODER:
		((obs_encoder_t *)obj)->info.id = id;
		((obs_encoder_t *)obj)->orig_info.id = original_id;
		field = &((obs_encoder_t *)obj)->info.id;
		break;
	case OBS_OBJ_TYPE_SERVICE:
		((obs_service_t *)obj)->info.id = id;
		field = &((obs_service_t *)obj)->info.id;
		break;
	}
	memcpy(before, obj, size);
	bool valid = obs_object_id_kind(obj) == kind;
	switch (kind) {
	case OBS_OBJ_TYPE_SOURCE:
		valid = valid && obs_object_id_source_id(obj) == field;
		break;
	case OBS_OBJ_TYPE_OUTPUT:
		valid = valid && obs_object_id_output_id(obj) == field;
		break;
	case OBS_OBJ_TYPE_ENCODER:
		valid = valid && obs_object_id_encoder_id(obj) == field;
		break;
	case OBS_OBJ_TYPE_SERVICE:
		valid = valid && obs_object_id_service_id(obj) == field;
		break;
	}
	if (field)
		expected = id;
	for (int repeat = 0; repeat < 2; ++repeat)
		valid = valid && query(obj) == expected && oracle_obs_obj_get_id(obj) == expected &&
			safe_id == expected && memcmp(before, obj, size) == 0;
	free(before);
	free(obj);
	return valid;
}
