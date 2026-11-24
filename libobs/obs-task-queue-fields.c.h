#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust task queue entry points (obs-task-queue.c.h,
 * #5562): the graphics and audio thread flags, the UI task handler (its NULL
 * check and its call), the destruction task queue, the video thread's
 * initialized flag, the audio output, and the graphics and audio task queues
 * (each lock, push and unlock on its own, so the Rust makes the C's calls in
 * the C's order). Each reads the field as the original did, with no reference
 * taken; the layouts stay in C. */

/* Defined in obs.c.h, beside set_audio_thread(), which obs_init_audio() queues;
 * obs-video.c.h sets is_graphics_thread on the graphics thread. */
extern THREAD_LOCAL bool is_graphics_thread;
extern THREAD_LOCAL bool is_audio_thread;

/* The Rust passes the obs_task_type value to the route rules unchanged. */
_Static_assert(OBS_CORE_SETUP_RULES_TASK_UI == OBS_TASK_UI, "OBS_TASK_UI");
_Static_assert(OBS_CORE_SETUP_RULES_TASK_GRAPHICS == OBS_TASK_GRAPHICS, "OBS_TASK_GRAPHICS");
_Static_assert(OBS_CORE_SETUP_RULES_TASK_AUDIO == OBS_TASK_AUDIO, "OBS_TASK_AUDIO");
_Static_assert(OBS_CORE_SETUP_RULES_TASK_DESTROY == OBS_TASK_DESTROY, "OBS_TASK_DESTROY");

bool obs_task_queue_graphics_thread(void);
bool obs_task_queue_graphics_thread(void)
{
	return is_graphics_thread;
}

bool obs_task_queue_audio_thread(void);
bool obs_task_queue_audio_thread(void)
{
	return is_audio_thread;
}

os_task_queue_t *obs_task_queue_destruction(void);
os_task_queue_t *obs_task_queue_destruction(void)
{
	return obs->destruction_task_thread;
}

bool obs_task_queue_has_ui_handler(void);
bool obs_task_queue_has_ui_handler(void)
{
	return obs->ui_task_handler != NULL;
}

void obs_task_queue_call_ui_handler(obs_task_t task, void *param, bool wait);
void obs_task_queue_call_ui_handler(obs_task_t task, void *param, bool wait)
{
	obs->ui_task_handler(task, param, wait);
}

void obs_task_queue_lock_graphics(void);
void obs_task_queue_lock_graphics(void)
{
	pthread_mutex_lock(&obs->video.task_mutex);
}

void obs_task_queue_push_graphics(obs_task_t task, void *param);
void obs_task_queue_push_graphics(obs_task_t task, void *param)
{
	struct obs_task_info info = {task, param};
	deque_push_back(&obs->video.tasks, &info, sizeof(info));
}

void obs_task_queue_unlock_graphics(void);
void obs_task_queue_unlock_graphics(void)
{
	pthread_mutex_unlock(&obs->video.task_mutex);
}

void obs_task_queue_lock_audio(void);
void obs_task_queue_lock_audio(void)
{
	pthread_mutex_lock(&obs->audio.task_mutex);
}

void obs_task_queue_push_audio(obs_task_t task, void *param);
void obs_task_queue_push_audio(obs_task_t task, void *param)
{
	struct obs_task_info info = {task, param};
	deque_push_back(&obs->audio.tasks, &info, sizeof(info));
}

void obs_task_queue_unlock_audio(void);
void obs_task_queue_unlock_audio(void)
{
	pthread_mutex_unlock(&obs->audio.task_mutex);
}

bool obs_task_queue_video_thread_initialized(void);
bool obs_task_queue_video_thread_initialized(void)
{
	return obs->video.thread_initialized;
}

bool obs_task_queue_has_audio(void);
bool obs_task_queue_has_audio(void)
{
	return obs->audio.audio != NULL;
}
