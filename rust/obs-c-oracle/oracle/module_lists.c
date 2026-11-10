/* Test-only oracle: the original libobs/obs-module-lists.c.h, unmodified,
 * with its ten calls renamed to oracle_* so they can link next to the Rust
 * shims (rust/obs-core/src/ffi/module_lists.rs).
 *
 * The Rust shims reach the core and its modules through the module list
 * seam, libobs/obs-module-lists-seam.c.h, compiled here under its own names
 * over the same stand-in core, as it stays C in libobs.core. libobs's global
 * core pointer is a thread-local of this file, so parity tests on different
 * threads each have their own core (or none).
 *
 * blog() is renamed to a recording stand-in, which the parity test hands
 * the Rust shim too (its Calls): each line appends its level and its text,
 * "%s" filled in ("(null)" for NULL), to a thread-local log.
 *
 * obs_module_paths_find_file() and obs_module_names_config_path() keep their
 * names: both sides run libobs's own, the Rust cores of module_paths and
 * module_names. */

#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

void oracle_obs_log_loaded_modules(void);
void oracle_obs_module_add_source(obs_module_t *module, const char *id);
void oracle_obs_module_add_output(obs_module_t *module, const char *id);
void oracle_obs_module_add_encoder(obs_module_t *module, const char *id);
void oracle_obs_module_add_service(obs_module_t *module, const char *id);
char *oracle_obs_find_module_file(obs_module_t *module, const char *file);
char *oracle_obs_module_get_config_path(obs_module_t *module, const char *file);
void oracle_obs_add_module_path(const char *bin, const char *data);
void oracle_obs_add_safe_module(const char *name);
void oracle_obs_add_disabled_module(const char *name);

void oracle_module_lists_blog(int log_level, const char *format, ...);

#if defined(_MSC_VER)
static __declspec(thread) struct obs_core *module_lists_core;
#else
static _Thread_local struct obs_core *module_lists_core;
#endif

#define obs module_lists_core
#define blog oracle_module_lists_blog
#define obs_log_loaded_modules oracle_obs_log_loaded_modules
#define obs_module_add_source oracle_obs_module_add_source
#define obs_module_add_output oracle_obs_module_add_output
#define obs_module_add_encoder oracle_obs_module_add_encoder
#define obs_module_add_service oracle_obs_module_add_service
#define obs_find_module_file oracle_obs_find_module_file
#define obs_module_get_config_path oracle_obs_module_get_config_path
#define obs_add_module_path oracle_obs_add_module_path
#define obs_add_safe_module oracle_obs_add_safe_module
#define obs_add_disabled_module oracle_obs_add_disabled_module

#include "libobs/obs-module-lists.c.h"

void oracle_obs_enum_modules(obs_enum_module_callback_t callback, void *param);
#define obs_enum_modules oracle_obs_enum_modules
#include "libobs/obs-enum-modules.c.h"
#undef obs_enum_modules

/* Discovery traversal uses the original C with a callback-only path finder.
 * The fixture observes forwarding without replacing the traversal or layouts. */
typedef void (*oracle_find_modules_probe_t)(const char *, const char *, enum obs_runtime_module_type,
                                          const char *, obs_module_add_extension_t,
                                          obs_find_module_callback2_t, void *);
#if defined(_MSC_VER)
static __declspec(thread) oracle_find_modules_probe_t find_modules_probe;
#else
static _Thread_local oracle_find_modules_probe_t find_modules_probe;
#endif
void oracle_find_modules_set_probe(oracle_find_modules_probe_t probe);
void oracle_find_modules_set_probe(oracle_find_modules_probe_t probe)
{
    find_modules_probe = probe;
}
void oracle_find_modules_find(const char *bin, const char *data, enum obs_runtime_module_type type,
                             const char *ext, obs_module_add_extension_t hook,
                             obs_find_module_callback2_t callback, void *param);
void oracle_find_modules_find(const char *bin, const char *data, enum obs_runtime_module_type type,
                             const char *ext, obs_module_add_extension_t hook,
                             obs_find_module_callback2_t callback, void *param)
{
    find_modules_probe(bin, data, type, ext, hook, callback, param);
}
const char *oracle_find_modules_extension(void);
const char *oracle_find_modules_extension(void)
{
    return ".probe";
}
void oracle_find_modules_hook(struct dstr *path);
void oracle_find_modules_hook(struct dstr *path)
{
    (void)path;
}
void oracle_obs_find_modules2(obs_find_module_callback2_t callback, void *param);
void oracle_obs_find_modules(obs_find_module_callback_t callback, void *param);
#define obs_find_modules2 oracle_obs_find_modules2
#define obs_find_modules oracle_obs_find_modules
#define obs_module_paths_find oracle_find_modules_find
#define get_module_extension oracle_find_modules_extension
#define obs_host_add_module_extension oracle_find_modules_hook
#include "libobs/obs-find-modules.c.h"
#undef obs_host_add_module_extension
#undef get_module_extension
#undef obs_module_paths_find
#undef obs_find_modules
#undef obs_find_modules2

#undef obs_add_disabled_module
#undef obs_add_safe_module
#undef obs_add_module_path
#undef obs_module_get_config_path
#undef obs_find_module_file
#undef obs_module_add_service
#undef obs_module_add_encoder
#undef obs_module_add_output
#undef obs_module_add_source
#undef obs_log_loaded_modules
#undef blog

#include "libobs/obs-module-lists-seam.c.h"

#undef obs

/* ------------------------------------------------------------------ log */

#define MODULE_LISTS_LOG 65536

#if defined(_MSC_VER)
static __declspec(thread) char module_lists_log[MODULE_LISTS_LOG];
static __declspec(thread) size_t module_lists_log_len;
#else
static _Thread_local char module_lists_log[MODULE_LISTS_LOG];
static _Thread_local size_t module_lists_log_len;
#endif

static void log_bytes(const char *text, size_t n)
{
	if (module_lists_log_len + n < MODULE_LISTS_LOG) {
		memcpy(module_lists_log + module_lists_log_len, text, n);
		module_lists_log_len += n;
		module_lists_log[module_lists_log_len] = 0;
	}
}

static void log_str(const char *text)
{
	log_bytes(text, strlen(text));
}

/* No <stdio.h>: on MSVC it makes this object define snprintf(), and another
 * test binary that links this library then pulls the object in for it. */
static void log_hex(uint64_t value)
{
	char text[17];
	for (int i = 0; i < 16; i++)
		text[i] = "0123456789abcdef"[(value >> (60 - 4 * i)) & 0xf];
	text[16] = 0;
	log_str(text);
}

void oracle_module_lists_blog(int log_level, const char *format, ...)
{
	va_list args;

	va_start(args, format);
	log_str("blog ");
	log_hex((uint64_t)(int64_t)log_level);
	log_str(" ");
	for (const char *p = format; *p; p++) {
		if (p[0] == '%' && p[1] == 's') {
			const char *text = va_arg(args, const char *);
			log_str(text ? text : "(null)");
			p++;
		} else if (p[0] == '%') {
			log_str("<unknown conversion>");
			break;
		} else {
			log_bytes(p, 1);
		}
	}
	va_end(args);
	log_str(";");
}

size_t oracle_module_lists_take_log(char *out, size_t cap);
size_t oracle_module_lists_take_log(char *out, size_t cap)
{
	size_t n = module_lists_log_len < cap ? module_lists_log_len : cap;
	memcpy(out, module_lists_log, n);
	module_lists_log_len = 0;
	module_lists_log[0] = 0;
	return n;
}

/* ----------------------------------------------------------- the core */

/* A zeroed core with its module config path (NULL for none), made the
 * thread's core. */
struct obs_core *oracle_module_lists_core_new(const char *config_path);
struct obs_core *oracle_module_lists_core_new(const char *config_path)
{
	struct obs_core *core = bzalloc(sizeof(struct obs_core));
	core->module_config_path = bstrdup(config_path);
	return core;
}

/* Makes `core` (NULL for none) the thread's core. */
void oracle_module_lists_set_core(struct obs_core *core);
void oracle_module_lists_set_core(struct obs_core *core)
{
	module_lists_core = core;
}

/* A zeroed module with copies of its strings. A loaded one goes at the
 * front of the core's list, as obs_init_module() puts it; another is on no
 * list, as one from obs_create_disabled_module() is on the core's list of
 * loaded ones. */
obs_module_t *oracle_module_lists_module_new(struct obs_core *core, const char *file, const char *data_path,
					     const char *name, bool loaded);
obs_module_t *oracle_module_lists_module_new(struct obs_core *core, const char *file, const char *data_path,
					     const char *name, bool loaded)
{
	struct obs_module *module = bzalloc(sizeof(struct obs_module));
	/* As obs_open_module() sets it: the file is the binary path. */
	module->bin_path = bstrdup(file);
	module->file = module->bin_path;
	module->data_path = bstrdup(data_path);
	module->mod_name = bstrdup(name);
	if (loaded) {
		module->next = core->first_module;
		core->first_module = module;
	}
	return module;
}

static void free_strings(char **items, size_t num)
{
	for (size_t i = 0; i < num; i++)
		bfree(items[i]);
}

void oracle_module_lists_module_free(obs_module_t *module);
void oracle_module_lists_module_free(obs_module_t *module)
{
	free_strings(module->sources.array, module->sources.num);
	free_strings(module->outputs.array, module->outputs.num);
	free_strings(module->encoders.array, module->encoders.num);
	free_strings(module->services.array, module->services.num);
	da_free(module->sources);
	da_free(module->outputs);
	da_free(module->encoders);
	da_free(module->services);
	bfree(module->bin_path);
	bfree(module->data_path);
	bfree(module->mod_name);
	bfree(module);
}

/* Frees the core and its lists, not its modules. */
void oracle_module_lists_core_free(struct obs_core *core);
void oracle_module_lists_core_free(struct obs_core *core)
{
	for (size_t i = 0; i < core->module_paths.num; i++)
		free_module_path(core->module_paths.array + i);
	da_free(core->module_paths);
	free_strings(core->safe_modules.array, core->safe_modules.num);
	free_strings(core->disabled_modules.array, core->disabled_modules.num);
	da_free(core->safe_modules);
	da_free(core->disabled_modules);
	bfree(core->module_config_path);
	bfree(core);
}

/* ---------------------------------------------------------- the state */

struct state {
	char *out;
	size_t cap;
	size_t len;
};

static void put_bytes(struct state *s, const char *text, size_t n)
{
	if (s->len + n < s->cap) {
		memcpy(s->out + s->len, text, n);
		s->len += n;
		s->out[s->len] = 0;
	}
}

static void put_str(struct state *s, const char *text)
{
	put_bytes(s, text, strlen(text));
}

static void put_hex(struct state *s, uint64_t value)
{
	char text[17];
	for (int i = 0; i < 16; i++)
		text[i] = "0123456789abcdef"[(value >> (60 - 4 * i)) & 0xf];
	text[16] = 0;
	put_str(s, text);
}

/* A string as its length and bytes, or "N" for NULL. */
static void put_string(struct state *s, const char *text)
{
	if (!text) {
		put_str(s, "N,");
		return;
	}
	put_hex(s, strlen(text));
	put_str(s, ":");
	put_str(s, text);
	put_str(s, ",");
}

static void put_list(struct state *s, const char *label, char **items, size_t num, size_t capacity)
{
	put_str(s, label);
	put_str(s, "[");
	put_hex(s, num);
	put_str(s, "/");
	put_hex(s, capacity);
	put_str(s, "]");
	for (size_t i = 0; i < num; i++)
		put_string(s, items[i]);
	put_str(s, ";");
}

/* Every list of the core and of `modules`, with each one's size and
 * capacity, written to `out`. */
size_t oracle_module_lists_state(const struct obs_core *core, obs_module_t *const *modules, size_t count, char *out,
				 size_t cap);
size_t oracle_module_lists_state(const struct obs_core *core, obs_module_t *const *modules, size_t count, char *out,
				 size_t cap)
{
	struct state s = {out, cap, 0};
	if (cap)
		out[0] = 0;

	put_str(&s, "paths[");
	put_hex(&s, core->module_paths.num);
	put_str(&s, "/");
	put_hex(&s, core->module_paths.capacity);
	put_str(&s, "]");
	for (size_t i = 0; i < core->module_paths.num; i++) {
		const struct obs_module_path *omp = core->module_paths.array + i;
		put_string(&s, omp->bin);
		put_string(&s, omp->data);
		put_hex(&s, (uint64_t)omp->type);
		put_str(&s, ",");
	}
	put_str(&s, ";");
	put_list(&s, "safe", core->safe_modules.array, core->safe_modules.num, core->safe_modules.capacity);
	put_list(&s, "disabled", core->disabled_modules.array, core->disabled_modules.num,
		 core->disabled_modules.capacity);
	for (size_t i = 0; i < count; i++) {
		const struct obs_module *m = modules[i];
		put_list(&s, "sources", m->sources.array, m->sources.num, m->sources.capacity);
		put_list(&s, "outputs", m->outputs.array, m->outputs.num, m->outputs.capacity);
		put_list(&s, "encoders", m->encoders.array, m->encoders.num, m->encoders.capacity);
		put_list(&s, "services", m->services.array, m->services.num, m->services.capacity);
	}
	return s.len;
}

void oracle_module_lists_layout(size_t *layout);
void oracle_module_lists_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_module_path);
	layout[1] = _Alignof(struct obs_module_path);
	layout[2] = offsetof(struct obs_module_path, bin);
	layout[3] = offsetof(struct obs_module_path, data);
	layout[4] = offsetof(struct obs_module_path, type);
	layout[5] = sizeof(enum obs_runtime_module_type);
	layout[6] = (size_t)OBS_MODULE_TYPE_UNDEFINED;
	layout[7] = (size_t)LOG_INFO;
}

/* Test-only mutation of a live node; no private layout crosses into Rust. */
void oracle_enum_modules_set_next(obs_module_t *module, obs_module_t *next);
void oracle_enum_modules_set_next(obs_module_t *module, obs_module_t *next)
{
	module->next = next;
}
