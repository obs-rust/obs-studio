/* The real private audio-capture adapters, compiled against the recording
 * mutex of test_source_audio_capture_callbacks.c, so its unchanged lock and
 * mutation-under-lock assertions see the Rust registration (#5243). */
#include "libobs/build/libobs.pch.h"

int source_audio_capture_record_lock(pthread_mutex_t *mutex);
int source_audio_capture_record_unlock(pthread_mutex_t *mutex);

#define pthread_mutex_lock source_audio_capture_record_lock
#define pthread_mutex_unlock source_audio_capture_record_unlock
#include "libobs/obs-source-audio-capture-callback-fields.c.h"
#undef pthread_mutex_unlock
#undef pthread_mutex_lock
