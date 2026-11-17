/* Original extracted control and the real field adapter over the real layout. */
#include "libobs/build/libobs.pch.h"
#include <assert.h>
#include <stddef.h>
#include <string.h>

static _Thread_local struct obs_core control_core;
static _Thread_local bool control_present;
static _Thread_local bool control_locked;
static _Thread_local size_t control_locks, control_unlocks, control_wakes;
static _Thread_local size_t control_callback_state[4];

static struct obs_core *control_read_core(void)
{
	assert(control_locked);
	return &control_core;
}

static bool control_lock(void)
{
	assert(!control_locked);
	control_locks++;
	control_locked = control_present;
	return control_locked;
}

static void control_unlock(void)
{
	assert(control_locked);
	control_unlocks++;
	control_locked = false;
}

static void control_wake(void *data)
{
	assert(control_locked);
	assert(data == &control_core);
	control_wakes++;
	control_callback_state[0] = control_core.hotkeys.background.retry;
	control_callback_state[1] = control_core.hotkeys.background.configure;
	control_callback_state[2] = control_core.hotkeys.background.probe;
	control_callback_state[3] = control_core.hotkeys.background.flags;
}

void oracle_obs_hotkeys_retry_background(void);
void oracle_obs_hotkeys_configure_background(void);
void oracle_obs_hotkeys_set_background_probe(bool enable);

#define obs control_read_core()
#define obs_hotkey_seam_lock control_lock
#define obs_hotkey_seam_unlock control_unlock
#define obs_hotkeys_retry_background oracle_obs_hotkeys_retry_background
#define obs_hotkeys_configure_background oracle_obs_hotkeys_configure_background
#define obs_hotkeys_set_background_probe oracle_obs_hotkeys_set_background_probe
#include "obs-hotkey-background-control.c.h"
#undef obs_hotkeys_retry_background
#undef obs_hotkeys_configure_background
#undef obs_hotkeys_set_background_probe
#include "hotkey_background_control_fields.inc"
#undef obs_hotkey_seam_lock
#undef obs_hotkey_seam_unlock
#undef obs

void oracle_hotkey_control_set(bool present, bool retry, bool configure, bool probe, uint32_t flags, bool callback);
void oracle_hotkey_control_set(bool present, bool retry, bool configure, bool probe, uint32_t flags, bool callback)
{
	assert(!control_locked);
	control_present = present;
	control_core.hotkeys.background.retry = retry;
	control_core.hotkeys.background.configure = configure;
	control_core.hotkeys.background.probe = probe;
	control_core.hotkeys.background.flags = flags;
	control_core.hotkeys.background.wake = callback ? control_wake : NULL;
	control_core.hotkeys.background.wake_data = &control_core;
	control_locks = control_unlocks = control_wakes = 0;
	memset(control_callback_state, 0, sizeof(control_callback_state));
}

void oracle_hotkey_control_snapshot(size_t *out);
void oracle_hotkey_control_snapshot(size_t *out)
{
	assert(!control_locked);
	out[0] = control_core.hotkeys.background.retry;
	out[1] = control_core.hotkeys.background.configure;
	out[2] = control_core.hotkeys.background.probe;
	out[3] = control_core.hotkeys.background.flags;
	out[4] = control_locks;
	out[5] = control_unlocks;
	out[6] = control_wakes;
	memcpy(out + 7, control_callback_state, sizeof(control_callback_state));
}

void oracle_hotkey_control_layout(size_t *out);
void oracle_hotkey_control_layout(size_t *out)
{
	out[0] = sizeof(control_core.hotkeys.background.retry);
	out[1] = sizeof(control_core.hotkeys.background.configure);
	out[2] = sizeof(control_core.hotkeys.background.probe);
	out[3] = sizeof(control_core.hotkeys.background.flags);
	out[4] = _Alignof(bool);
	out[5] = _Alignof(uint32_t);
	out[6] = OBS_HOTKEYS_BACKGROUND_CONFIGURE_FAILED;
}
