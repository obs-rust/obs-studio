#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_enum_outputs(), obs_enum_encoders() and
 * obs_enum_services(): each kind's list mutex, list head, an entry's next
 * link and the kind's get_ref/release. The lists, their layout and the
 * reference counting stay in C; Rust decides which reads and calls are made,
 * and in which order. kind: 0 outputs, 1 encoders, 2 services. */
static pthread_mutex_t *obs_enum_contexts_mutex(uint32_t kind)
{
	switch (kind) {
	case 0:
		return &obs->data.outputs_mutex;
	case 1:
		return &obs->data.encoders_mutex;
	default:
		return &obs->data.services_mutex;
	}
}

void obs_enum_contexts_lock(uint32_t kind);
void obs_enum_contexts_lock(uint32_t kind)
{
	pthread_mutex_lock(obs_enum_contexts_mutex(kind));
}

void obs_enum_contexts_unlock(uint32_t kind);
void obs_enum_contexts_unlock(uint32_t kind)
{
	pthread_mutex_unlock(obs_enum_contexts_mutex(kind));
}

struct obs_context_data *obs_enum_contexts_first(uint32_t kind);
struct obs_context_data *obs_enum_contexts_first(uint32_t kind)
{
	switch (kind) {
	case 0:
		return (struct obs_context_data *)obs->data.first_output;
	case 1:
		return (struct obs_context_data *)obs->data.first_encoder;
	default:
		return (struct obs_context_data *)obs->data.first_service;
	}
}

struct obs_context_data *obs_enum_contexts_next(const struct obs_context_data *context);
struct obs_context_data *obs_enum_contexts_next(const struct obs_context_data *context)
{
	return context->next;
}

struct obs_context_data *obs_enum_contexts_get_ref(uint32_t kind, struct obs_context_data *context);
struct obs_context_data *obs_enum_contexts_get_ref(uint32_t kind, struct obs_context_data *context)
{
	switch (kind) {
	case 0:
		return (struct obs_context_data *)obs_output_get_ref((obs_output_t *)context);
	case 1:
		return (struct obs_context_data *)obs_encoder_get_ref((obs_encoder_t *)context);
	default:
		return (struct obs_context_data *)obs_service_get_ref((obs_service_t *)context);
	}
}

void obs_enum_contexts_release(uint32_t kind, struct obs_context_data *context);
void obs_enum_contexts_release(uint32_t kind, struct obs_context_data *context)
{
	switch (kind) {
	case 0:
		obs_output_release((obs_output_t *)context);
		break;
	case 1:
		obs_encoder_release((obs_encoder_t *)context);
		break;
	default:
		obs_service_release((obs_service_t *)context);
		break;
	}
}
