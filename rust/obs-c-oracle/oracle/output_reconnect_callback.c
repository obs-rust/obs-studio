/* Exact original setter over the real private output and callback layout. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_output_set_reconnect_callback oracle_obs_output_set_reconnect_callback
#include "obs-output-reconnect-callback.c.h"
#undef obs_output_set_reconnect_callback
#include "output_reconnect_callback_fields.inc"

void oracle_output_reconnect_callback_layout(size_t *layout);
void oracle_output_reconnect_callback_layout(size_t *layout)
{
	layout[0] = sizeof(struct reconnect_callback);
	layout[1] = _Alignof(struct reconnect_callback);
	layout[2] = offsetof(struct reconnect_callback, reconnect_cb);
	layout[3] = offsetof(struct reconnect_callback, param);
}

struct output_reconnect_callback_fixture {
	struct obs_output output;
	struct obs_output before;
};
void *oracle_output_reconnect_callback_new(void);
void *oracle_output_reconnect_callback_new(void)
{
	struct output_reconnect_callback_fixture *fixture = calloc(1, sizeof(*fixture));
	if (fixture) {
		fixture->output.reconnect_retry_cur_msec = 29;
		memcpy(&fixture->before, &fixture->output, sizeof(fixture->before));
	}
	return fixture;
}
void oracle_output_reconnect_callback_free(void *output);
void oracle_output_reconnect_callback_free(void *output)
{
	free(output);
}
/* Compare callback identity and data against independently supplied inputs,
 * and verify every other output byte remains untouched. */
bool oracle_output_reconnect_callback_matches(void *output, bool (*callback)(void *, obs_output_t *, int), void *param);
bool oracle_output_reconnect_callback_matches(void *output, bool (*callback)(void *, obs_output_t *, int), void *param)
{
	struct output_reconnect_callback_fixture *fixture = output;
	struct obs_output expected;
	memcpy(&expected, &fixture->before, sizeof(expected));
	expected.reconnect_callback.reconnect_cb = callback;
	expected.reconnect_callback.param = param;
	return fixture->output.reconnect_callback.reconnect_cb == callback &&
	       fixture->output.reconnect_callback.param == param &&
	       memcmp(&fixture->output, &expected, sizeof(expected)) == 0;
}
bool oracle_output_reconnect_callback_invoke(void *output, void *argument, int code);
bool oracle_output_reconnect_callback_invoke(void *output, void *argument, int code)
{
	struct obs_output *value = output;
	return value->reconnect_callback.reconnect_cb(value->reconnect_callback.param, argument, code);
}
