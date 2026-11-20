#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_encoder_add_output() and
 * obs_encoder_remove_output(). The encoder's output list, its darray buffer
 * (grown and freed only by the bmem darray helpers here and in
 * obs_encoder_destroy()) and outputs_mutex stay in C; Rust decides when each
 * is used. The list is read and changed only between the lock and unlock
 * adapters, as every other C reader (destroy, full stop, unstarted shutdown)
 * does under the same mutex. */
void obs_encoder_outputs_lock(struct obs_encoder *encoder);
void obs_encoder_outputs_lock(struct obs_encoder *encoder)
{
	pthread_mutex_lock(&encoder->outputs_mutex);
}

void obs_encoder_outputs_unlock(struct obs_encoder *encoder);
void obs_encoder_outputs_unlock(struct obs_encoder *encoder)
{
	pthread_mutex_unlock(&encoder->outputs_mutex);
}

void obs_encoder_outputs_push(struct obs_encoder *encoder, struct obs_output *output);
void obs_encoder_outputs_push(struct obs_encoder *encoder, struct obs_output *output)
{
	da_push_back(encoder->outputs, &output);
}

size_t obs_encoder_outputs_count(const struct obs_encoder *encoder);
size_t obs_encoder_outputs_count(const struct obs_encoder *encoder)
{
	return encoder->outputs.num;
}

struct obs_output *obs_encoder_outputs_at(const struct obs_encoder *encoder, size_t index);
struct obs_output *obs_encoder_outputs_at(const struct obs_encoder *encoder, size_t index)
{
	return encoder->outputs.array[index];
}

void obs_encoder_outputs_erase(struct obs_encoder *encoder, size_t index);
void obs_encoder_outputs_erase(struct obs_encoder *encoder, size_t index)
{
	da_erase(encoder->outputs, index);
}
