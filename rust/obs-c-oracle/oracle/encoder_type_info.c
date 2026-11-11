/* Original extracted queries and real-header fixtures; no duplicate lookup. */
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#define obs_encoder_get_display_name oracle_obs_encoder_get_display_name
#define obs_get_encoder_codec oracle_obs_get_encoder_codec
#include "libobs/obs-encoder-type-info.c.h"
#undef obs_encoder_get_display_name
#undef obs_get_encoder_codec
#include "encoder_type_info_fields.inc"

struct obs_core *oracle_encoder_registry_exchange_core(struct obs_core *core);
struct eti_trace {
	const char *result;
	void *last;
	size_t calls;
};
#if defined(_MSC_VER)
static __declspec(thread) struct eti_trace *eti_active;
#else
static _Thread_local struct eti_trace *eti_active;
#endif
static const char *eti_name(void *data)
{
	eti_active->last = data;
	++eti_active->calls;
	return eti_active->result;
}
typedef const char *(*eti_query)(const char *id);

bool oracle_encoder_type_info_check(const char *const *ids, const char *const *codecs, size_t count, const char *query,
				    const char *const *results, size_t result_count, unsigned null_data, bool names,
				    eti_query name, eti_query codec);
bool oracle_encoder_type_info_check(const char *const *ids, const char *const *codecs, size_t count, const char *query,
				    const char *const *results, size_t result_count, unsigned null_data, bool names,
				    eti_query name, eti_query codec)
{
	if (count > 8 || result_count > 8)
		return false;
	struct obs_core core = {0}, before_core;
	struct obs_encoder_info entries[8] = {0}, before[8];
	unsigned tokens[8] = {0};
	struct eti_trace trace = {0};
	struct eti_trace *previous_trace = eti_active;
	eti_active = &trace;
	for (size_t i = 0; i < count; ++i) {
		entries[i].id = ids[i];
		entries[i].type = i % 2 ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
		entries[i].codec = codecs[i];
		entries[i].get_name = names ? eti_name : NULL;
		entries[i].type_data = null_data & (1u << i) ? NULL : &tokens[i];
		entries[i].caps = UINT32_MAX;
	}
	core.encoder_types.array = entries;
	core.encoder_types.num = count;
	core.encoder_types.capacity = 8;
	memcpy(before, entries, sizeof(entries));
	memcpy(&before_core, &core, sizeof(core));
	struct obs_core *previous = oracle_encoder_registry_exchange_core(&core);
	bool valid = true;
	for (size_t i = 0; i < count; ++i) {
		struct obs_encoder_type_info_fields fields = obs_encoder_type_info_fields(&entries[i]);
		valid = valid && fields.get_name == entries[i].get_name && fields.type_data == entries[i].type_data &&
			fields.codec == entries[i].codec;
	}
	for (size_t i = 0; i < result_count; ++i) {
		trace = (struct eti_trace){.result = results[i]};
		const char *rust_codec = codec(query);
		valid = valid && trace.calls == 0 && trace.last == NULL;
		const char *c_codec = oracle_obs_get_encoder_codec(query);
		valid = valid && rust_codec == c_codec && trace.calls == 0;
		if (names) {
			const char *rust_name = name(query);
			size_t rust_calls = trace.calls;
			void *rust_data = trace.last;
			trace.calls = 0;
			trace.last = NULL;
			const char *c_name = oracle_obs_encoder_get_display_name(query);
			valid = valid && rust_name == c_name && rust_calls == trace.calls && rust_data == trace.last;
			valid = valid && trace.calls <= 1;
		}
		valid = valid && memcmp(entries, before, sizeof(entries)) == 0 &&
			memcmp(&core, &before_core, sizeof(core)) == 0;
	}
	oracle_encoder_registry_exchange_core(previous);
	eti_active = previous_trace;
	return valid;
}

void oracle_encoder_type_info_layout(size_t *out);
void oracle_encoder_type_info_layout(size_t *out)
{
	out[0] = sizeof(struct obs_encoder_type_info_fields);
	out[1] = _Alignof(struct obs_encoder_type_info_fields);
	out[2] = offsetof(struct obs_encoder_type_info_fields, get_name);
	out[3] = offsetof(struct obs_encoder_type_info_fields, type_data);
	out[4] = offsetof(struct obs_encoder_type_info_fields, codec);
}

bool oracle_encoder_type_info_mutable(eti_query name, eti_query codec);
bool oracle_encoder_type_info_mutable(eti_query name, eti_query codec)
{
	/* unsigned storage avoids MSVC C4310 for arbitrary high bytes. */
	unsigned char text[] = {'n', 255, 0, 'x', 0};
	char video[] = " H264;;hevc;H264;unknown ";
	const char *ids[] = {"mutable"};
	const char *codecs[] = {video};
	const char *results[] = {(const char *)text, NULL, "", (const char *)text};
	bool valid = oracle_encoder_type_info_check(ids, codecs, 1, ids[0], results, 4, 0, true, name, codec);
	video[0] = 'x';
	text[0] = 'S';
	return valid && oracle_encoder_type_info_check(ids, codecs, 1, ids[0], results, 4, 0, true, name, codec);
}
