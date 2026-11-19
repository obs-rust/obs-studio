#include "libobs/build/libobs.pch.h"

/* Private field borrowing only; NULL and kind selection belong to Rust. */
int obs_object_id_kind(const void *obj);
const char *const *obs_object_id_source_id(const void *obj);
const char *const *obs_object_id_output_id(const void *obj);
const char *const *obs_object_id_encoder_id(const void *obj);
const char *const *obs_object_id_service_id(const void *obj);

int obs_object_id_kind(const void *obj)
{
	return (int)((const struct obs_context_data *)obj)->type;
}

const char *const *obs_object_id_source_id(const void *obj)
{
	return &((const obs_source_t *)obj)->info.id;
}

const char *const *obs_object_id_output_id(const void *obj)
{
	return &((const obs_output_t *)obj)->info.id;
}

const char *const *obs_object_id_encoder_id(const void *obj)
{
	return &((const obs_encoder_t *)obj)->info.id;
}

const char *const *obs_object_id_service_id(const void *obj)
{
	return &((const obs_service_t *)obj)->info.id;
}
