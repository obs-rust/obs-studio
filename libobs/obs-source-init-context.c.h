#include "libobs/build/libobs.pch.h"

static const char *source_signals[] = {
	"void destroy(ptr source)",
	"void remove(ptr source)",
	"void update(ptr source)",
	"void save(ptr source)",
	"void load(ptr source)",
	"void activate(ptr source)",
	"void deactivate(ptr source)",
	"void show(ptr source)",
	"void hide(ptr source)",
	"void mute(ptr source, bool muted)",
	"void push_to_mute_changed(ptr source, bool enabled)",
	"void push_to_mute_delay(ptr source, int delay)",
	"void push_to_talk_changed(ptr source, bool enabled)",
	"void push_to_talk_delay(ptr source, int delay)",
	"void enable(ptr source, bool enabled)",
	"void rename(ptr source, string new_name, string prev_name)",
	"void volume(ptr source, in out float volume)",
	"void update_properties(ptr source)",
	"void update_flags(ptr source, int flags)",
	"void audio_sync(ptr source, int out int offset)",
	"void audio_balance(ptr source, in out float balance)",
	"void audio_mixers(ptr source, in out int mixers)",
	"void audio_monitoring(ptr source, int type)",
	"void monitor(ptr source, bool monitor)",
	"void audio_activate(ptr source)",
	"void audio_deactivate(ptr source)",
	"void filter_add(ptr source, ptr filter)",
	"void filter_remove(ptr source, ptr filter)",
	"void reorder_filters(ptr source)",
	"void transition_start(ptr source)",
	"void transition_video_stop(ptr source)",
	"void transition_stop(ptr source)",
	"void media_play(ptr source)",
	"void media_pause(ptr source)",
	"void media_restart(ptr source)",
	"void media_stopped(ptr source)",
	"void media_next(ptr source)",
	"void media_previous(ptr source)",
	"void media_started(ptr source)",
	"void media_ended(ptr source)",
	NULL,
};

bool obs_source_init_context(struct obs_source *source, obs_data_t *settings, const char *name, const char *uuid,
			     obs_data_t *hotkey_data, bool private)
{
	if (!obs_context_data_init(&source->context, OBS_OBJ_TYPE_SOURCE, settings, name, uuid, hotkey_data, private))
		return false;

	return signal_handler_add_array(source->context.signals, source_signals);
}
