/* Real private layout and original C entry points; signals are recorded locally. */
#include "obs-internal.h"
#include <stdlib.h>
#include <string.h>

struct save_load_fixture {
	struct obs_source sources[9];
	struct obs_source *filters[8];
	uint64_t trace;
	unsigned calls;
	uint32_t bits;
	bool identity;
};
/* Per-thread fixture is not needed: each source carries its fixture in settings. */
static void sl_record(obs_source_t *source, unsigned kind)
{
	struct save_load_fixture *f = (void *)source->context.settings;
	f->trace = f->trace * 131 + kind * 16 + (unsigned)(source - f->sources) + 1;
	f->calls++;
}
static void sl_signal(obs_source_t *source, const char *global, const char *local)
{
	if (global && !source->context.private)
		sl_record(source, global[7] == 's' ? 1 : 4);
	if (local)
		sl_record(source, local[0] == 's' ? 2 : 5);
	struct save_load_fixture *f = (void *)source->context.settings;
	if (local && local[0] == 's' && (f->bits & (1u << 21)))
		source->info.save = NULL;
}
static void sl_save(void *data, obs_data_t *settings)
{
	obs_source_t *source = data;
	struct save_load_fixture *f = (void *)settings;
	f->identity = f->identity && source->context.data == data && source->context.settings == settings;
	sl_record(source, 3);
}
static void sl_load(void *data, obs_data_t *settings)
{
	obs_source_t *source = data;
	struct save_load_fixture *f = (void *)settings;
	f->identity = f->identity && source->context.data == data && source->context.settings == settings;
	sl_record(source, 6);
	/* The parent callback can reorder filters before load2 starts traversing. */
	if (source == f->sources && (f->bits & (1u << 22))) {
		for (size_t i = 0; i < source->filters.num / 2; i++) {
			size_t j = source->filters.num - i - 1;
			obs_source_t *tmp = source->filters.array[i];
			source->filters.array[i] = source->filters.array[j];
			source->filters.array[j] = tmp;
		}
	}
}
void oracle_obs_source_save(obs_source_t *source);
void oracle_obs_source_load(obs_source_t *source);
void oracle_obs_source_load2(obs_source_t *source);

#define obs_source_dosignal sl_signal
#define obs_source_save oracle_obs_source_save
#define obs_source_load oracle_obs_source_load
#define obs_source_load2 oracle_obs_source_load2
#include "obs-source-save-load.c.h"
#undef obs_source_load2
#undef obs_source_load
#undef obs_source_save
#include "source_save_load_fields.inc"
#undef obs_source_dosignal

typedef void (*sl_entry)(obs_source_t *);
uint64_t oracle_source_save_load_run(sl_entry entry, unsigned op, uint32_t bits, unsigned count);
uint64_t oracle_source_save_load_run(sl_entry entry, unsigned op, uint32_t bits, unsigned count)
{
	struct save_load_fixture f = {0};
	f.identity = true;
	f.bits = bits;
	count %= 9;
	for (unsigned i = 0; i <= count; i++) {
		obs_source_t *s = &f.sources[i];
		s->context.data = (bits & (1u << i)) ? s : NULL;
		s->context.settings = (void *)&f;
		s->context.private = (bits & (1u << (9 + i))) != 0;
		s->info.save = (bits & (1u << 18)) ? sl_save : NULL;
		s->info.load = (bits & (1u << 19)) ? sl_load : NULL;
		if (i)
			f.filters[i - 1] = s;
	}
	f.sources[0].filters.array = f.filters;
	f.sources[0].filters.num = count;
	obs_source_t *source = (bits & (1u << 20)) ? NULL : &f.sources[0];
	if (entry) {
		entry(source);
	} else if (op == 0) {
		oracle_obs_source_save(source);
	} else if (op == 1) {
		oracle_obs_source_load(source);
	} else {
		oracle_obs_source_load2(source);
	}
	return f.trace ^ ((uint64_t)f.calls << 48) ^ ((uint64_t)f.identity << 63);
}
