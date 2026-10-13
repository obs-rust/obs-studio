/* Hybrid MP4 recordings with more than two audio channels play silent in
 * VLC 3 (OBS forum thread 196049). For any track with more than two
 * channels the native MP4 muxer wrote a version 1 stsd box and version 1
 * AudioSampleEntry (ISO/IEC 14496-12:2022). Readers written before that
 * edition, VLC 3.0 among them, take a version 1 sound sample entry for a
 * QuickTime SoundDescriptionV1 and skip 16 more bytes before the child
 * boxes, so they lose the esds (dOps, dfLa, ...) box and with it the
 * decoder configuration. VLC 3's AAC decoder then outputs nothing. The same
 * audio remuxed by FFmpeg (version 0 entry) plays.
 *
 * The test builds obs-outputs' mp4-mux.c into the binary and muxes a few
 * packets from stub encoders (fixed codec configuration, no real encoding)
 * into a file, then reads the audio sample entry back the way such a reader
 * does. obs_startup() needs a display on Linux, so the cases skip without
 * one (run under xvfb-run). */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <obs-module.h>
#include <util/base.h>
#include <util/bmem.h>
#include <util/file-serializer.h>
#include <util/platform.h>

#include "mp4-mux.h"

#define OUTPUT_ID "mp4_mux_multichannel_output"

static bool obs_up;

/* mp4-mux.c names its start chapter through the module's locale. */
const char *obs_module_text(const char *val)
{
	return val;
}

static void quiet_log(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (lvl <= LOG_WARNING) {
		vfprintf(stderr, msg, args);
		fputc('\n', stderr);
	}
}

/* ------------------------------------------------------------------------ */
/* Stub encoders: report a codec and its configuration, never encode.       */

struct stub_codec {
	const char *id;
	const char *codec;
	const char *config_box; /* child box that carries the configuration */
	size_t frame_size;
};

static const struct stub_codec stub_aac = {"mp4mc_aac", "aac", "esds", 1024};
static const struct stub_codec stub_opus = {"mp4mc_opus", "opus", "dOps", 960};
static const struct stub_codec stub_flac = {"mp4mc_flac", "flac", "dfLa", 1024};
static const struct stub_codec stub_alac = {"mp4mc_alac", "alac", "alac", 4096};
static const struct stub_codec stub_pcm = {"mp4mc_pcm", "pcm_s16le", "pcmC", 1024};

struct stub_encoder {
	const struct stub_codec *codec;
	obs_encoder_t *encoder;
	uint8_t extra[64];
	size_t extra_size;
};

static const char *stub_name(void *type_data)
{
	return ((const struct stub_codec *)type_data)->id;
}

static void wb32(uint8_t *p, uint32_t v)
{
	p[0] = (uint8_t)(v >> 24);
	p[1] = (uint8_t)(v >> 16);
	p[2] = (uint8_t)(v >> 8);
	p[3] = (uint8_t)v;
}

/* Codec configuration as the real encoders produce it for `channels`. */
static void build_extra_data(struct stub_encoder *e, size_t channels)
{
	uint8_t *x = e->extra;
	const char *codec = e->codec->codec;

	if (strcmp(codec, "aac") == 0) {
		/* AudioSpecificConfig: AAC-LC, 48 kHz, channel configuration */
		uint8_t cfg = channels == 6 ? 6 : (uint8_t)channels;
		x[0] = 0x11;
		x[1] = (uint8_t)(0x80 | (cfg << 3));
		e->extra_size = 2;
	} else if (strcmp(codec, "opus") == 0) {
		/* OpusHead, mapping family 1 above two channels */
		memcpy(x, "OpusHead", 8);
		x[8] = 1;
		x[9] = (uint8_t)channels;
		x[10] = 0x38; /* pre-skip 312, little endian */
		x[11] = 0x01;
		x[12] = 0x80; /* 48000 Hz, little endian */
		x[13] = 0xbb;
		x[14] = 0;
		x[15] = 0;
		x[16] = 0; /* output gain */
		x[17] = 0;
		x[18] = channels > 2 ? 1 : 0;
		e->extra_size = 19;
		if (channels > 2) {
			x[19] = 4; /* stream count */
			x[20] = 2; /* coupled streams */
			const uint8_t map[6] = {0, 4, 1, 2, 3, 5};
			memcpy(x + 21, map, channels);
			e->extra_size = 21 + channels;
		}
	} else if (strcmp(codec, "flac") == 0) {
		/* STREAMINFO: 4096-sample blocks, 48 kHz, channels, 16 bit */
		memset(x, 0, 34);
		x[0] = 0x10;
		x[2] = 0x10;
		x[10] = 0x0b;
		x[11] = 0xb8;
		x[12] = (uint8_t)(((channels - 1) << 1) | 0);
		x[13] = 0xf0;
		e->extra_size = 34;
	} else if (strcmp(codec, "alac") == 0) {
		/* 'alac' magic cookie box as FFmpeg's encoder writes it */
		memset(x, 0, 36);
		wb32(x, 36);
		memcpy(x + 4, "alac", 4);
		wb32(x + 12, 4096); /* frame length */
		x[17] = 24;         /* bit depth */
		x[21] = (uint8_t)channels;
		wb32(x + 32, 48000);
		e->extra_size = 36;
	} else {
		e->extra_size = 0;
	}
}

static void *stub_create(obs_data_t *settings, obs_encoder_t *encoder)
{
	(void)settings;
	struct stub_encoder *e = bzalloc(sizeof(*e));
	const char *id = obs_encoder_get_id(encoder);
	const struct stub_codec *all[] = {&stub_aac, &stub_opus, &stub_flac, &stub_alac, &stub_pcm};
	for (size_t i = 0; i < sizeof(all) / sizeof(all[0]); i++) {
		if (strcmp(all[i]->id, id) == 0)
			e->codec = all[i];
	}
	e->encoder = encoder;
	build_extra_data(e, audio_output_get_channels(obs_encoder_audio(encoder)));
	return e;
}

static void stub_destroy(void *data)
{
	bfree(data);
}

static bool stub_encode(void *data, struct encoder_frame *frame, struct encoder_packet *packet, bool *received)
{
	(void)data;
	(void)frame;
	(void)packet;
	*received = false;
	return true;
}

static size_t stub_frame_size(void *data)
{
	return ((struct stub_encoder *)data)->codec->frame_size;
}

static bool stub_extra_data(void *data, uint8_t **extra_data, size_t *size)
{
	struct stub_encoder *e = data;
	if (!e->extra_size)
		return false;
	*extra_data = e->extra;
	*size = e->extra_size;
	return true;
}

static void register_stub(const struct stub_codec *codec)
{
	struct obs_encoder_info info = {
		.id = codec->id,
		.type = OBS_ENCODER_AUDIO,
		.codec = codec->codec,
		.get_name = stub_name,
		.create = stub_create,
		.destroy = stub_destroy,
		.encode = stub_encode,
		.get_frame_size = stub_frame_size,
		.get_extra_data = stub_extra_data,
		.type_data = (void *)codec,
	};
	obs_register_encoder(&info);
}

/* ------------------------------------------------------------------------ */
/* Output that only holds the encoders for the muxer.                       */

static const char *output_name(void *type_data)
{
	(void)type_data;
	return "mp4 mux multichannel test output";
}

static void *output_create(obs_data_t *settings, obs_output_t *output)
{
	(void)settings;
	return output;
}

static void output_destroy(void *data)
{
	(void)data;
}

static bool output_start(void *data)
{
	(void)data;
	return false;
}

static void output_stop(void *data, uint64_t ts)
{
	(void)data;
	(void)ts;
}

static void output_encoded_packet(void *data, struct encoder_packet *packet)
{
	(void)data;
	(void)packet;
}

/* ------------------------------------------------------------------------ */
/* Box reading                                                              */

static uint32_t rb32(const uint8_t *p)
{
	return (uint32_t)p[0] << 24 | (uint32_t)p[1] << 16 | (uint32_t)p[2] << 8 | p[3];
}

static uint16_t rb16(const uint8_t *p)
{
	return (uint16_t)(p[0] << 8 | p[1]);
}

/* Finds the first box named `name` among the children in [start, end) and
 * returns its offset, or 0 (no box we look for starts the file). */
static size_t find_box(const uint8_t *buf, size_t start, size_t end, const char *name, size_t *box_end)
{
	size_t off = start;
	while (off + 8 <= end) {
		size_t size = rb32(buf + off);
		if (size < 8 || off + size > end)
			return 0;
		if (memcmp(buf + off + 4, name, 4) == 0) {
			*box_end = off + size;
			return off;
		}
		off += size;
	}
	return 0;
}

struct sample_entry {
	uint8_t stsd_version;
	char fourcc[5];
	uint16_t entry_version;
	uint16_t channelcount;
	/* the first child box as a reader before ISO/IEC 14496-12:2022 finds
	 * it: version 1 means a QuickTime SoundDescriptionV1 with 16 more
	 * bytes (VLC 3.0 libmp4.c MP4_ReadBox_sample_soun) */
	char legacy_first_child[5];
	bool has_chnl;
};

static bool read_audio_entry(const uint8_t *buf, size_t size, struct sample_entry *out)
{
	const char *path[] = {"moov", "trak", "mdia", "minf", "stbl", "stsd"};
	size_t start = 0, end = size, box_end = 0;

	for (size_t i = 0; i < sizeof(path) / sizeof(path[0]); i++) {
		size_t off = find_box(buf, start, end, path[i], &box_end);
		if (!off)
			return false;
		start = off + 8;
		end = box_end;
	}

	/* stsd: version, flags, entry_count, then the sample entry */
	out->stsd_version = buf[start];
	size_t entry = start + 8;
	size_t entry_end = entry + rb32(buf + entry);
	memcpy(out->fourcc, buf + entry + 4, 4);
	out->fourcc[4] = 0;

	const uint8_t *body = buf + entry + 8;
	out->entry_version = rb16(body + 8);
	out->channelcount = rb16(body + 16);

	size_t first = entry + 8 + 28 + (out->entry_version == 1 ? 16 : 0);
	memset(out->legacy_first_child, 0, sizeof(out->legacy_first_child));
	if (first + 8 <= entry_end) {
		for (size_t i = 0; i < 4; i++) {
			uint8_t c = buf[first + 4 + i];
			out->legacy_first_child[i] = c >= 0x20 && c < 0x7f ? (char)c : '?';
		}
	}

	size_t children = entry + 8 + 28;
	size_t chnl_end;
	out->has_chnl = find_box(buf, children, entry_end, "chnl", &chnl_end) != 0;
	return true;
}

/* ------------------------------------------------------------------------ */
/* Muxing                                                                   */

static void submit(struct mp4_mux *mux, obs_encoder_t *enc, int64_t pts, size_t frame_size)
{
	uint8_t payload[32];
	memset(payload, 0x5a, sizeof(payload));

	/* refcounted packet data, as obs_encoder_packet_create_instance makes */
	long *refs = bmalloc(sizeof(long) + sizeof(payload));
	*refs = 1;
	memcpy(refs + 1, payload, sizeof(payload));

	struct encoder_packet pkt = {
		.data = (uint8_t *)(refs + 1),
		.size = sizeof(payload),
		.pts = pts * (int64_t)frame_size,
		.dts = pts * (int64_t)frame_size,
		.timebase_num = 1,
		.timebase_den = 48000,
		.type = OBS_ENCODER_AUDIO,
		.keyframe = true,
		.encoder = enc,
	};
	pkt.dts_usec = pkt.dts * 1000000 / 48000;
	pkt.sys_dts_usec = pkt.dts_usec;

	assert_true(mp4_mux_submit_packet(mux, &pkt));
	obs_encoder_packet_release(&pkt);
}

/* Muxes `codec` on a mix with `speakers` into a Hybrid MP4 file and
 * reads back its audio sample entry. */
static void mux_audio(const struct stub_codec *codec, enum speaker_layout speakers, struct sample_entry *entry)
{
	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = speakers};
	assert_true(obs_reset_audio(&oai));

	obs_encoder_t *enc = obs_audio_encoder_create(codec->id, "audio", NULL, 0, NULL);
	assert_non_null(enc);
	obs_encoder_set_audio(enc, obs_get_audio());

	obs_output_t *output = obs_output_create(OUTPUT_ID, "output", NULL, NULL);
	assert_non_null(output);
	obs_output_set_audio_encoder(output, enc, 0);
	assert_true(obs_output_initialize_encoders(output, 0));

	/* A file, like the real output: the muxer seeks back to rewrite the
	 * header when it finalises. */
	char path[64];
	snprintf(path, sizeof(path), "test_mp4_mux_multichannel_%s.mp4", codec->codec);
	struct serializer s;
	assert_true(file_output_serializer_init(&s, path));

	struct mp4_mux *mux = mp4_mux_create(output, &s, 0, FLAVOR_MP4);
	assert_non_null(mux);
	for (int64_t i = 0; i < 24; i++)
		submit(mux, enc, i, codec->frame_size);
	assert_true(mp4_mux_finalise(mux));
	mp4_mux_destroy(mux);
	file_output_serializer_free(&s);

	obs_output_release(output);
	obs_encoder_release(enc);

	uint8_t *buf = NULL;
	size_t size = 0;
	FILE *f = os_fopen(path, "rb");
	assert_non_null(f);
	fseek(f, 0, SEEK_END);
	size = (size_t)ftell(f);
	fseek(f, 0, SEEK_SET);
	buf = bmalloc(size);
	size_t got = fread(buf, 1, size, f);
	fclose(f);
	os_unlink(path);

	bool found = got == size && read_audio_entry(buf, size, entry);
	bfree(buf);

	if (!found) {
		fprintf(stderr, "no audio stsd box in the %s file\n", codec->codec);
		fail();
	}
	fprintf(stderr, "%s, %d speakers: stsd v%u, %s v%u, channelcount %u, first child '%s', chnl %s\n", codec->codec,
		(int)speakers, entry->stsd_version, entry->fourcc, entry->entry_version, entry->channelcount,
		entry->legacy_first_child, entry->has_chnl ? "yes" : "no");
}

/* A 5.1 track of a self-describing codec: version 0 entry, so readers find
 * the configuration box right after it; the chnl box keeps the layout. */
static void check_multichannel(const struct stub_codec *codec)
{
	struct sample_entry e;
	mux_audio(codec, SPEAKERS_5POINT1, &e);

	assert_int_equal(e.channelcount, 6);
	if (strcmp(e.legacy_first_child, codec->config_box) != 0) {
		fprintf(stderr,
			"%s 5.1: a version %u sample entry hides '%s' from VLC 3 and other "
			"pre-2022 readers (they read '%s' there)\n",
			codec->codec, e.entry_version, codec->config_box, e.legacy_first_child);
		fail();
	}
	assert_int_equal(e.entry_version, 0);
	assert_int_equal(e.stsd_version, 0);
	assert_true(e.has_chnl);
}

static void aac_51_config_box_follows_entry(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_multichannel(&stub_aac);
}

static void opus_51_config_box_follows_entry(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_multichannel(&stub_opus);
}

static void flac_51_config_box_follows_entry(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_multichannel(&stub_flac);
}

static void alac_51_config_box_follows_entry(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_multichannel(&stub_alac);
}

/* Stereo AAC was always written as version 0 without chnl; keep it so. */
static void aac_stereo_unchanged(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct sample_entry e;
	mux_audio(&stub_aac, SPEAKERS_STEREO, &e);
	assert_int_equal(e.channelcount, 2);
	assert_int_equal(e.stsd_version, 0);
	assert_int_equal(e.entry_version, 0);
	assert_string_equal(e.legacy_first_child, "esds");
	assert_false(e.has_chnl);
}

/* ISO/IEC 23003-5 PCM has no channel configuration of its own; its 5.1
 * entry stays version 1 with the chnl box. */
static void pcm_51_keeps_version_1(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct sample_entry e;
	mux_audio(&stub_pcm, SPEAKERS_5POINT1, &e);
	assert_string_equal(e.fourcc, "ipcm");
	assert_int_equal(e.channelcount, 6);
	assert_int_equal(e.stsd_version, 1);
	assert_int_equal(e.entry_version, 1);
	assert_true(e.has_chnl);
}

static int setup(void **state)
{
	(void)state;
	base_set_log_handler(quiet_log, NULL);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_output_info info = {
		.id = OUTPUT_ID,
		.flags = OBS_OUTPUT_AUDIO | OBS_OUTPUT_ENCODED | OBS_OUTPUT_MULTI_TRACK,
		.get_name = output_name,
		.create = output_create,
		.destroy = output_destroy,
		.start = output_start,
		.stop = output_stop,
		.encoded_packet = output_encoded_packet,
		.encoded_audio_codecs = "aac;opus;flac;alac;pcm_s16le",
	};
	obs_register_output(&info);

	register_stub(&stub_aac);
	register_stub(&stub_opus);
	register_stub(&stub_flac);
	register_stub(&stub_alac);
	register_stub(&stub_pcm);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(aac_51_config_box_follows_entry),
		cmocka_unit_test(opus_51_config_box_follows_entry),
		cmocka_unit_test(flac_51_config_box_follows_entry),
		cmocka_unit_test(alac_51_config_box_follows_entry),
		cmocka_unit_test(aac_stereo_unchanged),
		cmocka_unit_test(pcm_51_keeps_version_1),
	};
	return cmocka_run_group_tests(tests, setup, teardown);
}
