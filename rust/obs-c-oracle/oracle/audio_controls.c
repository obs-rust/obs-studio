/* Test-only oracle: the original libobs/obs-audio-controls.c.h, unmodified,
 * with its exports renamed to oracle_* so it can link next to the Rust
 * implementation. bmalloc, bfree, blog, and calldata_get_data stay the real
 * symbols. The source calls are not linked into cargo tests, so this file
 * stubs them. libobs supplies them when this file is not linked. The signal
 * calls are stubbed under oracle_* names: obs-callback exports the real
 * signal_handler_* symbols, and a test that links both would define them
 * twice. */

#include <stddef.h>

#define obs_fader_create oracle_obs_fader_create
#define obs_fader_destroy oracle_obs_fader_destroy
#define obs_fader_set_db oracle_obs_fader_set_db
#define obs_fader_get_db oracle_obs_fader_get_db
#define obs_fader_set_deflection oracle_obs_fader_set_deflection
#define obs_fader_get_deflection oracle_obs_fader_get_deflection
#define obs_fader_set_mul oracle_obs_fader_set_mul
#define obs_fader_get_mul oracle_obs_fader_get_mul
#define obs_fader_attach_source oracle_obs_fader_attach_source
#define obs_fader_detach_source oracle_obs_fader_detach_source
#define obs_fader_add_callback oracle_obs_fader_add_callback
#define obs_fader_remove_callback oracle_obs_fader_remove_callback
#define obs_volmeter_create oracle_obs_volmeter_create
#define obs_volmeter_destroy oracle_obs_volmeter_destroy
#define obs_volmeter_attach_source oracle_obs_volmeter_attach_source
#define obs_volmeter_detach_source oracle_obs_volmeter_detach_source
#define obs_volmeter_set_peak_meter_type oracle_obs_volmeter_set_peak_meter_type
#define obs_volmeter_get_nr_channels oracle_obs_volmeter_get_nr_channels
#define obs_volmeter_add_callback oracle_obs_volmeter_add_callback
#define obs_volmeter_remove_callback oracle_obs_volmeter_remove_callback
#define obs_mul_to_db oracle_obs_mul_to_db
#define obs_db_to_mul oracle_obs_db_to_mul
#define obs_fader_db_to_def oracle_obs_fader_db_to_def
#define signal_handler_connect oracle_audio_controls_signal_handler_connect
#define signal_handler_disconnect oracle_audio_controls_signal_handler_disconnect

size_t oracle_obs_audio_info_size(void);
size_t oracle_obs_audio_info_align(void);
size_t oracle_obs_audio_info_speakers_offset(void);

/* libobs builds this file with -Wno-sign-conversion and without
 * -Wformat. The oracle has both. The unaligned-plane printf passes a
 * float * to %p; that is the C file, left as written. GCC does not
 * accept a -Wformat-pedantic diagnostic pragma. Clang does, and that
 * is the warning it reports for the same call. */
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wsign-conversion"
#pragma GCC diagnostic ignored "-Wformat"
#if defined(__clang__)
#pragma GCC diagnostic ignored "-Wformat-pedantic"
#endif
#endif

#include "obs-audio-controls.c.h"

size_t oracle_obs_audio_info_size(void)
{
	return sizeof(struct obs_audio_info);
}

size_t oracle_obs_audio_info_align(void)
{
	return _Alignof(struct obs_audio_info);
}

size_t oracle_obs_audio_info_speakers_offset(void)
{
	return offsetof(struct obs_audio_info, speakers);
}

signal_handler_t *obs_source_get_signal_handler(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return NULL;
}

float obs_source_get_volume(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return 0.0f;
}

void obs_source_set_volume(obs_source_t *source, float volume)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(volume);
}

bool obs_source_muted(const obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return false;
}

enum speaker_layout obs_source_get_speaker_layout(obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return SPEAKERS_UNKNOWN;
}

void obs_source_add_audio_capture_callback(obs_source_t *source, obs_source_audio_capture_t callback, void *param)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(callback);
	UNUSED_PARAMETER(param);
}

void obs_source_remove_audio_capture_callback(obs_source_t *source, obs_source_audio_capture_t callback, void *param)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(callback);
	UNUSED_PARAMETER(param);
}

bool obs_get_audio_info(struct obs_audio_info *oai)
{
	UNUSED_PARAMETER(oai);
	return false;
}

void signal_handler_connect(signal_handler_t *handler, const char *signal, signal_callback_t callback, void *data)
{
	UNUSED_PARAMETER(handler);
	UNUSED_PARAMETER(signal);
	UNUSED_PARAMETER(callback);
	UNUSED_PARAMETER(data);
}

void signal_handler_disconnect(signal_handler_t *handler, const char *signal, signal_callback_t callback, void *data)
{
	UNUSED_PARAMETER(handler);
	UNUSED_PARAMETER(signal);
	UNUSED_PARAMETER(callback);
	UNUSED_PARAMETER(data);
}
