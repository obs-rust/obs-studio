/* Original query and real-header adapter; unported seams use callbacks only. */
#include "libobs/build/libobs.pch.h"
#include <stddef.h>
#include <assert.h>
#include <string.h>

static _Thread_local struct obs_core app_id_core;
static _Thread_local bool app_id_initialized;
static _Thread_local bool app_id_locked;
static _Thread_local size_t app_id_counts[3];

static struct obs_core *app_id_read_core(void)
{
	assert(app_id_locked);
	app_id_counts[1]++;
	return &app_id_core;
}

static bool app_id_lock(void)
{
	assert(!app_id_locked);
	app_id_counts[0]++;
	app_id_locked = app_id_initialized;
	return app_id_initialized;
}

static void app_id_unlock(void)
{
	assert(app_id_locked);
	app_id_counts[2]++;
	app_id_locked = false;
}

const char *oracle_obs_hotkeys_get_background_app_id(void);

#define obs app_id_read_core()
#define obs_hotkey_seam_lock app_id_lock
#define obs_hotkey_seam_unlock app_id_unlock
#define obs_hotkeys_get_background_app_id oracle_obs_hotkeys_get_background_app_id
#include "obs-hotkey-background-app-id.c.h"
#undef obs_hotkeys_get_background_app_id
#include "hotkey_background_app_id_fields.inc"
#undef obs_hotkey_seam_lock
#undef obs_hotkey_seam_unlock
#undef obs

void oracle_hotkey_app_id_set(bool initialized, const char *id);
void oracle_hotkey_app_id_set(bool initialized, const char *id)
{
	assert(!app_id_locked);
	app_id_initialized = initialized;
	app_id_core.hotkeys.background.app_id = id;
	memset(app_id_counts, 0, sizeof(app_id_counts));
}

void oracle_hotkey_app_id_counts(size_t *out);
void oracle_hotkey_app_id_counts(size_t *out)
{
	assert(!app_id_locked);
	memcpy(out, app_id_counts, sizeof(app_id_counts));
}

void oracle_hotkey_app_id_layout(size_t *out);
void oracle_hotkey_app_id_layout(size_t *out)
{
	out[0] = sizeof(app_id_core.hotkeys.background.app_id);
	out[1] = _Alignof(char *);
	out[2] = sizeof(const char *const *);
	out[3] = _Alignof(const char *const *);
	out[4] = sizeof(bool);
	out[5] = _Alignof(bool);
}
