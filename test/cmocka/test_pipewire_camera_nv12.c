#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <cmocka.h>

#include <X11/Xlib.h>

#include <pipewire/pipewire.h>
#include <spa/param/video/format-utils.h>
#include <spa/param/buffers.h>

#include <obs.h>
#include <obs-nix-platform.h>
#include <util/platform.h>
#include <util/threading.h>

#include "formats.h"
#include "pipewire.h"

/* obsproject/obs-studio#13823 (reported by EberhartLeberhart) and
 * obsproject/obs-studio#14002 (theCarlG): a PipeWire camera that sends NV12
 * shows a green, banded picture or crashes OBS as soon as it streams.
 *
 * End to end: a producer stream on a private PipeWire daemon (started by
 * test_pipewire_cursor_hdr.sh: own runtime dir, no session manager, no
 * devices) plays the camera. It offers only NV12, like the cameras in the
 * reports, and sends a picture of four coloured quadrants. The capture is
 * the PipeWire camera source's own stream code (pipewire.c), connected the
 * way camera-portal.c connects it: an async video source, the camera's
 * format selected, the producer's node as target. The camera portal itself
 * only hands over the PipeWire connection, so the test connects to the
 * private daemon directly. The source is the only one on the output, which is
 * read back in BGRA; every quadrant must show its colour.
 *
 * Producers send NV12 in different layouts: both planes in one block with
 * rows of the frame width (UVC cameras through the V4L2 SPA plugin), the same
 * with padded rows, and one block per plane.
 *
 * obsproject/obs-studio#13744 (reported by Owlock): a camera that offers only
 * I420 (DroidCam's v4l2loopback device) stayed black, the stream failing
 * with "no more input formats". The camera source lists a camera's formats
 * through the format table (formats.c), which had no I420, so no format
 * could be selected and the stream offered none the camera has. The I420
 * cases offer only I420 (and YV12, its chroma planes swapped), select the
 * format the way camera-portal.c does (none when the table lacks it), and
 * send three planes in one block or one block each.
 *
 * At an odd width (63) with rows not padded, I420 and YV12 chroma rows are
 * (63 + 1) / 2 = 32 bytes; the camera source used to take them to be
 * 63 / 2 = 31 and dropped every frame, so the source stayed black.
 *
 * Needs an X display for obs_startup() (xvfb-run is enough); the wrapper
 * skips (77) without PipeWire, and the cases skip when obs_startup() or the
 * private PipeWire is not available. */

#define SKIP_EXIT 77

#define WIDTH 64
#define HEIGHT 48

const char *obs_module_text(const char *val)
{
	return val;
}

/* ------------------------------------------------------------------ */
/* The picture: four quadrants, BT.709 limited range                  */

struct rgb {
	uint8_t r, g, b;
};

static const struct rgb quadrant_colors[4] = {
	{200, 40, 40},  /* top left: red */
	{40, 200, 40},  /* top right: green */
	{40, 40, 200},  /* bottom left: blue */
	{200, 200, 40}, /* bottom right: yellow */
};

static int quadrant_at(uint32_t x, uint32_t y)
{
	return (y >= HEIGHT / 2 ? 2 : 0) + (x >= WIDTH / 2 ? 1 : 0);
}

static uint8_t clamp_u8(double v)
{
	return v < 0.0 ? 0 : v > 255.0 ? 255 : (uint8_t)(v + 0.5);
}

static void rgb_to_yuv709(struct rgb c, uint8_t *y, uint8_t *u, uint8_t *v)
{
	const double kr = 0.2126, kb = 0.0722;
	double r = c.r / 255.0, g = c.g / 255.0, b = c.b / 255.0;
	double luma = kr * r + (1.0 - kr - kb) * g + kb * b;

	*y = clamp_u8(16.0 + 219.0 * luma);
	*u = clamp_u8(128.0 + 224.0 * (b - luma) / (2.0 * (1.0 - kb)));
	*v = clamp_u8(128.0 + 224.0 * (r - luma) / (2.0 * (1.0 - kr)));
}

/* ------------------------------------------------------------------ */
/* Producer: the camera                                               */

struct layout {
	const char *name;
	uint32_t spa_format; /* NV12, I420 or YV12 */
	uint32_t blocks;     /* 1: all planes in one block, else one per plane */
	uint32_t stride;     /* bytes per luma row (NV12: both planes) */
	uint32_t uv_pad;     /* bytes before a chroma plane in its own block */
	uint32_t width;      /* frame width; 0 for WIDTH */
};

static uint32_t frame_width(const struct layout *l)
{
	return l->width ? l->width : WIDTH;
}

static bool is_nv12(const struct layout *l)
{
	return l->spa_format == SPA_VIDEO_FORMAT_NV12;
}

static uint32_t n_planes(const struct layout *l)
{
	return is_nv12(l) ? 2 : 3;
}

struct producer {
	struct pw_thread_loop *loop;
	struct pw_context *context;
	struct pw_core *core;
	struct pw_stream *stream;
	struct spa_hook stream_listener;
	struct pw_registry *registry;
	struct spa_hook registry_listener;
	struct spa_source *timer;
	const struct layout *layout;
	uint32_t node;

	/* Written on the PipeWire thread, read under the loop lock. */
	uint32_t capture_node;
	struct {
		uint32_t id, node;
		bool out;
	} ports[16];
	size_t n_ports;
	struct pw_proxy *link;
};

static struct producer producer;

static uint32_t y_size(const struct layout *l)
{
	return l->stride * HEIGHT;
}

/* Bytes per chroma row: planar chroma rows are half the luma stride,
 * rounded up, as V4L2 lays out YU12. */
static uint32_t c_stride(const struct layout *l)
{
	return is_nv12(l) ? l->stride : (l->stride + 1) / 2;
}

static uint32_t c_size(const struct layout *l)
{
	return c_stride(l) * (HEIGHT / 2);
}

static uint32_t frame_size(const struct layout *l)
{
	return y_size(l) + (n_planes(l) - 1) * c_size(l);
}

/* planes[]: the planes in memory order (YV12: Y, V, U). */
static void fill_planes(const struct layout *l, uint8_t *planes[3])
{
	const bool yv12 = l->spa_format == SPA_VIDEO_FORMAT_YV12;
	uint8_t *u_plane = planes[yv12 ? 2 : 1];
	uint8_t *v_plane = planes[yv12 ? 1 : 2];

	for (uint32_t row = 0; row < HEIGHT; row++) {
		for (uint32_t x = 0; x < frame_width(l); x++) {
			uint8_t y, u, v;

			rgb_to_yuv709(quadrant_colors[quadrant_at(x, row)], &y, &u, &v);
			planes[0][row * l->stride + x] = y;
			if ((row & 1) || (x & 1))
				continue;
			if (is_nv12(l)) {
				planes[1][(row / 2) * l->stride + x] = u;
				planes[1][(row / 2) * l->stride + x + 1] = v;
			} else {
				u_plane[(row / 2) * c_stride(l) + x / 2] = u;
				v_plane[(row / 2) * c_stride(l) + x / 2] = v;
			}
		}
	}
}

static void on_producer_process(void *data)
{
	const struct layout *l = producer.layout;
	struct pw_buffer *b;
	struct spa_buffer *buf;

	UNUSED_PARAMETER(data);

	b = pw_stream_dequeue_buffer(producer.stream);
	if (!b)
		return;

	buf = b->buffer;
	if (l->blocks == 1 && buf->n_datas >= 1 && buf->datas[0].data && buf->datas[0].maxsize >= frame_size(l)) {
		struct spa_data *d = &buf->datas[0];
		uint8_t *planes[3] = {d->data, (uint8_t *)d->data + y_size(l),
				      (uint8_t *)d->data + y_size(l) + c_size(l)};

		memset(d->data, 0, d->maxsize);
		fill_planes(l, planes);
		*d->chunk = (struct spa_chunk){.offset = 0, .size = frame_size(l), .stride = (int32_t)l->stride};
	} else if (l->blocks == n_planes(l) && buf->n_datas >= l->blocks) {
		uint8_t *planes[3] = {NULL};

		for (uint32_t p = 0; p < l->blocks; p++) {
			struct spa_data *d = &buf->datas[p];
			uint32_t pad = p ? l->uv_pad : 0;
			uint32_t size = p ? c_size(l) : y_size(l);

			if (!d->data || d->maxsize < pad + size)
				goto queue;
			memset(d->data, 0, d->maxsize);
			planes[p] = (uint8_t *)d->data + pad;
			*d->chunk = (struct spa_chunk){.offset = pad,
						       .size = size,
						       .stride = (int32_t)(p ? c_stride(l) : l->stride)};
		}
		fill_planes(l, planes);
	}

queue:
	pw_stream_queue_buffer(producer.stream, b);
}

static void on_producer_param_changed(void *data, uint32_t id, const struct spa_pod *param)
{
	const struct layout *l = producer.layout;
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];
	uint32_t size = l->blocks == 1 ? frame_size(l) : y_size(l) + l->uv_pad;

	UNUSED_PARAMETER(data);

	if (!param || id != SPA_PARAM_Format)
		return;

	params[0] = spa_pod_builder_add_object(&b, SPA_TYPE_OBJECT_ParamBuffers, SPA_PARAM_Buffers,
					       SPA_PARAM_BUFFERS_buffers, SPA_POD_CHOICE_RANGE_Int(4, 2, 8),
					       SPA_PARAM_BUFFERS_blocks, SPA_POD_Int(l->blocks), SPA_PARAM_BUFFERS_size,
					       SPA_POD_Int(size), SPA_PARAM_BUFFERS_stride, SPA_POD_Int(l->stride),
					       SPA_PARAM_BUFFERS_dataType,
					       SPA_POD_CHOICE_FLAGS_Int(1 << SPA_DATA_MemPtr));
	pw_stream_update_params(producer.stream, params, 1);
}

static const struct pw_stream_events producer_events = {
	PW_VERSION_STREAM_EVENTS,
	.param_changed = on_producer_param_changed,
	.process = on_producer_process,
};

static void on_timeout(void *data, uint64_t expirations)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(expirations);

	pw_stream_trigger_process(producer.stream);
}

static uint32_t dict_uint(const struct spa_dict *props, const char *key)
{
	const char *str = props ? spa_dict_lookup(props, key) : NULL;

	return str ? (uint32_t)strtoul(str, NULL, 10) : SPA_ID_INVALID;
}

static const char *dict_str(const struct spa_dict *props, const char *key)
{
	const char *str = props ? spa_dict_lookup(props, key) : NULL;

	return str ? str : "";
}

static void on_registry_global(void *data, uint32_t id, uint32_t permissions, const char *type, uint32_t version,
			       const struct spa_dict *props)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(permissions);
	UNUSED_PARAMETER(version);

	if (strcmp(type, PW_TYPE_INTERFACE_Node) == 0) {
		if (strcmp(dict_str(props, PW_KEY_MEDIA_CLASS), "Stream/Input/Video") == 0)
			producer.capture_node = id;
	} else if (strcmp(type, PW_TYPE_INTERFACE_Port) == 0 && producer.n_ports < SPA_N_ELEMENTS(producer.ports)) {
		producer.ports[producer.n_ports].id = id;
		producer.ports[producer.n_ports].node = dict_uint(props, PW_KEY_NODE_ID);
		producer.ports[producer.n_ports].out = strcmp(dict_str(props, PW_KEY_PORT_DIRECTION), "out") == 0;
		producer.n_ports++;
	}
}

static uint32_t find_port(uint32_t node, bool out)
{
	for (size_t i = 0; i < producer.n_ports; i++) {
		if (producer.ports[i].node == node && producer.ports[i].out == out)
			return producer.ports[i].id;
	}
	return SPA_ID_INVALID;
}

static const struct pw_registry_events registry_events = {
	PW_VERSION_REGISTRY_EVENTS,
	.global = on_registry_global,
};

static bool producer_start(const struct layout *l)
{
	uint8_t buffer[1024];
	struct spa_pod_builder b = SPA_POD_BUILDER_INIT(buffer, sizeof(buffer));
	const struct spa_pod *params[1];
	struct timespec interval = {0, 1000000000 / 30};
	struct timespec first = {0, 1};

	memset(&producer, 0, sizeof(producer));
	producer.layout = l;
	producer.capture_node = SPA_ID_INVALID;
	producer.node = SPA_ID_INVALID;

	producer.loop = pw_thread_loop_new("camera-nv12-producer", NULL);
	producer.context = pw_context_new(pw_thread_loop_get_loop(producer.loop), NULL, 0);
	if (pw_thread_loop_start(producer.loop) < 0)
		return false;

	pw_thread_loop_lock(producer.loop);
	producer.core = pw_context_connect(producer.context, NULL, 0);
	if (!producer.core) {
		pw_thread_loop_unlock(producer.loop);
		return false;
	}

	producer.stream = pw_stream_new(producer.core, "camera-nv12-producer",
					pw_properties_new(PW_KEY_MEDIA_CLASS, "Video/Source", PW_KEY_MEDIA_TYPE,
							  "Video", PW_KEY_MEDIA_ROLE, "Camera", NULL));
	pw_stream_add_listener(producer.stream, &producer.stream_listener, &producer_events, NULL);

	params[0] = spa_pod_builder_add_object(&b, SPA_TYPE_OBJECT_Format, SPA_PARAM_EnumFormat, SPA_FORMAT_mediaType,
					       SPA_POD_Id(SPA_MEDIA_TYPE_video), SPA_FORMAT_mediaSubtype,
					       SPA_POD_Id(SPA_MEDIA_SUBTYPE_raw), SPA_FORMAT_VIDEO_format,
					       SPA_POD_Id(l->spa_format), SPA_FORMAT_VIDEO_size,
					       SPA_POD_Rectangle(&SPA_RECTANGLE(frame_width(l), HEIGHT)),
					       SPA_FORMAT_VIDEO_framerate, SPA_POD_Fraction(&SPA_FRACTION(30, 1)),
					       SPA_FORMAT_VIDEO_colorMatrix, SPA_POD_Id(SPA_VIDEO_COLOR_MATRIX_BT709),
					       SPA_FORMAT_VIDEO_colorRange, SPA_POD_Id(SPA_VIDEO_COLOR_RANGE_16_235));

	pw_stream_connect(producer.stream, PW_DIRECTION_OUTPUT, PW_ID_ANY,
			  PW_STREAM_FLAG_DRIVER | PW_STREAM_FLAG_MAP_BUFFERS, params, 1);

	producer.registry = pw_core_get_registry(producer.core, PW_VERSION_REGISTRY, 0);
	pw_registry_add_listener(producer.registry, &producer.registry_listener, &registry_events, NULL);

	producer.timer = pw_loop_add_timer(pw_thread_loop_get_loop(producer.loop), on_timeout, NULL);
	pw_loop_update_timer(pw_thread_loop_get_loop(producer.loop), producer.timer, &first, &interval, false);
	pw_thread_loop_unlock(producer.loop);

	for (int i = 0; i < 200; i++) {
		pw_thread_loop_lock(producer.loop);
		producer.node = pw_stream_get_node_id(producer.stream);
		pw_thread_loop_unlock(producer.loop);
		if (producer.node != SPA_ID_INVALID)
			return true;
		os_sleep_ms(10);
	}
	return false;
}

/* The camera's stream is meant for a session manager to link; this
 * PipeWire has none, so link the producer to the capture directly. */
static bool producer_link_ready(void)
{
	bool linked;

	pw_thread_loop_lock(producer.loop);
	uint32_t out_port = find_port(producer.node, true);
	uint32_t in_port = find_port(producer.capture_node, false);

	if (!producer.link && producer.capture_node != SPA_ID_INVALID && out_port != SPA_ID_INVALID &&
	    in_port != SPA_ID_INVALID) {
		struct pw_properties *props = pw_properties_new(NULL, NULL);

		pw_properties_setf(props, PW_KEY_LINK_OUTPUT_NODE, "%u", producer.node);
		pw_properties_setf(props, PW_KEY_LINK_OUTPUT_PORT, "%u", out_port);
		pw_properties_setf(props, PW_KEY_LINK_INPUT_NODE, "%u", producer.capture_node);
		pw_properties_setf(props, PW_KEY_LINK_INPUT_PORT, "%u", in_port);
		producer.link = pw_core_create_object(producer.core, "link-factory", PW_TYPE_INTERFACE_Link,
						      PW_VERSION_LINK, &props->dict, 0);
		pw_properties_free(props);
	}
	linked = producer.link != NULL;
	pw_thread_loop_unlock(producer.loop);
	return linked;
}

static void producer_stop(void)
{
	if (!producer.loop)
		return;
	pw_thread_loop_lock(producer.loop);
	if (producer.timer)
		pw_loop_destroy_source(pw_thread_loop_get_loop(producer.loop), producer.timer);
	if (producer.link)
		pw_proxy_destroy(producer.link);
	if (producer.registry)
		pw_proxy_destroy((struct pw_proxy *)producer.registry);
	if (producer.stream)
		pw_stream_destroy(producer.stream);
	if (producer.core)
		pw_core_disconnect(producer.core);
	pw_thread_loop_unlock(producer.loop);
	pw_thread_loop_stop(producer.loop);
	pw_context_destroy(producer.context);
	pw_thread_loop_destroy(producer.loop);
	memset(&producer, 0, sizeof(producer));
}

/* ------------------------------------------------------------------ */
/* The camera source: async video, fed by pipewire.c                  */

static const char *camera_get_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);
	return "test PipeWire camera";
}

static void *camera_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void camera_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static struct obs_source_info camera_info = {
	.id = "test_pipewire_camera_nv12",
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_ASYNC_VIDEO,
	.get_name = camera_get_name,
	.create = camera_create,
	.destroy = camera_destroy,
};

static int connect_pipewire_socket(void)
{
	const char *dir = getenv("PIPEWIRE_RUNTIME_DIR");
	struct sockaddr_un addr = {.sun_family = AF_UNIX};
	int fd;

	if (!dir)
		return -1;
	snprintf(addr.sun_path, sizeof(addr.sun_path), "%s/pipewire-0", dir);
	fd = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
	if (fd < 0)
		return -1;
	if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
		close(fd);
		return -1;
	}
	return fd;
}

/* ------------------------------------------------------------------ */
/* Output readback                                                    */

struct samples {
	uint64_t frames;
	struct rgb quadrant[4];
};

static pthread_mutex_t samples_mutex = PTHREAD_MUTEX_INITIALIZER;
static struct samples samples;

static void on_raw_video(void *param, struct video_data *frame)
{
	UNUSED_PARAMETER(param);

	pthread_mutex_lock(&samples_mutex);
	for (int q = 0; q < 4; q++) {
		uint32_t x = (q & 1) ? WIDTH * 3 / 4 : WIDTH / 4;
		uint32_t y = (q & 2) ? HEIGHT * 3 / 4 : HEIGHT / 4;
		const uint8_t *p = frame->data[0] + (size_t)y * frame->linesize[0] + x * 4;

		samples.quadrant[q] = (struct rgb){p[2], p[1], p[0]}; /* BGRA */
	}
	samples.frames++;
	pthread_mutex_unlock(&samples_mutex);
}

static bool color_close(struct rgb a, struct rgb b)
{
	const int tolerance = 24;

	return abs(a.r - b.r) <= tolerance && abs(a.g - b.g) <= tolerance && abs(a.b - b.b) <= tolerance;
}

static bool all_quadrants_match(const struct samples *s)
{
	for (int q = 0; q < 4; q++) {
		if (!color_close(s->quadrant[q], quadrant_colors[q]))
			return false;
	}
	return true;
}

/* Reads the output until every quadrant shows its colour for a few frames
 * in a row, or 5 seconds pass. Returns the last samples. */
static bool read_quadrants(struct samples *out)
{
	uint64_t end = os_gettime_ns() + 5000000000ULL;
	uint64_t matched_since = 0;
	bool ok = false;

	pthread_mutex_lock(&samples_mutex);
	memset(&samples, 0, sizeof(samples));
	pthread_mutex_unlock(&samples_mutex);

	obs_add_raw_video_callback(NULL, on_raw_video, NULL);
	while (os_gettime_ns() < end) {
		os_sleep_ms(20);

		pthread_mutex_lock(&samples_mutex);
		*out = samples;
		pthread_mutex_unlock(&samples_mutex);

		if (out->frames == 0 || !all_quadrants_match(out)) {
			matched_since = 0;
			continue;
		}
		if (!matched_since) {
			matched_since = out->frames;
		} else if (out->frames >= matched_since + 5) {
			ok = true;
			break;
		}
	}
	obs_remove_raw_video_callback(on_raw_video, NULL);
	return ok;
}

/* ------------------------------------------------------------------ */
/* Fixture                                                            */

static bool obs_up;
static Display *display;

static void log_handler(int level, const char *format, va_list args, void *param)
{
	UNUSED_PARAMETER(param);

	if (level <= LOG_WARNING || getenv("CAMERA_NV12_TEST_VERBOSE")) {
		vfprintf(stderr, format, args);
		fputc('\n', stderr);
	}
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);

	if (!getenv("DISPLAY")) {
		fprintf(stderr, "no X display: cases skip\n");
		return 0;
	}

	display = XOpenDisplay(NULL);
	if (!display) {
		fprintf(stderr, "cannot open the X display: cases skip\n");
		return 0;
	}

	pw_init(NULL, NULL);

	base_set_log_handler(log_handler, NULL);
	obs_set_nix_platform(OBS_NIX_PLATFORM_X11_EGL);
	obs_set_nix_platform_display(display);
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "obs_startup failed: cases skip\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = WIDTH,
		.base_height = HEIGHT,
		.output_width = WIDTH,
		.output_height = HEIGHT,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.adapter = 0,
		.gpu_conversion = true,
		.scale_type = OBS_SCALE_POINT,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "obs_reset_video failed: cases skip\n");
		obs_shutdown();
		return 0;
	}

	obs_register_source(&camera_info);
	obs_up = true;
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);

	if (obs_up)
		obs_shutdown();
	if (display)
		XCloseDisplay(display);
	return 0;
}

static void check_camera(const struct layout *l)
{
	struct obs_pipewire_connect_stream_info connect_info;
	struct obs_pw_video_format format;
	const struct spa_rectangle resolution = SPA_RECTANGLE(frame_width(l), HEIGHT);
	const struct spa_fraction framerate = SPA_FRACTION(30, 1);
	const enum spa_media_subtype subtype = SPA_MEDIA_SUBTYPE_raw;
	obs_pipewire_stream *stream = NULL;
	obs_pipewire *obs_pw = NULL;
	obs_source_t *camera = NULL;
	struct samples s = {0};
	bool linked = false;
	bool ok;
	int fd;

	if (!obs_up)
		skip();

	if (!producer_start(l)) {
		producer_stop();
		fprintf(stderr, "%s: the private PipeWire daemon is not reachable: skipping\n", l->name);
		skip();
	}

	fd = connect_pipewire_socket();
	assert_true(fd >= 0);

	camera = obs_source_create("test_pipewire_camera_nv12", "camera", NULL, NULL);
	assert_non_null(camera);
	obs_set_output_source(0, camera);

	/* What camera-portal.c does for a selected device: its format list
	 * (camera_format_list) offers the camera's formats found in the format
	 * table, and stream_camera() connects with the selected one, or with
	 * none (all zero) when nothing could be selected. */
	if (!obs_pw_video_format_from_spa_format(l->spa_format, &format)) {
		fprintf(stderr, "%s: the camera's only format is not in the format table: none can be selected\n",
			l->name);
		memset(&format, 0, sizeof(format));
	}
	connect_info = (struct obs_pipewire_connect_stream_info){
		.stream_name = "OBS PipeWire Camera",
		.stream_properties = pw_properties_new(PW_KEY_MEDIA_TYPE, "Video", PW_KEY_MEDIA_CATEGORY, "Capture",
						       PW_KEY_MEDIA_ROLE, "Camera", NULL),
		.video =
			{
				.subtype = &subtype,
				.format = &format,
				.resolution = &resolution,
				.framerate = &framerate,
			},
	};
	obs_pw = obs_pipewire_connect_fd(fd);
	assert_non_null(obs_pw);
	stream = obs_pipewire_connect_stream(obs_pw, camera, (int)producer.node, &connect_info);
	assert_non_null(stream);

	for (int i = 0; i < 250 && !linked; i++) {
		linked = producer_link_ready();
		if (!linked)
			os_sleep_ms(20);
	}

	ok = linked && read_quadrants(&s);

	obs_set_output_source(0, NULL);
	obs_pipewire_destroy(obs_pw); /* closes fd */
	obs_source_release(camera);
	obs_wait_for_destroy_queue();
	producer_stop();

	fprintf(stderr, "%s: %" PRIu64 " output frames", l->name, s.frames);
	for (int q = 0; q < 4; q++)
		fprintf(stderr, ", q%d (%u, %u, %u)", q, s.quadrant[q].r, s.quadrant[q].g, s.quadrant[q].b);
	fputc('\n', stderr);

	if (!linked) {
		fprintf(stderr, "%s: the producer was never linked to the camera's stream\n", l->name);
		fail();
	}
	if (!ok) {
		fprintf(stderr, "%s: the output does not show the camera's picture (expected", l->name);
		for (int q = 0; q < 4; q++)
			fprintf(stderr, " (%u, %u, %u)", quadrant_colors[q].r, quadrant_colors[q].g,
				quadrant_colors[q].b);
		fprintf(stderr, ")\n");
		fail();
	}
}

static void nv12_one_block(void **state)
{
	static const struct layout l = {"NV12, one block", SPA_VIDEO_FORMAT_NV12, 1, WIDTH, 0};

	UNUSED_PARAMETER(state);
	check_camera(&l);
}

static void nv12_one_block_padded_rows(void **state)
{
	static const struct layout l = {"NV12, one block, padded rows", SPA_VIDEO_FORMAT_NV12, 1, WIDTH + 32, 0};

	UNUSED_PARAMETER(state);
	check_camera(&l);
}

static void nv12_two_blocks(void **state)
{
	static const struct layout l = {"NV12, two blocks", SPA_VIDEO_FORMAT_NV12, 2, WIDTH, 64};

	UNUSED_PARAMETER(state);
	check_camera(&l);
}

static void i420_one_block(void **state)
{
	static const struct layout l = {"I420, one block", SPA_VIDEO_FORMAT_I420, 1, WIDTH, 0};

	UNUSED_PARAMETER(state);
	check_camera(&l);
}

static void i420_one_block_padded_rows(void **state)
{
	static const struct layout l = {"I420, one block, padded rows", SPA_VIDEO_FORMAT_I420, 1, WIDTH + 64, 0};

	UNUSED_PARAMETER(state);
	check_camera(&l);
}

static void i420_three_blocks(void **state)
{
	static const struct layout l = {"I420, three blocks", SPA_VIDEO_FORMAT_I420, 3, WIDTH, 32};

	UNUSED_PARAMETER(state);
	check_camera(&l);
}

static void yv12_one_block(void **state)
{
	static const struct layout l = {"YV12, one block", SPA_VIDEO_FORMAT_YV12, 1, WIDTH, 0};

	UNUSED_PARAMETER(state);
	check_camera(&l);
}

/* Odd width, rows not padded: chroma rows of (63 + 1) / 2 bytes. */
static void i420_odd_width(void **state)
{
	static const struct layout l = {"I420, odd width", SPA_VIDEO_FORMAT_I420, 1, WIDTH - 1, 0, WIDTH - 1};

	UNUSED_PARAMETER(state);
	check_camera(&l);
}

static void yv12_odd_width(void **state)
{
	static const struct layout l = {"YV12, odd width", SPA_VIDEO_FORMAT_YV12, 1, WIDTH - 1, 0, WIDTH - 1};

	UNUSED_PARAMETER(state);
	check_camera(&l);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(nv12_one_block),
		cmocka_unit_test(nv12_one_block_padded_rows),
		cmocka_unit_test(nv12_two_blocks),
		cmocka_unit_test(i420_one_block),
		cmocka_unit_test(i420_one_block_padded_rows),
		cmocka_unit_test(i420_three_blocks),
		cmocka_unit_test(yv12_one_block),
		cmocka_unit_test(i420_odd_width),
		cmocka_unit_test(yv12_odd_width),
	};

	if (!getenv("PIPEWIRE_RUNTIME_DIR")) {
		fprintf(stderr, "skipped: run through test_pipewire_cursor_hdr.sh (private PipeWire)\n");
		return SKIP_EXIT;
	}

	return cmocka_run_group_tests(tests, setup, teardown);
}
