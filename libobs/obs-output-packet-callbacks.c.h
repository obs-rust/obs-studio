#include "libobs/build/libobs.pch.h"

void obs_output_add_packet_callback(obs_output_t *output,
				    void (*packet_cb)(obs_output_t *output, struct encoder_packet *pkt,
						      struct encoder_packet_time *pkt_time, void *param),
				    void *param)
{
	struct packet_callback data = {packet_cb, param};

	pthread_mutex_lock(&output->pkt_callbacks_mutex);
	da_insert(output->pkt_callbacks, 0, &data);
	pthread_mutex_unlock(&output->pkt_callbacks_mutex);
}

void obs_output_remove_packet_callback(obs_output_t *output,
				       void (*packet_cb)(obs_output_t *output, struct encoder_packet *pkt,
							 struct encoder_packet_time *pkt_time, void *param),
				       void *param)
{
	struct packet_callback data = {packet_cb, param};

	pthread_mutex_lock(&output->pkt_callbacks_mutex);
	da_erase_item(output->pkt_callbacks, &data);
	pthread_mutex_unlock(&output->pkt_callbacks_mutex);
}
