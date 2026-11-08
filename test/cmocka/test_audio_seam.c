/*
 * The core-audio seam (libobs/obs-audio-seam.h, obs-rust/obs-studio#4046),
 * which the port of the audio tick (libobs/obs-audio.c) calls in place of
 * the struct reads, writes, locks and container calls it wraps:
 * - the core audio: the output, render_order and root_nodes, the
 *   buffered timestamps, the buffering fields, the monitoring duplicate and
 *   the task queue;
 * - the core data and video: the audio source list and its lock, the mixes
 *   and their lock, and a view's channels and their lock;
 * - a source: the next audio source, its buffer lock, the duplicate and
 *   pending flags, its audio timestamp and input, its type, flags and
 *   audio_render, its output buffers, and its mute, volume, mixers and
 *   monitoring state.
 *
 * Each read is checked against the field it wraps, each write is read back
 * from the field, each lock and unlock returns 0 and holds (or lets go of)
 * the mutex it names, and the container calls leave the field as
 * da_push_back(), da_find(), da_resize(.., 0), deque_push_back(),
 * deque_peek_front() and deque_pop_front() do.
 *
 * The accessors are hidden in libobs, so this test compiles the seam into
 * itself and runs it on a stand-in core (`obs`, with only the fields the seam
 * reads) and stand-in sources and views: zeroed structs with their mutexes
 * initialized.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <string.h>

#include "obs-internal.h"
#include "obs-audio-seam.h"

/* libobs has this one in obs.c, hidden; the seam in this test reads this
 * stand-in. */
struct obs_core *obs;

static int tag;

static obs_source_t *new_source(void)
{
	obs_source_t *source = bzalloc(sizeof(*source));
	assert_int_equal(pthread_mutex_init(&source->audio_buf_mutex, NULL), 0);
	return source;
}

static void free_source(obs_source_t *source)
{
	for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
		deque_free(&source->audio_input_buf[ch]);
	pthread_mutex_destroy(&source->audio_buf_mutex);
	bfree(source);
}

static int setup(void **state)
{
	obs = bzalloc(sizeof(*obs));
	if (pthread_mutex_init(&obs->audio.task_mutex, NULL) != 0)
		return -1;
	if (pthread_mutex_init(&obs->data.audio_sources_mutex, NULL) != 0)
		return -1;
	if (pthread_mutex_init(&obs->video.mixes_mutex, NULL) != 0)
		return -1;
	(void)state;
	return 0;
}

static int teardown(void **state)
{
	da_free(obs->audio.render_order);
	da_free(obs->audio.root_nodes);
	deque_free(&obs->audio.buffered_timestamps);
	deque_free(&obs->audio.tasks);
	for (size_t i = 0; i < obs->video.mixes.num; i++)
		bfree(obs->video.mixes.array[i]);
	da_free(obs->video.mixes);
	pthread_mutex_destroy(&obs->audio.task_mutex);
	pthread_mutex_destroy(&obs->data.audio_sources_mutex);
	pthread_mutex_destroy(&obs->video.mixes_mutex);
	bfree(obs);
	obs = NULL;
	(void)state;
	return 0;
}

/* A lock holds the mutex (a trylock from this thread is refused) and its
 * unlock lets it go (a trylock then succeeds). */
static void check_held(pthread_mutex_t *mutex, int lock_result)
{
	assert_int_equal(lock_result, 0);
	assert_int_not_equal(pthread_mutex_trylock(mutex), 0);
}

static void check_released(pthread_mutex_t *mutex, int unlock_result)
{
	assert_int_equal(unlock_result, 0);
	assert_int_equal(pthread_mutex_trylock(mutex), 0);
	assert_int_equal(pthread_mutex_unlock(mutex), 0);
}

static void test_output(void **state)
{
	(void)state;
	obs->audio.audio = (audio_t *)&tag;
	assert_ptr_equal(obs_audio_seam_get_output(), &tag);
	obs->audio.audio = NULL;
	assert_null(obs_audio_seam_get_output());
}

static void test_render_order(void **state)
{
	(void)state;
	obs_source_t *a = new_source();
	obs_source_t *b = new_source();
	obs_source_t *absent = new_source();

	obs_audio_seam_render_order_push(a);
	obs_audio_seam_render_order_push(b);
	assert_int_equal(obs->audio.render_order.num, 2);
	assert_ptr_equal(obs->audio.render_order.array[0], a);
	assert_ptr_equal(obs->audio.render_order.array[1], b);

	assert_int_equal(obs_audio_seam_render_order_num(), 2);
	assert_ptr_equal(obs_audio_seam_render_order_get(0), a);
	assert_ptr_equal(obs_audio_seam_render_order_get(1), b);
	assert_int_equal(obs_audio_seam_render_order_find(a), 0);
	assert_int_equal(obs_audio_seam_render_order_find(b), 1);
	assert_int_equal(obs_audio_seam_render_order_find(absent), DARRAY_INVALID);

	/* da_resize(.., 0): empty, the allocation kept. */
	size_t capacity = obs->audio.render_order.capacity;
	obs_audio_seam_render_order_clear();
	assert_int_equal(obs->audio.render_order.num, 0);
	assert_int_equal(obs->audio.render_order.capacity, capacity);
	assert_int_equal(obs_audio_seam_render_order_num(), 0);
	assert_int_equal(obs_audio_seam_render_order_find(a), DARRAY_INVALID);

	free_source(a);
	free_source(b);
	free_source(absent);
}

static void test_root_nodes(void **state)
{
	(void)state;
	obs_source_t *a = new_source();

	/* A source mixed twice as a root node is pushed twice. */
	obs_audio_seam_root_nodes_push(a);
	obs_audio_seam_root_nodes_push(a);
	assert_int_equal(obs->audio.root_nodes.num, 2);
	assert_ptr_equal(obs->audio.root_nodes.array[0], a);
	assert_ptr_equal(obs->audio.root_nodes.array[1], a);
	assert_int_equal(obs_audio_seam_root_nodes_num(), 2);
	assert_ptr_equal(obs_audio_seam_root_nodes_get(1), a);

	size_t capacity = obs->audio.root_nodes.capacity;
	obs_audio_seam_root_nodes_clear();
	assert_int_equal(obs->audio.root_nodes.num, 0);
	assert_int_equal(obs->audio.root_nodes.capacity, capacity);
	assert_int_equal(obs_audio_seam_root_nodes_num(), 0);

	free_source(a);
}

static void test_buffered_timestamps(void **state)
{
	(void)state;
	struct ts_info first = {100, 200};
	struct ts_info second = {300, 400};
	struct ts_info out = {0, 0};

	obs_audio_seam_buffered_timestamps_push_back(&first);
	obs_audio_seam_buffered_timestamps_push_back(&second);
	assert_int_equal(obs->audio.buffered_timestamps.size, 2 * sizeof(struct ts_info));

	obs_audio_seam_buffered_timestamps_peek_front(&out);
	assert_int_equal(out.start, 100);
	assert_int_equal(out.end, 200);
	/* A peek leaves the queue as it was. */
	assert_int_equal(obs->audio.buffered_timestamps.size, 2 * sizeof(struct ts_info));

	obs_audio_seam_buffered_timestamps_pop_front();
	assert_int_equal(obs->audio.buffered_timestamps.size, sizeof(struct ts_info));
	obs_audio_seam_buffered_timestamps_peek_front(&out);
	assert_int_equal(out.start, 300);
	assert_int_equal(out.end, 400);
}

static void test_buffering(void **state)
{
	(void)state;
	obs->audio.max_buffering_ticks = 45;
	obs->audio.total_buffering_ticks = 3;

	struct obs_audio_buffering buffering = obs_audio_seam_get_buffering();
	assert_ptr_equal(buffering.buffered_ts, &obs->audio.buffered_ts);
	assert_ptr_equal(buffering.buffered_timestamps, &obs->audio.buffered_timestamps);
	assert_ptr_equal(buffering.buffering_wait_ticks, &obs->audio.buffering_wait_ticks);
	assert_ptr_equal(buffering.total_buffering_ticks, &obs->audio.total_buffering_ticks);
	assert_int_equal(buffering.max_buffering_ticks, 45);

	obs->audio.fixed_buffer = true;
	assert_true(obs_audio_seam_get_fixed_buffer());
	obs->audio.fixed_buffer = false;
	assert_false(obs_audio_seam_get_fixed_buffer());

	obs_audio_seam_set_buffering_wait_ticks(7);
	assert_int_equal(obs->audio.buffering_wait_ticks, 7);
	assert_int_equal(obs_audio_seam_get_buffering_wait_ticks(), 7);
	obs->audio.buffering_wait_ticks = UINT64_MAX;
	assert_int_equal(obs_audio_seam_get_buffering_wait_ticks(), UINT64_MAX);
}

static void test_monitoring_duplicating_source(void **state)
{
	(void)state;
	obs_source_t *dup = new_source();

	assert_null(obs_audio_seam_get_monitoring_duplicating_source());
	obs->audio.monitoring_duplicating_source = dup;
	assert_ptr_equal(obs_audio_seam_get_monitoring_duplicating_source(), dup);
	obs->audio.monitoring_duplicating_source = NULL;

	free_source(dup);
}

static void count_task(void *param)
{
	(*(int *)param)++;
}

static void test_tasks(void **state)
{
	(void)state;
	int first_param = 0;
	int second_param = 0;
	struct obs_task_info first = {count_task, &first_param};
	struct obs_task_info second = {count_task, &second_param};
	struct obs_task_info out = {NULL, NULL};

	check_held(&obs->audio.task_mutex, obs_audio_seam_lock_tasks());
	check_released(&obs->audio.task_mutex, obs_audio_seam_unlock_tasks());

	deque_push_back(&obs->audio.tasks, &first, sizeof(first));
	deque_push_back(&obs->audio.tasks, &second, sizeof(second));
	assert_int_equal(obs_audio_seam_tasks_size(), 2 * sizeof(struct obs_task_info));

	obs_audio_seam_tasks_pop_front(&out);
	assert_ptr_equal(out.task, count_task);
	assert_ptr_equal(out.param, &first_param);
	assert_int_equal(obs->audio.tasks.size, sizeof(struct obs_task_info));
	assert_int_equal(obs_audio_seam_tasks_size(), sizeof(struct obs_task_info));

	obs_audio_seam_tasks_pop_front(&out);
	assert_ptr_equal(out.param, &second_param);
	assert_int_equal(obs_audio_seam_tasks_size(), 0);

	/* The seam hands the task back; it does not run it. */
	assert_int_equal(first_param, 0);
	assert_int_equal(second_param, 0);
}

static void test_audio_sources(void **state)
{
	(void)state;
	obs_source_t *a = new_source();
	obs_source_t *b = new_source();

	assert_null(obs_audio_seam_get_first_audio_source());
	obs->data.first_audio_source = a;
	a->next_audio_source = b;
	assert_ptr_equal(obs_audio_seam_get_first_audio_source(), a);
	assert_ptr_equal(obs_audio_seam_source_get_next(a), b);
	assert_null(obs_audio_seam_source_get_next(b));

	check_held(&obs->data.audio_sources_mutex, obs_audio_seam_lock_audio_sources());
	check_released(&obs->data.audio_sources_mutex, obs_audio_seam_unlock_audio_sources());

	obs->data.first_audio_source = NULL;
	free_source(a);
	free_source(b);
}

static struct obs_core_video_mix *add_mix(struct obs_view *view, bool mix_audio)
{
	struct obs_core_video_mix *mix = bzalloc(sizeof(*mix));
	mix->view = view;
	mix->mix_audio = mix_audio;
	da_push_back(obs->video.mixes, &mix);
	return mix;
}

static void test_mixes(void **state)
{
	(void)state;
	struct obs_view *view = bzalloc(sizeof(*view));

	assert_int_equal(obs_audio_seam_mixes_num(), 0);
	add_mix(view, true);
	add_mix(view, false);
	add_mix(NULL, true);
	assert_int_equal(obs_audio_seam_mixes_num(), 3);
	assert_ptr_equal(obs_audio_seam_mix_get_view(0), view);
	assert_ptr_equal(obs_audio_seam_mix_get_view(1), view);
	assert_null(obs_audio_seam_mix_get_view(2));
	assert_true(obs_audio_seam_mix_get_mix_audio(0));
	assert_false(obs_audio_seam_mix_get_mix_audio(1));
	assert_true(obs_audio_seam_mix_get_mix_audio(2));

	check_held(&obs->video.mixes_mutex, obs_audio_seam_lock_mixes());
	check_released(&obs->video.mixes_mutex, obs_audio_seam_unlock_mixes());

	bfree(view);
}

static void test_view_channels(void **state)
{
	(void)state;
	struct obs_view *view = bzalloc(sizeof(*view));
	assert_int_equal(pthread_mutex_init(&view->channels_mutex, NULL), 0);
	obs_source_t *a = new_source();
	obs_source_t *b = new_source();

	view->channels[0] = a;
	view->channels[MAX_CHANNELS - 1] = b;
	assert_ptr_equal(obs_audio_seam_view_get_channel(view, 0), a);
	assert_null(obs_audio_seam_view_get_channel(view, 1));
	assert_ptr_equal(obs_audio_seam_view_get_channel(view, MAX_CHANNELS - 1), b);

	check_held(&view->channels_mutex, obs_audio_seam_view_lock_channels(view));
	check_released(&view->channels_mutex, obs_audio_seam_view_unlock_channels(view));

	free_source(a);
	free_source(b);
	pthread_mutex_destroy(&view->channels_mutex);
	bfree(view);
}

static void test_source_lock(void **state)
{
	(void)state;
	obs_source_t *source = new_source();

	check_held(&source->audio_buf_mutex, obs_audio_seam_source_lock_buf(source));
	check_released(&source->audio_buf_mutex, obs_audio_seam_source_unlock_buf(source));

	free_source(source);
}

static void test_source_flags(void **state)
{
	(void)state;
	obs_source_t *source = new_source();

	obs_audio_seam_source_set_is_duplicated(source, true);
	assert_true(source->audio_is_duplicated);
	assert_true(obs_audio_seam_source_get_is_duplicated(source));
	source->audio_is_duplicated = false;
	assert_false(obs_audio_seam_source_get_is_duplicated(source));

	obs_audio_seam_source_set_pending(source, true);
	assert_true(source->audio_pending);
	assert_true(obs_audio_seam_source_get_pending(source));
	obs_audio_seam_source_set_pending(source, false);
	assert_false(source->audio_pending);
	source->audio_pending = true;
	assert_true(obs_audio_seam_source_get_pending(source));

	/* Each write changes only its own field. */
	assert_false(source->audio_is_duplicated);

	free_source(source);
}

static void test_source_input(void **state)
{
	(void)state;
	obs_source_t *source = new_source();
	float samples[16] = {0};

	source->audio_ts = 123456789;
	assert_int_equal(obs_audio_seam_source_get_audio_ts(source), 123456789);

	assert_int_equal(obs_audio_seam_source_get_input_size(source), 0);
	deque_push_back(&source->audio_input_buf[0], samples, sizeof(samples));
	/* Only channel 0's size is read. */
	deque_push_back(&source->audio_input_buf[1], samples, sizeof(float));
	assert_int_equal(obs_audio_seam_source_get_input_size(source), sizeof(samples));

	struct obs_audio_input in = obs_audio_seam_source_get_input(source);
	assert_ptr_equal(in.buf, source->audio_input_buf);
	assert_ptr_equal(in.audio_ts, &source->audio_ts);
	assert_ptr_equal(in.last_size, &source->last_audio_input_buf_size);
	assert_ptr_equal(in.pending, &source->audio_pending);
	assert_ptr_equal(in.pending_stop, &source->pending_stop);
	assert_ptr_equal(in.timing_set, &source->timing_set);

	free_source(source);
}

static bool fake_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio_output, uint32_t mixers,
			      size_t channels, size_t sample_rate)
{
	/* Never called: the seam only says whether the callback is set. */
	(void)data;
	(void)ts_out;
	(void)audio_output;
	(void)mixers;
	(void)channels;
	(void)sample_rate;
	return false;
}

static void test_source_info(void **state)
{
	(void)state;
	obs_source_t *source = new_source();

	source->info.type = OBS_SOURCE_TYPE_SCENE;
	source->info.output_flags = OBS_SOURCE_AUDIO | OBS_SOURCE_COMPOSITE;
	assert_int_equal(obs_audio_seam_source_get_type(source), OBS_SOURCE_TYPE_SCENE);
	assert_int_equal(obs_audio_seam_source_get_output_flags(source), OBS_SOURCE_AUDIO | OBS_SOURCE_COMPOSITE);
	source->info.type = OBS_SOURCE_TYPE_TRANSITION;
	assert_int_equal(obs_audio_seam_source_get_type(source), OBS_SOURCE_TYPE_TRANSITION);

	assert_false(obs_audio_seam_source_has_audio_render(source));
	source->info.audio_render = fake_audio_render;
	assert_true(obs_audio_seam_source_has_audio_render(source));

	free_source(source);
}

static void test_source_output_buf(void **state)
{
	(void)state;
	obs_source_t *source = new_source();
	float first = 0.0f;
	float last = 0.0f;

	source->audio_output_buf[0][0] = &first;
	source->audio_output_buf[MAX_AUDIO_MIXES - 1][MAX_AUDIO_CHANNELS - 1] = &last;

	obs_audio_seam_output_row_t *buf = obs_audio_seam_source_get_output_buf(source);
	assert_ptr_equal(buf, source->audio_output_buf);
	assert_ptr_equal(buf[0][0], &first);
	assert_null(buf[0][1]);
	assert_ptr_equal(buf[MAX_AUDIO_MIXES - 1][MAX_AUDIO_CHANNELS - 1], &last);

	free_source(source);
}

static void test_source_mix_controls(void **state)
{
	(void)state;
	obs_source_t *source = new_source();

	source->muted = true;
	source->volume = 0.25f;
	source->audio_mixers = 0x2a;
	assert_true(obs_audio_seam_source_get_muted(source));
	assert_true(obs_audio_seam_source_get_volume(source) == 0.25f);
	assert_int_equal(obs_audio_seam_source_get_audio_mixers(source), 0x2a);
	source->muted = false;
	source->volume = 1.0f;
	assert_false(obs_audio_seam_source_get_muted(source));
	assert_true(obs_audio_seam_source_get_volume(source) == 1.0f);

	os_atomic_store_bool(&source->monitoring_enabled, true);
	assert_true(obs_audio_seam_source_get_monitoring_enabled(source));
	os_atomic_store_bool(&source->monitoring_enabled, false);
	assert_false(obs_audio_seam_source_get_monitoring_enabled(source));

	free_source(source);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(test_output, setup, teardown),
		cmocka_unit_test_setup_teardown(test_render_order, setup, teardown),
		cmocka_unit_test_setup_teardown(test_root_nodes, setup, teardown),
		cmocka_unit_test_setup_teardown(test_buffered_timestamps, setup, teardown),
		cmocka_unit_test_setup_teardown(test_buffering, setup, teardown),
		cmocka_unit_test_setup_teardown(test_monitoring_duplicating_source, setup, teardown),
		cmocka_unit_test_setup_teardown(test_tasks, setup, teardown),
		cmocka_unit_test_setup_teardown(test_audio_sources, setup, teardown),
		cmocka_unit_test_setup_teardown(test_mixes, setup, teardown),
		cmocka_unit_test_setup_teardown(test_view_channels, setup, teardown),
		cmocka_unit_test_setup_teardown(test_source_lock, setup, teardown),
		cmocka_unit_test_setup_teardown(test_source_flags, setup, teardown),
		cmocka_unit_test_setup_teardown(test_source_input, setup, teardown),
		cmocka_unit_test_setup_teardown(test_source_info, setup, teardown),
		cmocka_unit_test_setup_teardown(test_source_output_buf, setup, teardown),
		cmocka_unit_test_setup_teardown(test_source_mix_controls, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
