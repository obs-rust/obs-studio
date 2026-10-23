/* Minimal OBS module standing in for a core module in test_core_module_load. */
#include <obs-module.h>

OBS_DECLARE_MODULE()

#ifdef CORE_STUB_NEEDS_DEP
/* Defined in a shared library that is not installed next to the module, like
 * libcef.so for an obs-browser.so packaged without CEF. */
extern int core_stub_missing_dependency(void);
#endif

#ifdef CORE_STUB_REGISTERS
/* Registers one type of each kind that libobs accepts, and types that fail a
 * check of obs_register_*_s(), while this module is the loading module. The
 * test asks which of them the module's type lists name. */
static const char *stub_name(void *type_data)
{
	(void)type_data;
	return "stub";
}

#define STUB_CREATE(kind, owner_type)                                     \
	static void *stub_create_##kind(obs_data_t *settings, owner_type *owner) \
	{                                                                    \
		(void)settings;                                              \
		(void)owner;                                                 \
		return NULL;                                                 \
	}
STUB_CREATE(source, obs_source_t)
STUB_CREATE(output, obs_output_t)
STUB_CREATE(encoder, obs_encoder_t)
STUB_CREATE(service, obs_service_t)

static void stub_destroy(void *data)
{
	(void)data;
}

static uint32_t stub_size(void *data)
{
	(void)data;
	return 16;
}

static bool stub_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio_output, uint32_t mixers,
			      size_t channels, size_t sample_rate)
{
	(void)data;
	(void)ts_out;
	(void)audio_output;
	(void)mixers;
	(void)channels;
	(void)sample_rate;
	return false;
}

static bool stub_start(void *data)
{
	(void)data;
	return false;
}

static void stub_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

static bool stub_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received_packet)
{
	(void)data;
	(void)frame;
	(void)packet;
	(void)received_packet;
	return false;
}

static const char *stub_protocol(void *data)
{
	(void)data;
	return "STUB";
}

static void register_source(const char *id, uint32_t flags, bool name, bool size)
{
	struct obs_source_info info = {0};
	info.id = id;
	info.type = OBS_SOURCE_TYPE_INPUT;
	info.output_flags = flags;
	info.get_name = name ? stub_name : NULL;
	info.create = stub_create_source;
	info.destroy = stub_destroy;
	info.get_width = size ? stub_size : NULL;
	info.get_height = size ? stub_size : NULL;
	info.audio_render = stub_audio_render;
	obs_register_source(&info);
}

static void register_output(const char *id, bool start)
{
	struct obs_output_info info = {0};
	info.id = id;
	info.get_name = stub_name;
	info.create = stub_create_output;
	info.destroy = stub_destroy;
	info.start = start ? stub_start : NULL;
	info.stop = stub_stop;
	obs_register_output(&info);
}

static void register_encoder(const char *id, bool encode)
{
	struct obs_encoder_info info = {0};
	info.id = id;
	info.type = OBS_ENCODER_VIDEO;
	info.codec = "stub";
	info.get_name = stub_name;
	info.create = stub_create_encoder;
	info.destroy = stub_destroy;
	info.encode = encode ? stub_encode : NULL;
	obs_register_encoder(&info);
}

static void register_service(const char *id, bool protocol)
{
	struct obs_service_info info = {0};
	info.id = id;
	info.get_name = stub_name;
	info.create = stub_create_service;
	info.destroy = stub_destroy;
	info.get_protocol = protocol ? stub_protocol : NULL;
	obs_register_service(&info);
}
#endif

bool obs_module_load(void)
{
#ifdef CORE_STUB_NEEDS_DEP
	return core_stub_missing_dependency() == 1;
#else
#ifdef CORE_STUB_REGISTERS
	register_source("regstub_source", OBS_SOURCE_VIDEO, true, true);
	/* Fail the composite rule, the get_name rule and the get_width rule. */
	register_source("regstub_composite_audio", OBS_SOURCE_COMPOSITE | OBS_SOURCE_AUDIO, true, true);
	register_source("regstub_no_name", 0, false, true);
	register_source("regstub_no_width", OBS_SOURCE_VIDEO, true, false);
	register_output("regstub_output", true);
	register_output("regstub_output_no_start", false);
	register_encoder("regstub_encoder", true);
	register_encoder("regstub_encoder_no_encode", false);
	register_service("regstub_service", true);
	register_service("regstub_service_no_protocol", false);
#endif
	return true;
#endif
}
