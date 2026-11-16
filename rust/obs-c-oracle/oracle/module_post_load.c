/* Original extracted C and the production field adapter over real private
 * records. The core-category test re-execs before touching the host probe. */
#include "libobs/build/libobs.pch.h"

static struct obs_core module_post_load_core;
static struct obs_module module_post_load_modules[9];
static uint64_t *module_post_load_report;
static size_t module_post_load_events;
static unsigned module_post_load_mutations;
static unsigned module_post_load_host;
static unsigned module_post_load_enabled;
static unsigned module_post_load_order[8];

void oracle_module_lists_set_core(struct obs_core *core);
void oracle_obs_post_load_modules(void);
void oracle_module_post_load_run(void (*entry)(void), const unsigned *order, unsigned enabled, unsigned callbacks,
				 unsigned loaded, const int *states, unsigned mutations, unsigned host,
				 unsigned repeats, uint64_t *report);
void oracle_module_post_load_layout(size_t *out);

static void module_post_load_event(unsigned id)
{
	if (module_post_load_events >= 64)
		abort();
	module_post_load_report[16 + module_post_load_events++] = id;
}

static void module_post_load_callback(unsigned id)
{
	struct obs_module *module = &module_post_load_modules[id];
	/* Observe prior callbacks' state, not just the final state. */
	unsigned flags = 0;
	for (unsigned i = 0; i < 9; ++i)
		flags |= module_post_load_modules[i].loaded ? 1u << i : 0;
	module_post_load_event(1u + id + (flags << 8));
	if (module_post_load_mutations & 1u)
		module->loaded = !module->loaded;
	if ((module_post_load_mutations & 2u) && module->next)
		module->next->loaded = !module->next->loaded;
	if ((module_post_load_mutations & 4u) && module->next)
		module->next = module->next->next;
	if ((module_post_load_mutations & 8u) && module->next)
		module->next->post_load = NULL;
}

static void module_post_load_0(void)
{
	module_post_load_callback(0);
}
static void module_post_load_1(void)
{
	module_post_load_callback(1);
}
static void module_post_load_2(void)
{
	module_post_load_callback(2);
}
static void module_post_load_3(void)
{
	module_post_load_callback(3);
}
static void module_post_load_4(void)
{
	module_post_load_callback(4);
}
static void module_post_load_5(void)
{
	module_post_load_callback(5);
}
static void module_post_load_6(void)
{
	module_post_load_callback(6);
}
static void module_post_load_7(void)
{
	module_post_load_callback(7);
}
static void module_post_load_excluded(void)
{
	module_post_load_event(0xffff);
}

/* Host hook is the only external service replaced. Both implementations call
 * this same probe; its list mutations prove the first read follows the hook. */
void obs_host_post_load_modules(void)
{
	module_post_load_event(0);
	if (module_post_load_host == 1)
		module_post_load_core.first_module = NULL;
	else if (module_post_load_host == 2 && module_post_load_enabled)
		module_post_load_core.first_module =
			&module_post_load_modules[module_post_load_order[module_post_load_enabled - 1]];
}

#define obs (&module_post_load_core)
#define obs_post_load_modules oracle_obs_post_load_modules
#include "libobs/obs-module-post-load.c.h"
#undef obs_post_load_modules
#undef obs
#include "module-post-load-fields.inc"

static size_t module_post_load_identity(const struct obs_module *module)
{
	if (!module)
		return 0;
	for (size_t i = 0; i < 9; ++i)
		if (module == &module_post_load_modules[i])
			return i + 1;
	return 99;
}

void oracle_module_post_load_run(void (*entry)(void), const unsigned *order, unsigned enabled, unsigned callbacks,
				 unsigned loaded, const int *states, unsigned mutations, unsigned host,
				 unsigned repeats, uint64_t *report)
{
	void (*hooks[8])(void) = {module_post_load_0, module_post_load_1, module_post_load_2, module_post_load_3,
				  module_post_load_4, module_post_load_5, module_post_load_6, module_post_load_7};
	if (enabled > 8 || repeats > 3 || host > 2)
		abort();
	memset(&module_post_load_core, 0, sizeof(module_post_load_core));
	memset(module_post_load_modules, 0, sizeof(module_post_load_modules));
	memset(report, 0, 128 * sizeof(*report));
	module_post_load_report = report;
	module_post_load_events = 0;
	module_post_load_mutations = mutations;
	module_post_load_host = host;
	module_post_load_enabled = enabled;
	unsigned seen = 0;
	for (unsigned i = 0; i < 8; ++i) {
		if (order[i] >= 8 || (seen & (1u << order[i])))
			abort();
		seen |= 1u << order[i];
		module_post_load_order[i] = order[i];
		struct obs_module *module = &module_post_load_modules[order[i]];
		module->next = i + 1 < 8 && i + 1 != enabled ? &module_post_load_modules[order[i + 1]] : NULL;
		module->post_load = callbacks & (1u << order[i]) ? hooks[order[i]] : NULL;
		module->loaded = (loaded & (1u << order[i])) != 0;
		module->load_state = (enum obs_module_load_state)states[order[i]];
	}
	module_post_load_modules[8].post_load = module_post_load_excluded;
	module_post_load_core.first_module = enabled ? &module_post_load_modules[order[0]] : NULL;
	module_post_load_core.first_disabled_module = enabled < 8 ? &module_post_load_modules[order[enabled]] : NULL;
	struct obs_module before[9];
	memcpy(before, module_post_load_modules, sizeof(before));
	oracle_module_lists_set_core(&module_post_load_core);
	for (unsigned i = 0; i < repeats; ++i)
		entry();
	report[0] = module_post_load_events;
	report[1] = module_post_load_identity(module_post_load_core.first_module);
	report[2] = module_post_load_identity(module_post_load_core.first_disabled_module);
	for (size_t i = 0; i < 9; ++i) {
		struct obs_module *module = &module_post_load_modules[i];
		report[80 + i * 4] = module->loaded;
		report[81 + i * 4] = module->post_load != NULL;
		report[82 + i * 4] = (uint64_t)module->load_state;
		report[83 + i * 4] = module_post_load_identity(module->next);
		/* All other bytes, including detached/disabled record storage, must
		 * remain untouched. Reset only fields the callbacks can mutate. */
		module->loaded = before[i].loaded;
		module->post_load = before[i].post_load;
		module->next = before[i].next;
	}
	report[3] = memcmp(before, module_post_load_modules, sizeof(before)) == 0;
	oracle_module_lists_set_core(NULL);
	module_post_load_report = NULL;
}

void oracle_module_post_load_layout(size_t *out)
{
	out[0] = sizeof(module_post_load_modules[0].post_load);
	out[1] = _Alignof(void (*)(void));
	module_post_load_modules[0].post_load = module_post_load_0;
	out[2] = obs_module_post_load_callback(&module_post_load_modules[0]) == module_post_load_0;
	module_post_load_modules[0].post_load = NULL;
	out[3] = obs_module_post_load_callback(&module_post_load_modules[0]) == NULL;
}
