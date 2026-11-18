/* Original setter and the real private adapter over a thread-local core. */
#include "libobs/build/libobs.pch.h"
#include <assert.h>
#include <string.h>

static _Thread_local struct obs_core press_core;
static _Thread_local bool press_present, press_held, press_active, press_expected;
static _Thread_local size_t press_counts[10];
static _Thread_local int press_cookie;

static struct obs_core *press_read_core(void)
{
	assert(press_present);
	return &press_core;
}

static bool press_lock(void)
{
	assert(!press_held);
	press_counts[1]++;
	press_held = press_present;
	return press_held;
}

static void press_unlock(void)
{
	assert(press_held);
	press_counts[2]++;
	press_held = false;
}

static void press_listener(void *data, calldata_t *params)
{
	assert(data == &press_cookie);
	assert(press_active && !press_held);
	assert(press_core.hotkeys.thread_disable_press == !press_expected);
	bool enabled = !press_expected;
	assert(calldata_get_bool(params, "enabled", &enabled));
	assert(enabled == press_expected);
	assert(params->fixed && params->stack && params->capacity == 128);
	assert(params->size > 0 && params->size <= params->capacity);
	assert(press_lock());
	assert(press_core.hotkeys.background.state == OBS_HOTKEYS_BACKGROUND_NATIVE);
	assert(press_core.hotkeys.background.flags == 0x1234);
	press_unlock();
	press_counts[4]++;
}

static void press_signal(signal_handler_t *handler, const char *name, calldata_t *params)
{
	assert(press_active && !press_held);
	assert(handler == press_core.hotkeys.signals);
	assert(strcmp(name, "hotkey_background_press_changed") == 0);
	bool enabled = !press_expected;
	assert(calldata_get_bool(params, "enabled", &enabled));
	assert(enabled == press_expected);
	assert(press_core.hotkeys.thread_disable_press == !enabled);
	assert(params->fixed && params->stack && params->capacity == 128);
	assert(params->size > 0 && params->size <= params->capacity);
	press_counts[3]++;
	press_counts[5] = enabled;
	press_counts[6] = params->fixed;
	press_counts[7] = params->capacity;
	press_counts[8] = params->size;
	signal_handler_signal(handler, name, params);
}

void oracle_obs_hotkey_enable_background_press(bool enable);
#define obs press_read_core()
#define obs_hotkey_seam_lock press_lock
#define obs_hotkey_seam_unlock press_unlock
#define signal_handler_signal press_signal
#define obs_hotkey_enable_background_press oracle_obs_hotkey_enable_background_press
#include "libobs/obs-hotkey-background-press.c.h"
#undef obs_hotkey_enable_background_press
#include "hotkey_background_press_fields.inc"
#undef signal_handler_signal
#undef obs_hotkey_seam_unlock
#undef obs_hotkey_seam_lock
#undef obs

void oracle_hotkey_background_press_init(bool present, bool disabled, bool listener);
void oracle_hotkey_background_press_init(bool present, bool disabled, bool listener)
{
	assert(!press_held && !press_core.hotkeys.signals);
	press_present = present;
	press_core.hotkeys.thread_disable_press = disabled;
	press_core.hotkeys.background.state = OBS_HOTKEYS_BACKGROUND_NATIVE;
	press_core.hotkeys.background.flags = 0x1234;
	press_core.hotkeys.signals = signal_handler_create();
	assert(press_core.hotkeys.signals);
	assert(signal_handler_add(press_core.hotkeys.signals, "void hotkey_background_press_changed(bool enabled)"));
	if (listener)
		signal_handler_connect(press_core.hotkeys.signals, "hotkey_background_press_changed", press_listener,
				       &press_cookie);
	memset(press_counts, 0, sizeof(press_counts));
}

void oracle_hotkey_background_press_step(bool present, bool enabled);
void oracle_hotkey_background_press_step(bool present, bool enabled)
{
	assert(!press_held);
	press_present = present;
	press_expected = enabled;
	press_active = true;
}

void oracle_hotkey_background_press_snapshot(size_t *out);
void oracle_hotkey_background_press_snapshot(size_t *out)
{
	assert(!press_held);
	press_active = false;
	press_counts[0] = press_core.hotkeys.thread_disable_press;
	press_counts[9] = press_core.hotkeys.background.flags;
	memcpy(out, press_counts, sizeof(press_counts));
}

void oracle_hotkey_background_press_free(void);
void oracle_hotkey_background_press_free(void)
{
	assert(!press_held);
	signal_handler_disconnect(press_core.hotkeys.signals, "hotkey_background_press_changed", press_listener,
				  &press_cookie);
	signal_handler_destroy(press_core.hotkeys.signals);
	press_core.hotkeys.signals = NULL;
}
