/* Compile the unchanged original callbacks with recording source operations. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef size_t obs_hotkey_pair_id;
typedef void obs_hotkey_t;
struct obs_source {
	bool muted;
	bool monitored;
	unsigned int reads;
	unsigned int mute_sets;
	unsigned int monitor_sets;
	unsigned int action_type;
	bool action_set;
};
static bool mute_monitor_oracle_muted(struct obs_source *source)
{
	source->reads++;
	return source->muted;
}
static bool mute_monitor_oracle_monitored(struct obs_source *source)
{
	source->reads++;
	return source->monitored;
}
static void mute_monitor_oracle_set_muted(struct obs_source *source, bool enabled)
{
	source->muted = enabled;
	source->mute_sets++;
	source->action_type = 1; /* AUDIO_ACTION_MUTE */
	source->action_set = enabled;
}
static void mute_monitor_oracle_set_monitored(struct obs_source *source, bool enabled)
{
	source->monitored = enabled;
	source->monitor_sets++;
}
#define UNUSED_PARAMETER(x) (void)(x)
#define obs_source_muted mute_monitor_oracle_muted
#define obs_source_get_monitoring_enabled mute_monitor_oracle_monitored
#define obs_source_set_muted mute_monitor_oracle_set_muted
#define obs_source_set_monitoring_enabled mute_monitor_oracle_set_monitored
#include "obs-source-mute-monitor-hotkeys.inc"
#undef obs_source_set_monitoring_enabled
#undef obs_source_set_muted
#undef obs_source_get_monitoring_enabled
#undef obs_source_muted
#undef UNUSED_PARAMETER

void oracle_source_mute_monitor_hotkey(bool muted, bool monitored, bool monitor, bool target, bool pressed,
				       uint32_t *out);
void oracle_source_mute_monitor_hotkey(bool muted, bool monitored, bool monitor, bool target, bool pressed,
				       uint32_t *out)
{
	struct obs_source source = {0};
	source.muted = muted;
	source.monitored = monitored;
	bool result;
	if (monitor)
		result = target ? obs_source_hotkey_monitor_on(&source, 0, NULL, pressed)
				: obs_source_hotkey_monitor_off(&source, 0, NULL, pressed);
	else
		result = target ? obs_source_hotkey_mute(&source, 0, NULL, pressed)
				: obs_source_hotkey_unmute(&source, 0, NULL, pressed);
	out[0] = result;
	out[1] = source.muted;
	out[2] = source.monitored;
	out[3] = source.reads;
	out[4] = source.mute_sets;
	out[5] = source.monitor_sets;
	out[6] = source.action_type;
	out[7] = source.action_set;
}
