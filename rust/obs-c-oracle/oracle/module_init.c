/* Verbatim initializer and live adapters, measured against real private headers.
 * Only external lifecycle services are observed; neither implementation calls
 * the other. The Rust category test re-execs before touching global core state. */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "obs-internal.h"
#include "util/profiler.h"

bool oracle_obs_init_module(obs_module_t *module);
extern obs_module_t *loadingModule;
extern void oracle_core_initialized_set(void *core);
void oracle_module_init_run(bool rust, bool present, bool core, bool loaded, unsigned results, unsigned count,
			    unsigned mutations, uint64_t *report);
void oracle_module_init_layout(size_t *layout);

static struct obs_core *module_init_core;
static struct obs_module *module_init_current;
static obs_module_t **module_init_loading;
static uint64_t *module_init_report;
static unsigned module_init_index;
static unsigned module_init_results;
static unsigned module_init_calls;
static unsigned module_init_mutations;
static char module_init_profile[256];
static int module_init_store;
static void module_init_event(unsigned event)
{
	module_init_report[module_init_index++] = event;
}
static bool module_init_callback(void)
{
	module_init_event(10 + (*module_init_loading == module_init_current ? 1u : 0u) +
			  (module_init_current->loaded ? 2u : 0u));
	return ((module_init_results >> module_init_calls++) & 1u) != 0;
}
static profiler_name_store_t *module_init_name_store(void)
{
	module_init_event(1);
	return (profiler_name_store_t *)&module_init_store;
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static const char *module_init_store_name(profiler_name_store_t *store, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	vsnprintf(module_init_profile, sizeof(module_init_profile), format, args);
	va_end(args);
	module_init_event(store == (profiler_name_store_t *)&module_init_store &&
					  strcmp(format, "obs_init_module(%s)") == 0 &&
					  strcmp(module_init_profile, "obs_init_module(plugin-\xC3\xA9.so)") == 0
				  ? 2
				  : 99);
	return module_init_profile;
}
static void module_init_start(const char *name)
{
	module_init_event(name == module_init_profile ? 3 : 99);
}
static void module_init_end(const char *name)
{
	module_init_event(name == module_init_profile && !*module_init_loading ? 5 : 99);
	if (module_init_mutations & 2u)
		module_init_current->loaded = !module_init_current->loaded;
}
#ifdef __GNUC__
__attribute__((format(printf, 2, 3)))
#endif
static void module_init_blog(int level, const char *format, ...)
{
	char text[256];
	va_list args;
	va_start(args, format);
	vsnprintf(text, sizeof(text), format, args);
	va_end(args);
	module_init_event(level == LOG_WARNING && !*module_init_loading && !module_init_current->loaded &&
					  strcmp(format, "Failed to initialize module '%s'") == 0 &&
					  strcmp(text, "Failed to initialize module 'plugin-\xC3\xA9.so'") == 0
				  ? 4
				  : 99);
	if (module_init_mutations & 1u)
		module_init_current->loaded = true;
}
#define obs module_init_core
#define loadingModule module_init_oracle_loading
#define obs_init_module oracle_obs_init_module
#define obs_get_profiler_name_store module_init_name_store
#define profile_store_name module_init_store_name
#define profile_start module_init_start
#define profile_end module_init_end
#define blog module_init_blog
#include "module-init.inc"
#undef obs_init_module
#undef loadingModule
#undef obs
#include "module-init-fields.inc"
#undef obs_get_profiler_name_store
#undef profile_store_name
#undef profile_start
#undef profile_end
#undef blog

void oracle_module_init_run(bool rust, bool present, bool core, bool loaded, unsigned results, unsigned count,
			    unsigned mutations, uint64_t *report)
{
	struct obs_module module;
	memset(&module, 0, sizeof(module));
	struct obs_module sentinel = {0};
	module.loaded = loaded;
	module.load = module_init_callback;
	module.file = "plugin-\xC3\xA9.so";
	unsigned char original[sizeof(module)];
	memcpy(original, &module, sizeof(module));
	module_init_core = core ? (struct obs_core *)&module_init_store : NULL;
	oracle_core_initialized_set(module_init_core);
	module_init_current = &module;
	module_init_loading = rust ? &loadingModule : &module_init_oracle_loading;
	*module_init_loading = &sentinel;
	module_init_report = report;
	module_init_index = 16;
	module_init_results = results;
	module_init_calls = 0;
	module_init_mutations = mutations;
	memset(report, 0, 128 * sizeof(*report));
	for (unsigned i = 0; i < count; i++) {
		bool ok = rust ? obs_init_module(present ? &module : NULL)
			       : oracle_obs_init_module(present ? &module : NULL);
		report[i] = (ok ? 1u : 0u) + (module.loaded ? 2u : 0u) + (*module_init_loading == &sentinel ? 4u : 0u) +
			    (*module_init_loading == &module ? 8u : 0u);
	}
	report[8] = module_init_calls;
	report[9] = module_init_index;
	module.loaded = loaded;
	report[10] = memcmp(original, &module, sizeof(module)) == 0 ? 0 : 99;
	*module_init_loading = NULL;
	oracle_core_initialized_set(NULL);
}
void oracle_module_init_layout(size_t *layout)
{
	struct obs_module module = {0};
	layout[0] = sizeof(module.loaded);
	layout[1] = _Alignof(bool);
	layout[2] = (size_t)((char *)obs_module_init_loaded_field(&module) - (char *)&module);
	layout[3] = offsetof(struct obs_module, loaded);
	layout[4] = sizeof(module.load);
	layout[5] = _Alignof(bool (*)(void));
}
