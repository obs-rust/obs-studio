/* Original C query and real private layout. No exclusions. */
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#define obs_output_get_frames_dropped oracle_obs_output_get_frames_dropped
#include "obs-output-frames-dropped.c.h"
#undef obs_output_get_frames_dropped
#include "output_frames_dropped_fields.inc"

extern int obs_output_get_frames_dropped(const obs_output_t *output);

void oracle_output_frames_dropped_layout(size_t *layout);
void oracle_output_frames_dropped_layout(size_t *layout)
{
    layout[0] = sizeof(struct obs_output_frames_dropped_fields);
    layout[1] = _Alignof(struct obs_output_frames_dropped_fields);
    layout[2] = offsetof(struct obs_output_frames_dropped_fields, callback);
    layout[3] = offsetof(struct obs_output_frames_dropped_fields, data);
    layout[4] = sizeof(int);
    layout[5] = _Alignof(int);
}

struct output_frames_dropped_context {
    int result;
    unsigned calls;
    void *seen;
};
struct output_frames_dropped_fixture {
    struct obs_output output;
    struct obs_output before;
    struct output_frames_dropped_context context;
};
static int output_frames_dropped_callback(void *data)
{
    if (!data)
        return -73;
    struct output_frames_dropped_context *context = data;
    context->calls++;
    context->seen = data;
    return context->result;
}
void *oracle_output_frames_dropped_new(int result, bool callback, bool null_data);
void *oracle_output_frames_dropped_new(int result, bool callback, bool null_data)
{
    struct output_frames_dropped_fixture *fixture = calloc(1, sizeof(*fixture));
    if (fixture) {
        fixture->context.result = result;
        fixture->output.info.get_dropped_frames = callback ? output_frames_dropped_callback : NULL;
        fixture->output.context.data = null_data ? NULL : &fixture->context;
        fixture->output.starting_lagged_count = 17;
        memcpy(&fixture->before, &fixture->output, sizeof(fixture->before));
    }
    return fixture;
}
bool oracle_output_frames_dropped_state(void *output, unsigned calls);
bool oracle_output_frames_dropped_state(void *output, unsigned calls)
{
    struct output_frames_dropped_fixture *fixture = output;
    return memcmp(&fixture->output, &fixture->before, sizeof(fixture->output)) == 0 &&
           fixture->context.calls == calls &&
           fixture->context.seen == (calls ? &fixture->context : NULL);
}
void oracle_output_frames_dropped_reset(void *output);
void oracle_output_frames_dropped_reset(void *output)
{
    struct output_frames_dropped_fixture *fixture = output;
    fixture->context.calls = 0;
    fixture->context.seen = NULL;
}
void oracle_output_frames_dropped_free(void *output);
void oracle_output_frames_dropped_free(void *output)
{
    free(output);
}

struct output_frames_dropped_log {
    unsigned count;
    int level;
    char message[128];
};
#if defined(__GNUC__) || defined(__clang__)
static void output_frames_dropped_capture(int level, const char *format, va_list args, void *param)
    __attribute__((format(printf, 2, 0)));
#endif
static void output_frames_dropped_capture(int level, const char *format, va_list args, void *param)
{
    struct output_frames_dropped_log *log = param;
    log->count++;
    log->level = level;
    vsnprintf(log->message, sizeof(log->message), format, args);
}
bool oracle_output_frames_dropped_null_log(bool rust);
bool oracle_output_frames_dropped_null_log(bool rust)
{
    struct output_frames_dropped_log log = {0};
    log_handler_t old_handler;
    void *old_param;
    base_get_log_handler(&old_handler, &old_param);
    base_set_log_handler(output_frames_dropped_capture, &log);
    int frames = rust ? obs_output_get_frames_dropped(NULL) : oracle_obs_output_get_frames_dropped(NULL);
    base_set_log_handler(old_handler, old_param);
    return frames == 0 && log.count == 1 && log.level == LOG_DEBUG &&
           strcmp(log.message, "obs_output_get_frames_dropped: Null 'output' parameter") == 0;
}
