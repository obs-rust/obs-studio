#include "libobs/build/libobs.pch.h"

void obs_output_set_reconnect_callback(obs_output_t *output,
				       bool (*reconnect_cb)(void *data, obs_output_t *output, int code), void *param)
{
	if (!reconnect_cb) {
		output->reconnect_callback.reconnect_cb = NULL;
		output->reconnect_callback.param = NULL;
	} else {
		output->reconnect_callback.reconnect_cb = reconnect_cb;
		output->reconnect_callback.param = param;
	}
}
