/* Unmodified original queries, real-header fixtures, and the production adapter. */
#include "obs-internal.h"
#include <stddef.h>
#include <string.h>

const char *oracle_obs_output_get_display_name(const char *id);
uint32_t oracle_obs_get_output_flags(const char *id);
const char *oracle_obs_get_output_supported_video_codecs(const char *id);
const char *oracle_obs_get_output_supported_audio_codecs(const char *id);

#define obs_output_get_display_name oracle_obs_output_get_display_name
#define obs_get_output_flags oracle_obs_get_output_flags
#define obs_get_output_supported_video_codecs oracle_obs_get_output_supported_video_codecs
#define obs_get_output_supported_audio_codecs oracle_obs_get_output_supported_audio_codecs
#include "libobs/obs-output-type-info.c.h"
#undef obs_output_get_display_name
#undef obs_get_output_flags
#undef obs_get_output_supported_video_codecs
#undef obs_get_output_supported_audio_codecs
#include "output_type_info_fields.inc"

struct obs_core *oracle_output_type_exchange_core(struct obs_core *core);
struct output_type_trace {
	size_t calls;
	void *data;
	const char *next;
};
#if defined(_MSC_VER)
static __declspec(thread) struct output_type_trace *output_type_trace;
#else
static _Thread_local struct output_type_trace *output_type_trace;
#endif
static const char *output_type_name(void *data)
{
	++output_type_trace->calls;
	output_type_trace->data = data;
	return output_type_trace->next;
}
typedef const char *(*output_type_text)(const char *);
typedef uint32_t (*output_type_flags)(const char *);
bool oracle_output_type_info_check(const char *const *ids, const uint32_t *flags, const char *const *videos,
				   const char *const *audios, size_t count, const char *query, unsigned mode,
				   output_type_text name, output_type_flags bits, output_type_text video,
				   output_type_text audio);
bool oracle_output_type_info_check(const char *const *ids, const uint32_t *flags, const char *const *videos,
				   const char *const *audios, size_t count, const char *query, unsigned mode,
				   output_type_text name, output_type_flags bits, output_type_text video,
				   output_type_text audio)
{
	if (count > 8 || ((mode & 1) && query))
		return false;
	struct obs_core core = {0};
	struct obs_core before_core;
	struct obs_output_info entries[8] = {0};
	struct obs_output_info before[8];
	for (size_t i = 0; i < count; ++i) {
		entries[i].id = ids[i];
		entries[i].flags = flags[i];
		entries[i].get_name = mode & 2 ? NULL : output_type_name;
		entries[i].type_data = i & 1 ? &entries[i] : NULL;
		entries[i].encoded_video_codecs = videos[i];
		entries[i].encoded_audio_codecs = audios[i];
	}
	core.output_types.array = entries;
	core.output_types.num = count;
	core.output_types.capacity = 8;
	memcpy(before, entries, sizeof(entries));
	memcpy(&before_core, &core, sizeof(core));
	struct obs_core *previous = oracle_output_type_exchange_core(mode & 1 ? NULL : &core);
	const struct obs_output_info *found = find_output(query);
	struct output_type_trace trace = {0};
	output_type_trace = &trace;
	bool valid = true;
	for (size_t repeat = 0; repeat < 3; ++repeat) {
		valid = valid && bits(query) == oracle_obs_get_output_flags(query);
		valid = valid && video(query) == oracle_obs_get_output_supported_video_codecs(query);
		valid = valid && audio(query) == oracle_obs_get_output_supported_audio_codecs(query);
		valid = valid && trace.calls == 0;
	}
	if (!(mode & 2)) {
		char first[] = "same display name";
		char second[] = "same display name";
		char empty[] = "";
		const char raw[] = "n\xff\0x";
		const char *results[] = {first, second, empty, raw, NULL, first};
		valid = valid && &first[0] != &second[0];
		for (size_t i = 0; i < 6; ++i) {
			trace.next = results[i];
			trace.calls = 0;
			trace.data = NULL;
			const char *c = oracle_obs_output_get_display_name(query);
			void *data = trace.data;
			size_t calls = trace.calls;
			trace.calls = 0;
			trace.data = NULL;
			const char *r = name(query);
			valid = valid && r == c && r == (found ? results[i] : NULL) && trace.calls == calls &&
				calls == (found ? 1u : 0u) && trace.data == data &&
				data == (found ? found->type_data : NULL);
		}
	}
	valid = valid && memcmp(before, entries, sizeof(entries)) == 0 &&
		memcmp(&before_core, &core, sizeof(core)) == 0;
	/* Check borrowed mutable backing storage independently of caller buffers. */
	if (found) {
		char mutable_video[] = " H264;;hevc;H264;unknown ";
		char mutable_audio[] = "aac;opus;;aac;";
		size_t index = (size_t)(found - entries);
		entries[index].encoded_video_codecs = mutable_video;
		entries[index].encoded_audio_codecs = mutable_audio;
		valid = valid && video(query) == mutable_video && audio(query) == mutable_audio;
		valid = valid && strcmp(video(query), " H264;;hevc;H264;unknown ") == 0 &&
			strcmp(audio(query), "aac;opus;;aac;") == 0;
		mutable_video[0] = 'x';
		mutable_audio[0] = 'y';
		valid = valid && video(query) == mutable_video && video(query)[0] == 'x' &&
			audio(query) == mutable_audio && audio(query)[0] == 'y';
	}
	output_type_trace = NULL;
	oracle_output_type_exchange_core(previous);
	return valid;
}
void oracle_output_type_info_layout(size_t *out);
void oracle_output_type_info_layout(size_t *out)
{
	out[0] = sizeof(struct obs_output_type_info_fields);
	out[1] = _Alignof(struct obs_output_type_info_fields);
	out[2] = offsetof(struct obs_output_type_info_fields, get_name);
	out[3] = offsetof(struct obs_output_type_info_fields, type_data);
	out[4] = offsetof(struct obs_output_type_info_fields, flags);
	out[5] = offsetof(struct obs_output_type_info_fields, video);
	out[6] = offsetof(struct obs_output_type_info_fields, audio);
}
