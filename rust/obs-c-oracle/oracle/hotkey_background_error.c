/* Original getter and real-header adapter. No placeholder obs global. */
#include "libobs/build/libobs.pch.h"
#include <assert.h>
#include <stddef.h>
#include <string.h>

static _Thread_local struct obs_core background_error_core;
static _Thread_local bool background_error_available;
static _Thread_local bool background_error_held;
static _Thread_local size_t background_error_counts[3];

static struct obs_core *background_error_read(void)
{
	assert(background_error_held);
	background_error_counts[1] = 1;
	return &background_error_core;
}

bool obs_hotkey_background_error_test_lock(void);
bool obs_hotkey_background_error_test_lock(void)
{
	assert(!background_error_held);
	background_error_counts[0]++;
	background_error_held = background_error_available;
	return background_error_available;
}

void obs_hotkey_background_error_test_unlock(void);
void obs_hotkey_background_error_test_unlock(void)
{
	assert(background_error_held);
	background_error_counts[2]++;
	background_error_held = false;
}

char *oracle_obs_hotkeys_get_background_error(void);
#define obs background_error_read()
#define obs_hotkeys_get_background_error oracle_obs_hotkeys_get_background_error
#define obs_hotkey_seam_lock obs_hotkey_background_error_test_lock
#define obs_hotkey_seam_unlock obs_hotkey_background_error_test_unlock
#include "libobs/obs-hotkey-background-error.c.h"
#include "hotkey_background_error_fields.inc"
#undef obs_hotkey_seam_unlock
#undef obs_hotkey_seam_lock
#undef obs_hotkeys_get_background_error
#undef obs

void oracle_hotkey_background_error_set(bool available, char *error);
void oracle_hotkey_background_error_set(bool available, char *error)
{
	assert(!background_error_held);
	background_error_available = available;
	background_error_core.hotkeys.background.error = error;
	memset(background_error_counts, 0, sizeof(background_error_counts));
}

void oracle_hotkey_background_error_counts(size_t *out);
void oracle_hotkey_background_error_counts(size_t *out)
{
	assert(!background_error_held);
	memcpy(out, background_error_counts, sizeof(background_error_counts));
}

void oracle_hotkey_background_error_layout(size_t *out);
void oracle_hotkey_background_error_layout(size_t *out)
{
	out[0] = sizeof(background_error_core.hotkeys.background.error);
	out[1] = _Alignof(char *);
}
