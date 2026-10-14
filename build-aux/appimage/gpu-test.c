/* Physical GPU checks for the packaged AppImage (#625). No software fallback. */
#include <EGL/egl.h>
#include <EGL/eglext.h>
#include <GLES2/gl2.h>
#include <GLES2/gl2ext.h>
#include <gbm.h>
#include <libavcodec/avcodec.h>
#include <libavformat/avformat.h>
#include <libavutil/hwcontext.h>
#include <fcntl.h>
#include <errno.h>
#include <inttypes.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

static void loaded_libraries(void)
{
	FILE *maps = fopen("/proc/self/maps", "r");
	char line[4096];
	if (!maps)
		return;
	while (fgets(line, sizeof line, maps)) {
		if (strstr(line, "/libgbm") || strstr(line, "/libEGL") || strstr(line, "/libGL") ||
		    strstr(line, "/libOpenGL") || strstr(line, "/libdrm") || strstr(line, "/libva") ||
		    strstr(line, "/libvulkan") || strstr(line, "/libwayland") || strstr(line, "/libgallium") ||
		    strstr(line, "/libLLVM") || strstr(line, "/libcuda") || strstr(line, "/libnvidia") ||
		    strstr(line, "_drv_video.so") || strstr(line, "/libav"))
			fputs(line, stdout);
	}
	fclose(maps);
}

static int gbm_frames(const char *node)
{
	int result = 1, dma_fd = -1, fd = open(node, O_RDWR | O_CLOEXEC);
	struct gbm_device *device = NULL;
	struct gbm_bo *buffer = NULL;
	EGLDisplay display = EGL_NO_DISPLAY;
	EGLContext context = EGL_NO_CONTEXT;
	EGLImageKHR image = EGL_NO_IMAGE_KHR;
	GLuint renderbuffer = 0, framebuffer = 0;
	PFNEGLCREATEIMAGEKHRPROC create_image = (PFNEGLCREATEIMAGEKHRPROC)eglGetProcAddress("eglCreateImageKHR");
	PFNEGLDESTROYIMAGEKHRPROC destroy_image = (PFNEGLDESTROYIMAGEKHRPROC)eglGetProcAddress("eglDestroyImageKHR");
	PFNGLEGLIMAGETARGETRENDERBUFFERSTORAGEOESPROC import_image =
		(PFNGLEGLIMAGETARGETRENDERBUFFERSTORAGEOESPROC)eglGetProcAddress(
			"glEGLImageTargetRenderbufferStorageOES");
	if (fd < 0 || !create_image || !destroy_image || !import_image)
		goto done;
	device = gbm_create_device(fd);
	if (!device)
		goto done;
	display = eglGetPlatformDisplay(EGL_PLATFORM_GBM_KHR, device, NULL);
	if (display == EGL_NO_DISPLAY || !eglInitialize(display, NULL, NULL) || !eglBindAPI(EGL_OPENGL_ES_API))
		goto done;
	EGLConfig config;
	EGLint count;
	const EGLint config_attributes[] = {EGL_SURFACE_TYPE,
					    EGL_PBUFFER_BIT,
					    EGL_RENDERABLE_TYPE,
					    EGL_OPENGL_ES2_BIT,
					    EGL_RED_SIZE,
					    8,
					    EGL_GREEN_SIZE,
					    8,
					    EGL_BLUE_SIZE,
					    8,
					    EGL_ALPHA_SIZE,
					    8,
					    EGL_NONE};
	const EGLint context_attributes[] = {EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE};
	if (!eglChooseConfig(display, config_attributes, &config, 1, &count) || count != 1)
		goto done;
	context = eglCreateContext(display, config, EGL_NO_CONTEXT, context_attributes);
	if (context == EGL_NO_CONTEXT || !eglMakeCurrent(display, EGL_NO_SURFACE, EGL_NO_SURFACE, context))
		goto done;
	const char *renderer = (const char *)glGetString(GL_RENDERER);
	if (!renderer || strstr(renderer, "llvmpipe") || strstr(renderer, "softpipe") || strstr(renderer, "Software"))
		goto done;
	buffer = gbm_bo_create(device, 64, 64, GBM_FORMAT_ARGB8888, GBM_BO_USE_RENDERING);
	if (!buffer || gbm_bo_get_plane_count(buffer) != 1)
		goto done;
	dma_fd = gbm_bo_get_fd(buffer);
	if (dma_fd < 0)
		goto done;
	uint64_t modifier = gbm_bo_get_modifier(buffer);
	EGLint attributes[24] = {EGL_WIDTH,
				 64,
				 EGL_HEIGHT,
				 64,
				 EGL_LINUX_DRM_FOURCC_EXT,
				 GBM_FORMAT_ARGB8888,
				 EGL_DMA_BUF_PLANE0_FD_EXT,
				 dma_fd,
				 EGL_DMA_BUF_PLANE0_OFFSET_EXT,
				 0,
				 EGL_DMA_BUF_PLANE0_PITCH_EXT,
				 (EGLint)gbm_bo_get_stride(buffer)};
	int next = 12;
	if (modifier != UINT64_C(0x00ffffffffffffff)) {
		attributes[next++] = EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT;
		attributes[next++] = (EGLint)(modifier & UINT32_MAX);
		attributes[next++] = EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT;
		attributes[next++] = (EGLint)(modifier >> 32);
	}
	attributes[next] = EGL_NONE;
	image = create_image(display, EGL_NO_CONTEXT, EGL_LINUX_DMA_BUF_EXT, NULL, attributes);
	if (image == EGL_NO_IMAGE_KHR)
		goto done;
	glGenRenderbuffers(1, &renderbuffer);
	glBindRenderbuffer(GL_RENDERBUFFER, renderbuffer);
	import_image(GL_RENDERBUFFER, image);
	glGenFramebuffers(1, &framebuffer);
	glBindFramebuffer(GL_FRAMEBUFFER, framebuffer);
	glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_RENDERBUFFER, renderbuffer);
	if (glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE)
		goto done;
	glViewport(0, 0, 64, 64);
	for (unsigned frame = 0; frame < 12; frame++) {
		unsigned char expected[] = {(frame & 1) ? 255 : 0, (frame & 2) ? 255 : 0, (frame & 4) ? 255 : 0, 255};
		unsigned char pixels[64 * 64 * 4];
		glClearColor(expected[0] / 255.0f, expected[1] / 255.0f, expected[2] / 255.0f, 1);
		glClear(GL_COLOR_BUFFER_BIT);
		glFinish();
		glReadPixels(0, 0, 64, 64, GL_RGBA, GL_UNSIGNED_BYTE, pixels);
		if (glGetError() != GL_NO_ERROR)
			goto done;
		for (unsigned pixel = 0; pixel < 64 * 64; pixel++) {
			if (memcmp(pixels + pixel * 4, expected, sizeof expected)) {
				fprintf(stderr, "DMA-BUF pixel mismatch at frame %u pixel %u\n", frame, pixel);
				goto done;
			}
		}
	}
	printf("GBM/DMA-BUF: node=%s backend=%s renderer=%s modifier=%" PRIx64 " frames=12 pixels=49152 verified\n",
	       node, gbm_device_get_backend_name(device), renderer, modifier);
	loaded_libraries();
	result = 0;
done:
	if (result)
		fprintf(stderr, "GBM/DMA-BUF failed: node=%s EGL=%x GL=%x\n", node, eglGetError(), glGetError());
	if (framebuffer)
		glDeleteFramebuffers(1, &framebuffer);
	if (renderbuffer)
		glDeleteRenderbuffers(1, &renderbuffer);
	if (image != EGL_NO_IMAGE_KHR)
		destroy_image(display, image);
	if (context != EGL_NO_CONTEXT) {
		eglMakeCurrent(display, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
		eglDestroyContext(display, context);
	}
	if (display != EGL_NO_DISPLAY)
		eglTerminate(display);
	if (dma_fd >= 0)
		close(dma_fd);
	if (buffer)
		gbm_bo_destroy(buffer);
	if (device)
		gbm_device_destroy(device);
	if (fd >= 0)
		close(fd);
	return result;
}

static enum AVPixelFormat hardware_format(AVCodecContext *context, const enum AVPixelFormat *formats)
{
	(void)context;
	for (; *formats != AV_PIX_FMT_NONE; formats++) {
		if (*formats == AV_PIX_FMT_VAAPI)
			return *formats;
	}
	fprintf(stderr, "Decoder did not offer VAAPI; software fallback refused\n");
	return AV_PIX_FMT_NONE;
}

static int write_yuv420(FILE *output, const AVFrame *frame)
{
	if (frame->width != 128 || frame->height != 72 ||
	    (frame->format != AV_PIX_FMT_NV12 && frame->format != AV_PIX_FMT_YUV420P))
		return -1;
	for (int row = 0; row < frame->height; row++) {
		if (fwrite(frame->data[0] + row * frame->linesize[0], 1, frame->width, output) != (size_t)frame->width)
			return -1;
	}
	for (int plane = 1; plane <= 2; plane++) {
		for (int row = 0; row < frame->height / 2; row++) {
			if (frame->format == AV_PIX_FMT_YUV420P) {
				if (fwrite(frame->data[plane] + row * frame->linesize[plane], 1, frame->width / 2,
					   output) != (size_t)frame->width / 2)
					return -1;
			} else {
				for (int col = 0; col < frame->width / 2; col++) {
					if (fputc(frame->data[1][row * frame->linesize[1] + col * 2 + plane - 1],
						  output) == EOF)
						return -1;
				}
			}
		}
	}
	return 0;
}

static int receive_frames(AVCodecContext *decoder, FILE *output, unsigned *frames)
{
	AVFrame *gpu = av_frame_alloc(), *cpu = av_frame_alloc();
	int result = AVERROR(ENOMEM);
	if (!gpu || !cpu)
		goto done;
	while ((result = avcodec_receive_frame(decoder, gpu)) >= 0) {
		if (gpu->format != AV_PIX_FMT_VAAPI) {
			result = AVERROR(EINVAL);
			break;
		}
		result = av_hwframe_transfer_data(cpu, gpu, 0);
		if (result < 0)
			break;
		if (write_yuv420(output, cpu)) {
			result = AVERROR(EIO);
			break;
		}
		(*frames)++;
		av_frame_unref(gpu);
		av_frame_unref(cpu);
	}
	if (result == AVERROR(EAGAIN) || result == AVERROR_EOF)
		result = 0;
done:
	av_frame_free(&gpu);
	av_frame_free(&cpu);
	return result;
}

static int vaapi_frames(const char *node, const char *input, const char *destination)
{
	AVFormatContext *format = NULL;
	AVCodecContext *decoder = NULL;
	AVPacket *packet = av_packet_alloc();
	AVBufferRef *device = NULL;
	FILE *output = NULL;
	unsigned frames = 0;
	int result = avformat_open_input(&format, input, NULL, NULL);
	if (result < 0 || (result = avformat_find_stream_info(format, NULL)) < 0)
		goto done;
	const AVCodec *codec = NULL;
	int stream = av_find_best_stream(format, AVMEDIA_TYPE_VIDEO, -1, -1, &codec, 0);
	if (stream < 0 || !packet) {
		result = stream < 0 ? stream : AVERROR(ENOMEM);
		goto done;
	}
	decoder = avcodec_alloc_context3(codec);
	if (!decoder) {
		result = AVERROR(ENOMEM);
		goto done;
	}
	fprintf(stderr, "VAAPI: creating device on %s\n", node);
	result = avcodec_parameters_to_context(decoder, format->streams[stream]->codecpar);
	if (result < 0 || (result = av_hwdevice_ctx_create(&device, AV_HWDEVICE_TYPE_VAAPI, node, NULL, 0)) < 0)
		goto done;
	decoder->hw_device_ctx = av_buffer_ref(device);
	decoder->get_format = hardware_format;
	if (!decoder->hw_device_ctx) {
		result = AVERROR(ENOMEM);
		goto done;
	}
	if ((result = avcodec_open2(decoder, codec, NULL)) < 0)
		goto done;
	fprintf(stderr, "VAAPI: decoder opened, reading packets\n");
	output = fopen(destination, "wb");
	if (!output) {
		result = AVERROR(errno);
		goto done;
	}
	while ((result = av_read_frame(format, packet)) >= 0) {
		if (packet->stream_index == stream) {
			result = avcodec_send_packet(decoder, packet);
			if (result >= 0)
				result = receive_frames(decoder, output, &frames);
		}
		av_packet_unref(packet);
		if (result < 0)
			goto done;
	}
	if (result != AVERROR_EOF || (result = avcodec_send_packet(decoder, NULL)) < 0 ||
	    (result = receive_frames(decoder, output, &frames)) < 0)
		goto done;
	if (frames != 12 || fflush(output)) {
		result = AVERROR(EIO);
		goto done;
	}
	printf("VAAPI: node=%s decoder=%s frames=%u size=128x72 hardware surfaces downloaded\n", node, codec->name,
	       frames);
	loaded_libraries();
done:
	if (result < 0) {
		char error[AV_ERROR_MAX_STRING_SIZE];
		av_strerror(result, error, sizeof error);
		fprintf(stderr, "VAAPI failed: %s\n", error);
	}
	if (output && fclose(output) && result >= 0)
		result = AVERROR(EIO);
	av_packet_free(&packet);
	avcodec_free_context(&decoder);
	av_buffer_unref(&device);
	avformat_close_input(&format);
	return result < 0;
}

int main(int argc, char **argv)
{
	if (argc == 3 && !strcmp(argv[1], "gbm"))
		return gbm_frames(argv[2]);
	if (argc == 5 && !strcmp(argv[1], "vaapi"))
		return vaapi_frames(argv[2], argv[3], argv[4]);
	fprintf(stderr, "usage: %s gbm <render-node> | vaapi <render-node> <12-frame-128x72-h264> <output-yuv420p>\n",
		argv[0]);
	return 1;
}
