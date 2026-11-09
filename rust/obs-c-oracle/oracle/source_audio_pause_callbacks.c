/* Tier 3: original bodies, with calls to the unchanged signal subsystem
 * recorded rather than executed. Each fixture owns its trace (no globals). */
#include "libobs/build/libobs.pch.h"
#include <string.h>

struct audio_pause_trace {
	unsigned code;
	signal_callback_t callback;
	void *param;
};

static signal_handler_t *audio_pause_get_handler(const obs_source_t *source)
{
	struct audio_pause_trace *trace = (void *)source->context.signals;
	trace->code = trace->code * 16 + 1;
	return source->context.signals;
}

static void audio_pause_record(signal_handler_t *handler, const char *name, signal_callback_t callback, void *param,
			       bool adding)
{
	struct audio_pause_trace *trace = (void *)handler;
	unsigned event = strcmp(name, "media_pause") == 0 ? 2 : strcmp(name, "media_stopped") == 0 ? 3 : 15;
	if (!adding)
		event += 4;
	if (callback != trace->callback || param != trace->param)
		event = 15;
	trace->code = trace->code * 16 + event;
}

static void audio_pause_connect(signal_handler_t *handler, const char *name, signal_callback_t callback, void *param)
{
	audio_pause_record(handler, name, callback, param, true);
}

static void audio_pause_disconnect(signal_handler_t *handler, const char *name, signal_callback_t callback, void *param)
{
	audio_pause_record(handler, name, callback, param, false);
}

#define obs_source_get_signal_handler audio_pause_get_handler
#define signal_handler_connect audio_pause_connect
#define signal_handler_disconnect audio_pause_disconnect
#define obs_source_add_audio_pause_callback oracle_obs_source_add_audio_pause_callback
#define obs_source_remove_audio_pause_callback oracle_obs_source_remove_audio_pause_callback
void oracle_obs_source_add_audio_pause_callback(obs_source_t *source, signal_callback_t callback, void *param);
void oracle_obs_source_remove_audio_pause_callback(obs_source_t *source, signal_callback_t callback, void *param);
#include "obs-source-audio-pause-callbacks.c.h"

static void audio_pause_callback(void *param, calldata_t *data)
{
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(data);
}

unsigned oracle_audio_pause_trace(bool present, bool adding, bool null_callback, bool null_param);
unsigned oracle_audio_pause_trace(bool present, bool adding, bool null_callback, bool null_param)
{
	struct audio_pause_trace trace = {0};
	trace.callback = null_callback ? NULL : audio_pause_callback;
	trace.param = null_param ? NULL : &trace;
	struct obs_source source = {0};
	source.context.signals = (void *)&trace;
	obs_source_t *ptr = present ? &source : NULL;
	if (adding)
		oracle_obs_source_add_audio_pause_callback(ptr, trace.callback, trace.param);
	else
		oracle_obs_source_remove_audio_pause_callback(ptr, trace.callback, trace.param);
	return trace.code;
}
