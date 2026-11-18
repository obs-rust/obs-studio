/* Original device-load C and production adapters over a thread-local recorder. */
#include <stdarg.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

void oracle_gs_load_vertexbuffer(gs_vertbuffer_t *buffer);
void oracle_gs_load_indexbuffer(gs_indexbuffer_t *buffer);
void oracle_gs_load_texture(gs_texture_t *texture, int unit);
void oracle_gs_load_samplerstate(gs_samplerstate_t *sampler, int unit);
void oracle_gs_load_default_samplerstate(bool b_3d, int unit);

static THREAD_LOCAL struct graphics_subsystem dl_graphics;
static THREAD_LOCAL unsigned dl_calls, dl_logs;
static THREAD_LOCAL int dl_kind, dl_unit, dl_level;
static THREAD_LOCAL const void *dl_device, *dl_object;
static THREAD_LOCAL bool dl_3d;
static THREAD_LOCAL char dl_log[160];

#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void dl_blog(int level, const char *format, ...)
{
    va_list args;
    va_start(args, format);
    dl_logs++;
    dl_level = level;
    vsnprintf(dl_log, sizeof(dl_log), format, args);
    va_end(args);
}

#define blog dl_blog
#define gs_load_vertexbuffer oracle_gs_load_vertexbuffer
#define gs_load_indexbuffer oracle_gs_load_indexbuffer
#define gs_load_texture oracle_gs_load_texture
#define gs_load_samplerstate oracle_gs_load_samplerstate
#define gs_load_default_samplerstate oracle_gs_load_default_samplerstate
#include "graphics-device-load.inc"
#undef gs_load_vertexbuffer
#undef gs_load_indexbuffer
#undef gs_load_texture
#undef gs_load_samplerstate
#undef gs_load_default_samplerstate

/* graphics.c.h's original context validator, verbatim. */
static inline bool gs_valid(const char *f)
{
    if (!thread_graphics) {
        blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
        return false;
    }
    return true;
}

static graphics_t *dl_context(void)
{
    return thread_graphics;
}
#define gs_get_context dl_context
#include "graphics-device-load-fields.inc"
#undef gs_get_context
#undef blog

static void dl_record(int kind, gs_device_t *device, const void *object, int unit, bool b_3d)
{
    dl_calls++;
    dl_kind = kind;
    dl_device = device;
    dl_object = object;
    dl_unit = unit;
    dl_3d = b_3d;
}

static void dl_vertexbuffer(gs_device_t *device, gs_vertbuffer_t *buffer)
{
    dl_record(0, device, buffer, 0, false);
}

static void dl_indexbuffer(gs_device_t *device, gs_indexbuffer_t *buffer)
{
    dl_record(1, device, buffer, 0, false);
}

static void dl_texture(gs_device_t *device, gs_texture_t *texture, int unit)
{
    dl_record(2, device, texture, unit, false);
}

static void dl_samplerstate(gs_device_t *device, gs_samplerstate_t *sampler, int unit)
{
    dl_record(3, device, sampler, unit, false);
}

static void dl_default_samplerstate(gs_device_t *device, bool b_3d, int unit)
{
    dl_record(4, device, NULL, unit, b_3d);
}

void oracle_device_load_reset(bool context, void *device);
void oracle_device_load_reset(bool context, void *device)
{
    memset(&dl_graphics, 0, sizeof(dl_graphics));
    dl_graphics.device = (gs_device_t *)device;
    dl_graphics.exports.device_load_vertexbuffer = dl_vertexbuffer;
    dl_graphics.exports.device_load_indexbuffer = dl_indexbuffer;
    dl_graphics.exports.device_load_texture = dl_texture;
    dl_graphics.exports.device_load_samplerstate = dl_samplerstate;
    dl_graphics.exports.device_load_default_samplerstate = dl_default_samplerstate;
    thread_graphics = context ? &dl_graphics : NULL;
    dl_calls = dl_logs = 0;
    dl_kind = -1;
    dl_unit = dl_level = 0;
    dl_device = dl_object = NULL;
    dl_3d = false;
    dl_log[0] = '\0';
}

unsigned oracle_device_load_calls(void);
unsigned oracle_device_load_calls(void)
{
    return dl_calls;
}

unsigned oracle_device_load_logs(void);
unsigned oracle_device_load_logs(void)
{
    return dl_logs;
}

int oracle_device_load_kind(void);
int oracle_device_load_kind(void)
{
    return dl_kind;
}

int oracle_device_load_unit(void);
int oracle_device_load_unit(void)
{
    return dl_unit;
}

int oracle_device_load_level(void);
int oracle_device_load_level(void)
{
    return dl_level;
}

const void * oracle_device_load_device(void);
const void * oracle_device_load_device(void)
{
    return dl_device;
}

const void * oracle_device_load_object(void);
const void * oracle_device_load_object(void)
{
    return dl_object;
}

bool oracle_device_load_default_3d(void);
bool oracle_device_load_default_3d(void)
{
    return dl_3d;
}

const char * oracle_device_load_log(void);
const char * oracle_device_load_log(void)
{
    return dl_log;
}
