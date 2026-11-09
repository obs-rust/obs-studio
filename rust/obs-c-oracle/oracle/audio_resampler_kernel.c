#include "audio_resampler_kernel.h"
/* Fail allocation, init, matrix, compensation, conversion, samples allocation.
 * Only serialized differential tests change this; the default is success. */
static int failure;
static int live;
static uint64_t trace;
static void record(int64_t value)
{
	trace = trace * UINT64_C(1315423911) + (uint64_t)value;
}
uint64_t oracle_resampler_kernel_trace(void)
{
	return trace;
}
struct SwrContext {
	int input_rate, output_rate, input_format, output_format, input_channels, output_channels;
	int calls;
};
void oracle_resampler_kernel_script(int value)
{
	failure = value;
	trace = 0;
}
int oracle_resampler_kernel_live(void)
{
	return live;
}
static int channels(uint64_t mask)
{
	int n = 0;
	while (mask) {
		n += (int)(mask & 1);
		mask >>= 1;
	}
	return n;
}
struct SwrContext *swr_alloc_set_opts(struct SwrContext *old, uint64_t out_layout, int out_format, int out_rate,
				      uint64_t in_layout, int in_format, int in_rate, int offset, void *log)
{
	(void)old;
	(void)offset;
	(void)log;
	record((int64_t)out_layout);
	record(out_format);
	record(out_rate);
	record((int64_t)in_layout);
	record(in_format);
	record(in_rate);
	if (failure == 1)
		return NULL;
	struct SwrContext *c = calloc(1, sizeof(*c));
	if (!c)
		return NULL;
	live++;
	c->input_rate = in_rate;
	c->output_rate = out_rate;
	c->input_format = in_format;
	c->output_format = out_format;
	c->input_channels = channels(in_layout);
	c->output_channels = channels(out_layout);
	return c;
}
int swr_init(struct SwrContext *c)
{
	return failure == 2 || c->input_rate <= 0 || c->output_rate <= 0 || !c->input_channels || !c->output_channels
		       ? -22
		       : 0;
}
int swr_set_matrix(struct SwrContext *c, const double *row, int stride)
{
	(void)c;
	for (int i = 0; i < c->output_channels; i++)
		record((int64_t)row[i]);
	record(stride);
	return failure == 3 ? -22 : 0;
}
int swr_set_compensation(struct SwrContext *c, int delta, int distance)
{
	(void)c;
	record(delta);
	record(distance);
	return failure == 4 ? -22 : 0;
}
int64_t swr_get_delay(struct SwrContext *c, int64_t base)
{
	record(base);
	return c->input_rate != c->output_rate && c->calls ? 32 * base / c->input_rate : 0;
}
static int width(int fmt)
{
	return fmt == 0 || fmt == 5 ? 1 : (fmt == 1 || fmt == 6 ? 2 : 4);
}
int av_samples_alloc(uint8_t **out, int *line, int ch, int frames, int fmt, int align)
{
	(void)line;
	record(ch);
	record(frames);
	record(fmt);
	record(align);
	if (failure == 6 || frames <= 0)
		return -22;
	size_t size = (size_t)frames * (size_t)width(fmt) * (size_t)ch;
	uint8_t *data = calloc(1, size);
	if (!data)
		return -12;
	live++;
	out[0] = data;
	if (fmt >= 5)
		for (int p = 1; p < ch; p++)
			out[p] = data + (size_t)p * (size_t)frames * (size_t)width(fmt);
	return (int)size;
}
static float sample(const uint8_t **in, int fmt, int channels_, int ch, int frame)
{
	int index = fmt >= 5 ? frame : frame * channels_ + ch;
	const uint8_t *p = in[fmt >= 5 ? ch : 0] + (size_t)index * (size_t)width(fmt);
	if (fmt == 3 || fmt == 8) {
		float v;
		memcpy(&v, p, 4);
		return v;
	}
	if (fmt == 1 || fmt == 6) {
		int16_t v;
		memcpy(&v, p, 2);
		return (float)v / 32768.0f;
	}
	if (fmt == 0 || fmt == 5)
		return ((float)*p - 128.0f) / 128.0f;
	int32_t v;
	memcpy(&v, p, 4);
	return (float)((double)v / 2147483648.0);
}
static double bounded(double value, double minimum, double maximum)
{
	/* Keep this dependency double defined for arbitrary safe byte inputs,
	 * including NaNs, infinities, and samples outside the nominal range. */
	if (value != value)
		return 0.0;
	return value < minimum ? minimum : value > maximum ? maximum : value;
}
static void store(uint8_t **out, int fmt, int channels_, int ch, int frame, float value)
{
	int index = fmt >= 5 ? frame : frame * channels_ + ch;
	uint8_t *p = out[fmt >= 5 ? ch : 0] + (size_t)index * (size_t)width(fmt);
	if (fmt == 3 || fmt == 8)
		memcpy(p, &value, 4);
	else if (fmt == 1 || fmt == 6) {
		int16_t v = (int16_t)bounded((double)value * 32768.0, -32768.0, 32767.0);
		memcpy(p, &v, 2);
	} else if (fmt == 0 || fmt == 5)
		*p = (uint8_t)bounded((double)value * 128.0 + 128.0, 0.0, 255.0);
	else {
		int32_t v = (int32_t)bounded((double)value * 2147483648.0, -2147483648.0, 2147483647.0);
		memcpy(p, &v, 4);
	}
}
int swr_convert(struct SwrContext *c, uint8_t **out, int capacity, const uint8_t **in, int frames)
{
	record(capacity);
	record(frames);
	if (failure == 5 || !out[0])
		return -22;
	int count = (int)((int64_t)frames * c->output_rate / c->input_rate);
	if (c->input_rate != c->output_rate && !c->calls)
		count -= 16;
	if (count < 0)
		count = 0;
	if (count > capacity)
		count = capacity;
	for (int f = 0; f < count; f++)
		for (int ch = 0; ch < c->output_channels; ch++) {
			int source = (int)((int64_t)f * c->input_rate / c->output_rate);
			float v = sample(in, c->input_format, c->input_channels,
					 c->input_channels == 1 ? 0 : ch % c->input_channels, source);
			store(out, c->output_format, c->output_channels, ch, f, v);
		}
	c->calls++;
	return count;
}
void swr_free(struct SwrContext **c)
{
	if (*c) {
		free(*c);
		*c = NULL;
		live--;
	}
}
void av_freep(void *p)
{
	void **v = p;
	if (*v) {
		free(*v);
		*v = NULL;
		live--;
	}
}
