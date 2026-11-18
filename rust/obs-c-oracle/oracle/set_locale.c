/* Original C and the real private adapters over a thread-owned core. Tier 2
 * additionally exercises these same paths in the complete shared libraries. */
#include "libobs/build/libobs.pch.h"

static THREAD_LOCAL struct obs_core locale_core;
static THREAD_LOCAL struct obs_module locale_modules[7];
static THREAD_LOCAL const char *locale_argument;
static THREAD_LOCAL long locale_baseline;
static THREAD_LOCAL size_t locale_count;
static THREAD_LOCAL uint64_t locale_events[64];
static THREAD_LOCAL bool locale_mutate;

void oracle_obs_set_locale(const char *locale);
void oracle_set_locale_reset(const unsigned char *order, unsigned count, unsigned callbacks, bool mutate);
size_t oracle_set_locale_step(bool rust, char *locale, uint64_t *out);
void oracle_set_locale_finish(void);
void oracle_set_locale_layout(size_t *out);

#define obs (&locale_core)
#define obs_set_locale oracle_obs_set_locale
#include "obs-set-locale.inc"
#undef obs_set_locale
#include "set-locale-fields.inc"
#undef obs

static void locale_record(unsigned id, const char *locale)
{
	const char *owned = locale_core.locale;
	locale_events[locale_count++] = id;
	locale_events[locale_count++] = locale == locale_argument;
	locale_events[locale_count++] = locale ? owned && owned != locale && strcmp(owned, locale) == 0 : !owned;
	locale_events[locale_count++] = (uint64_t)(bnum_allocs() - locale_baseline);
	/* The C traversal reads next after dispatch, rather than snapshotting it. */
	if (locale_mutate && id < 6) {
		struct obs_module *next = locale_modules[id].next;
		if (next)
			locale_modules[id].next = next->next;
	}
}
#define LOCALE_CALLBACK(id) \
	static void locale_callback_##id(const char *locale) \
	{ \
		locale_record(id, locale); \
	}
LOCALE_CALLBACK(0)
LOCALE_CALLBACK(1)
LOCALE_CALLBACK(2)
LOCALE_CALLBACK(3)
LOCALE_CALLBACK(4)
LOCALE_CALLBACK(5)
LOCALE_CALLBACK(6)
#undef LOCALE_CALLBACK

void oracle_set_locale_reset(const unsigned char *order, unsigned count, unsigned callbacks, bool mutate)
{
	memset(&locale_core, 0, sizeof(locale_core));
	memset(locale_modules, 0, sizeof(locale_modules));
	locale_baseline = bnum_allocs();
	locale_core.locale = bstrdup("old");
	void (*handlers[])(const char *) = {locale_callback_0, locale_callback_1, locale_callback_2, locale_callback_3,
					    locale_callback_4, locale_callback_5, locale_callback_6};
	for (unsigned i = 0; i < 7; ++i) {
		locale_modules[i].set_locale = callbacks & (1u << i) ? handlers[i] : NULL;
		locale_modules[i].loaded = false;
		locale_modules[i].load_state = OBS_MODULE_DISABLED;
	}
	for (unsigned i = 0; i < count; ++i)
		locale_modules[order[i]].next = i + 1 < count ? &locale_modules[order[i + 1]] : NULL;
	locale_core.first_module = count ? &locale_modules[order[0]] : NULL;
	locale_core.first_disabled_module = &locale_modules[6];
	locale_mutate = mutate;
}

size_t oracle_set_locale_step(bool rust, char *locale, uint64_t *out)
{
	locale_argument = locale;
	locale_count = 0;
	if (rust)
		obs_set_locale(locale);
	else
		oracle_obs_set_locale(locale);
	size_t n = 0;
	out[n++] = locale_core.locale != NULL;
	out[n++] = (uint64_t)(bnum_allocs() - locale_baseline);
	out[n++] = locale_count;
	for (size_t i = 0; i < locale_count; ++i)
		out[n++] = locale_events[i];
	/* Mutating caller storage after dispatch must not affect the owned copy. */
	if (locale) {
		char *input = locale;
		char saved = input[0];
		input[0] = saved ? 'x' : '\0';
		for (const unsigned char *p = (const unsigned char *)locale_core.locale; *p; ++p)
			out[n++] = *p;
		input[0] = saved;
	}
	out[n++] = locale_core.first_module ? (uint64_t)(locale_core.first_module - locale_modules) + 1 : 0;
	out[n++] = locale_core.first_disabled_module == &locale_modules[6];
	for (unsigned i = 0; i < 7; ++i) {
		out[n++] = locale_modules[i].next ? (uint64_t)(locale_modules[i].next - locale_modules) + 1 : 0;
		out[n++] = locale_modules[i].loaded;
		out[n++] = (uint64_t)locale_modules[i].load_state;
	}
	return n;
}

void oracle_set_locale_finish(void)
{
	bfree(locale_core.locale);
	locale_core.locale = NULL;
}

void oracle_set_locale_layout(size_t *out)
{
	out[0] = sizeof(locale_modules[0].set_locale);
	out[1] = _Alignof(void (*)(const char *));
}
