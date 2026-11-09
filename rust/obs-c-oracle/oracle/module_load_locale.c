/* Unchanged extracted C orchestration with deterministic per-thread services.
 * Only module->file is read; the private layout is not exposed to Rust. */
#include <stdarg.h>
#include <stdbool.h>
#include <string.h>
#include "util/base.h"
#include "util/bmem.h"

typedef struct obs_module {
	const char *file;
} obs_module_t;
typedef struct locale_lookup {
	int unused;
} lookup_t;
typedef bool (*loader_event_t)(void *, unsigned int, const char *);
struct loader_context {
	loader_event_t event;
	void *data;
	lookup_t lookup;
};
#ifdef _MSC_VER
static __declspec(thread) struct loader_context *loader_context;
#else
static _Thread_local struct loader_context *loader_context;
#endif

extern char *oracle_obs_module_locale_default_file(const char *locale);
extern char *oracle_obs_module_locale_file(const char *locale);
extern bool oracle_obs_module_locale_is_default(const char *locale, const char *default_locale);

static char *loader_find(obs_module_t *module, const char *name)
{
	(void)module;
	return loader_context->event(loader_context->data, 0, name) ? bstrdup(name) : NULL;
}
static lookup_t *loader_create(const char *file)
{
	return loader_context->event(loader_context->data, 1, file) ? &loader_context->lookup : NULL;
}
static bool loader_add(lookup_t *lookup, const char *file)
{
	(void)lookup;
	return loader_context->event(loader_context->data, 2, file);
}
#if defined(__GNUC__) || defined(__clang__)
__attribute__((__format__(__printf__, 2, 3)))
#endif
static void loader_log(int level, const char *format, ...);
static void loader_log(int level, const char *format, ...)
{
	(void)level;
	if (strcmp(format, "obs_module_load_locale: Invalid parameters") == 0) {
		loader_context->event(loader_context->data, 3, NULL);
	} else {
		va_list args;
		va_start(args, format);
		loader_context->event(loader_context->data, 4, va_arg(args, const char *));
		va_end(args);
	}
}
static lookup_t *loader_load(obs_module_t *module, const char *def, const char *locale);
#define obs_module_load_locale loader_load
#define obs_find_module_file loader_find
#define text_lookup_create loader_create
#define text_lookup_add loader_add
#define blog loader_log
#define obs_module_locale_default_file oracle_obs_module_locale_default_file
#define obs_module_locale_file oracle_obs_module_locale_file
#define obs_module_locale_is_default oracle_obs_module_locale_is_default
#include "obs-module-load-locale.inc"

bool oracle_module_load_locale(bool module, const char *def, const char *locale, loader_event_t event, void *data);
bool oracle_module_load_locale(bool module, const char *def, const char *locale, loader_event_t event, void *data)
{
	struct loader_context context = {event, data, {0}};
	struct loader_context *previous = loader_context;
	loader_context = &context;
	obs_module_t value = {"module.so"};
	bool result = loader_load(module ? &value : NULL, def, locale) != NULL;
	loader_context = previous;
	return result;
}
