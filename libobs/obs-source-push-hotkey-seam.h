#pragma once

#include <stdbool.h>
#include <stdint.h>

void obs_source_push_hotkey_dispatch(void *source, bool talk, bool pressed, uint64_t (*clock)(void),
				     void (*lock)(void *), void (*append)(void *, uint64_t, bool, bool),
				     void (*unlock)(void *), void (*store)(void *, bool, bool));
void obs_source_push_hotkey_lock(void *source);
void obs_source_push_hotkey_append(void *source, uint64_t timestamp, bool talk, bool pressed);
void obs_source_push_hotkey_unlock(void *source);
void obs_source_push_hotkey_store(void *source, bool talk, bool pressed);
