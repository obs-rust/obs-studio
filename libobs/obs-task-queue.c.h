#include "libobs/build/libobs.pch.h"

/* Defined in obs.c.h, beside set_audio_thread(), which obs_init_audio() queues;
 * obs-video.c.h sets is_graphics_thread on the graphics thread. */
extern THREAD_LOCAL bool is_graphics_thread;
extern THREAD_LOCAL bool is_audio_thread;

struct task_wait_info {
	obs_task_t task;
	void *param;
	os_event_t *event;
	/* Set instead of the event when it could not be created. */
	volatile bool done;
};

static void task_wait_callback(void *param)
{
	struct task_wait_info *info = param;
	if (info->task)
		info->task(info->param);
	if (info->event)
		os_event_signal(info->event);
	else
		os_atomic_set_bool(&info->done, true);
}

/* Waits for task_wait_callback(). Without an event (its init failed), it
 * polls: the task still runs once, on its own thread, and the caller still
 * waits for it, which a caller whose param lives on its stack relies on. */
static void task_wait_for_callback(struct task_wait_info *info)
{
	if (info->event) {
		os_event_wait(info->event);
		return;
	}

	while (!os_atomic_load_bool(&info->done))
		os_sleep_ms(1);
	os_atomic_set_bool(&info->done, false);
}

_Static_assert(OBS_CORE_SETUP_RULES_TASK_UI == OBS_TASK_UI, "OBS_TASK_UI");
_Static_assert(OBS_CORE_SETUP_RULES_TASK_GRAPHICS == OBS_TASK_GRAPHICS, "OBS_TASK_GRAPHICS");
_Static_assert(OBS_CORE_SETUP_RULES_TASK_AUDIO == OBS_TASK_AUDIO, "OBS_TASK_AUDIO");
_Static_assert(OBS_CORE_SETUP_RULES_TASK_DESTROY == OBS_TASK_DESTROY, "OBS_TASK_DESTROY");

bool obs_in_task_thread(enum obs_task_type type)
{
	switch (obs_core_setup_rules_task_thread((int)type)) {
	case OBS_CORE_SETUP_RULES_TASK_GRAPHICS:
		return is_graphics_thread;
	case OBS_CORE_SETUP_RULES_TASK_AUDIO:
		return is_audio_thread;
	case OBS_CORE_SETUP_RULES_TASK_UI:
		return obs_core_is_ui_thread();
	case OBS_CORE_SETUP_RULES_TASK_DESTROY:
		return os_task_queue_inside(obs->destruction_task_thread);
	}

	assert(false);
	return false;
}

void obs_queue_task(enum obs_task_type type, obs_task_t task, void *param, bool wait)
{
	/* obs_core_setup_rules_task_step(): each read is made only on the
	 * step that asks for it, so a UI task reads only the handler and never
	 * asks obs_in_task_thread(). */
	enum obs_core_setup_rules_task step =
		obs_core_setup_rules_task_step(OBS_CORE_SETUP_RULES_TASK_TYPE, (int)type, false);
	if (step == OBS_CORE_SETUP_RULES_TASK_UI_HANDLER)
		step = obs_core_setup_rules_task_step(step, (int)type, obs->ui_task_handler != NULL);
	if (step == OBS_CORE_SETUP_RULES_TASK_IN_THREAD)
		step = obs_core_setup_rules_task_step(step, (int)type, obs_in_task_thread(type));
	if (step == OBS_CORE_SETUP_RULES_TASK_WAIT)
		step = obs_core_setup_rules_task_step(step, (int)type, wait);

	switch (step) {
	case OBS_CORE_SETUP_RULES_TASK_UI_CALL:
		obs->ui_task_handler(task, param, wait);
		break;

	case OBS_CORE_SETUP_RULES_TASK_UI_NONE:
		blog(LOG_ERROR, "UI task could not be queued, "
				"there's no UI task handler!");
		break;

	case OBS_CORE_SETUP_RULES_TASK_RUN:
		task(param);
		break;

	case OBS_CORE_SETUP_RULES_TASK_WAIT_QUEUE: {
		struct task_wait_info info = {
			.task = task,
			.param = param,
		};

		if (os_event_init(&info.event, OS_EVENT_TYPE_MANUAL) != 0)
			blog(LOG_WARNING, "obs_queue_task: Failed to create the wait event, "
					  "polling for the task instead");
		obs_queue_task(type, task_wait_callback, &info, false);
		task_wait_for_callback(&info);
		os_event_destroy(info.event);
		break;
	}

	case OBS_CORE_SETUP_RULES_TASK_PUSH_GRAPHICS: {
		struct obs_core_video *video = &obs->video;
		struct obs_task_info info = {task, param};

		pthread_mutex_lock(&video->task_mutex);
		deque_push_back(&video->tasks, &info, sizeof(info));
		pthread_mutex_unlock(&video->task_mutex);
		break;
	}

	case OBS_CORE_SETUP_RULES_TASK_PUSH_AUDIO: {
		struct obs_core_audio *audio = &obs->audio;
		struct obs_task_info info = {task, param};

		pthread_mutex_lock(&audio->task_mutex);
		deque_push_back(&audio->tasks, &info, sizeof(info));
		pthread_mutex_unlock(&audio->task_mutex);
		break;
	}

	case OBS_CORE_SETUP_RULES_TASK_PUSH_DESTROY: {
		os_task_t os_task = (os_task_t)task;
		os_task_queue_queue_task(obs->destruction_task_thread, os_task, param);
		break;
	}

	case OBS_CORE_SETUP_RULES_TASK_UNKNOWN:
		blog(LOG_ERROR, "obs_queue_task: unknown task type %d, the task was not queued", (int)type);
		break;

	case OBS_CORE_SETUP_RULES_TASK_DROP:
	case OBS_CORE_SETUP_RULES_TASK_TYPE:
	case OBS_CORE_SETUP_RULES_TASK_UI_HANDLER:
	case OBS_CORE_SETUP_RULES_TASK_IN_THREAD:
	case OBS_CORE_SETUP_RULES_TASK_WAIT:
		break;
	}
}

bool obs_wait_for_destroy_queue(void)
{
	struct task_wait_info info = {0};

	/* allow video and audio threads time to release objects. Each thread
	 * is waited for when it runs: video without audio still ticks sources
	 * and holds references on them while it does. Without either thread
	 * sources are still destroyed by the destruction thread, so that queue
	 * is waited for regardless. */
	const bool wait_video = obs->video.thread_initialized;
	const bool wait_audio = obs->audio.audio != NULL;
	if (wait_video || wait_audio) {
		if (os_event_init(&info.event, OS_EVENT_TYPE_AUTO) != 0)
			blog(LOG_WARNING, "obs_wait_for_destroy_queue: Failed to create the wait event, "
					  "polling instead");
		if (wait_video) {
			obs_queue_task(OBS_TASK_GRAPHICS, task_wait_callback, &info, false);
			task_wait_for_callback(&info);
		}
		if (wait_audio) {
			obs_queue_task(OBS_TASK_AUDIO, task_wait_callback, &info, false);
			task_wait_for_callback(&info);
		}
		os_event_destroy(info.event);
	}

	/* wait for destroy task queue */
	return os_task_queue_wait(obs->destruction_task_thread);
}
