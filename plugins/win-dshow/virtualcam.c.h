#include "plugins/win-dshow/build/win-dshow-virtualcam.pch.h"

struct virtualcam_data {
	obs_output_t *output;
	video_queue_t *vq;
	volatile bool active;
	volatile bool stopping;

	/* Virtual Camera audio to a virtual cable (#3183). */
	bool audio_enabled;
	size_t audio_mixer;
	char *audio_device;
	struct vmic_ring ring;
	pthread_mutex_t ring_mutex;
	pthread_t render_thread;
	bool render_thread_started;
	os_event_t *render_ready;
	volatile bool render_active;
	volatile bool render_stop;
};

#define VMIC_RATE 48000
#define VMIC_CHANNELS 2
#define VMIC_RING_SECONDS 1

/* Local copies of the WASAPI GUIDs, so the group needs no uuid libraries. */
static const CLSID vmic_clsid_enumerator = {0xbcde0395,
					    0xe52f,
					    0x467c,
					    {0x8e, 0x3d, 0xc4, 0x57, 0x92, 0x91, 0x69, 0x2e}};
static const IID vmic_iid_enumerator = {0xa95664d2, 0x9614, 0x4f35, {0xa7, 0x46, 0xde, 0x8d, 0xb6, 0x36, 0x17, 0xe6}};
static const IID vmic_iid_audio_client = {0x1cb9ad4c, 0xdbfa, 0x4c32, {0xb1, 0x78, 0xc2, 0xf5, 0x68, 0xa7, 0x03, 0xb2}};
static const IID vmic_iid_render_client = {0xf294acfc, 0x3146, 0x4483, {0xa7, 0xbf, 0xad, 0xdc, 0xa7, 0xc2, 0x60, 0xe2}};
static const PROPERTYKEY vmic_pkey_friendly_name = {
	{0xa45c254e, 0xdf1c, 0x4efd, {0x80, 0x20, 0x67, 0xd1, 0x46, 0xa8, 0x50, 0xe0}},
	14};

static void vmic_read_settings(struct virtualcam_data *vcam, obs_data_t *settings)
{
	vcam->audio_enabled = !obs_data_has_user_value(settings, "audio_enabled") ||
			      obs_data_get_bool(settings, "audio_enabled");
	long long track = obs_data_get_int(settings, "audio_track");
	vcam->audio_mixer = track >= 1 && track <= MAX_AUDIO_MIXES ? (size_t)(track - 1) : 0;
	bfree(vcam->audio_device);
	vcam->audio_device = bstrdup(obs_data_get_string(settings, "audio_device"));
}

/* Opens the render endpoint vmic_route_pick chooses among the active ones,
 * with its friendly name in *name. NULL when there is none. */
static IMMDevice *vmic_open_endpoint(IMMDeviceEnumerator *en, const char *saved, char **name)
{
	IMMDeviceCollection *list = NULL;
	IMMDevice *found = NULL;
	UINT count = 0;

	if (FAILED(IMMDeviceEnumerator_EnumAudioEndpoints(en, eRender, DEVICE_STATE_ACTIVE, &list))) {
		return NULL;
	}
	IMMDeviceCollection_GetCount(list, &count);

	char **names = bzalloc(sizeof(char *) * (count + 1));
	char **ids = bzalloc(sizeof(char *) * (count + 1));
	for (UINT i = 0; i < count; i++) {
		IMMDevice *dev = NULL;
		IPropertyStore *props = NULL;
		LPWSTR wid = NULL;
		PROPVARIANT pv;
		PropVariantInit(&pv);
		if (SUCCEEDED(IMMDeviceCollection_Item(list, i, &dev))) {
			if (SUCCEEDED(IMMDevice_GetId(dev, &wid))) {
				os_wcs_to_utf8_ptr(wid, 0, &ids[i]);
				CoTaskMemFree(wid);
			}
			if (SUCCEEDED(IMMDevice_OpenPropertyStore(dev, STGM_READ, &props))) {
				if (SUCCEEDED(IPropertyStore_GetValue(props, &vmic_pkey_friendly_name, &pv)) &&
				    pv.vt == VT_LPWSTR) {
					os_wcs_to_utf8_ptr(pv.pwszVal, 0, &names[i]);
				}
				PropVariantClear(&pv);
				IPropertyStore_Release(props);
			}
			IMMDevice_Release(dev);
		}
		if (!ids[i]) {
			ids[i] = bstrdup("");
		}
		if (!names[i]) {
			names[i] = bstrdup("");
		}
	}

	const char *id = vmic_route_pick(saved, (const char *const *)names, (const char *const *)ids, count);
	for (UINT i = 0; id && i < count; i++) {
		if (ids[i] != id) {
			continue;
		}
		wchar_t *wid = NULL;
		os_utf8_to_wcs_ptr(id, 0, &wid);
		if (wid && SUCCEEDED(IMMDeviceEnumerator_GetDevice(en, wid, &found))) {
			*name = bstrdup(names[i]);
		}
		bfree(wid);
	}

	for (UINT i = 0; i < count; i++) {
		bfree(names[i]);
		bfree(ids[i]);
	}
	bfree(names);
	bfree(ids);
	IMMDeviceCollection_Release(list);
	return found;
}

/* Initializes a shared-mode client for float stereo at VMIC_RATE; Windows
 * converts to the endpoint's own format. */
static IAudioClient *vmic_open_client(IMMDevice *dev)
{
	IAudioClient *client = NULL;
	if (FAILED(IMMDevice_Activate(dev, &vmic_iid_audio_client, CLSCTX_ALL, NULL, (void **)&client))) {
		return NULL;
	}

	WAVEFORMATEX wfx = {0};
	wfx.wFormatTag = WAVE_FORMAT_IEEE_FLOAT;
	wfx.nChannels = VMIC_CHANNELS;
	wfx.nSamplesPerSec = VMIC_RATE;
	wfx.wBitsPerSample = 32;
	wfx.nBlockAlign = VMIC_CHANNELS * sizeof(float);
	wfx.nAvgBytesPerSec = VMIC_RATE * wfx.nBlockAlign;

	DWORD flags = AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
	/* 100 ms of device buffer, in 100 ns units. */
	if (FAILED(IAudioClient_Initialize(client, AUDCLNT_SHAREMODE_SHARED, flags, 1000000, 0, &wfx, NULL))) {
		IAudioClient_Release(client);
		return NULL;
	}
	return client;
}

/* Copies the ring into the endpoint every 10 ms. Whatever the ring lacks is
 * written as silence, so the cable keeps a steady clock. */
static void vmic_render(struct virtualcam_data *vcam, IAudioClient *client, IAudioRenderClient *render)
{
	UINT32 frames_total = 0;
	IAudioClient_GetBufferSize(client, &frames_total);
	IAudioClient_Start(client);

	while (!os_atomic_load_bool(&vcam->render_stop)) {
		UINT32 padding = 0;
		BYTE *buf = NULL;
		if (FAILED(IAudioClient_GetCurrentPadding(client, &padding))) {
			break;
		}
		UINT32 frames = frames_total - padding;
		if (frames && SUCCEEDED(IAudioRenderClient_GetBuffer(render, frames, &buf))) {
			size_t want = (size_t)frames * VMIC_CHANNELS;
			pthread_mutex_lock(&vcam->ring_mutex);
			size_t got = vmic_ring_pop(&vcam->ring, (float *)buf, want);
			pthread_mutex_unlock(&vcam->ring_mutex);
			memset((float *)buf + got, 0, (want - got) * sizeof(float));
			IAudioRenderClient_ReleaseBuffer(render, frames, 0);
		}
		os_sleep_ms(10);
	}
	IAudioClient_Stop(client);
}

static void *vmic_render_thread(void *data)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;
	HRESULT com = CoInitializeEx(NULL, COINIT_MULTITHREADED);
	IMMDeviceEnumerator *en = NULL;
	IMMDevice *dev = NULL;
	IAudioClient *client = NULL;
	IAudioRenderClient *render = NULL;
	char *name = NULL;

	os_set_thread_name("virtualcam: audio");

	if (SUCCEEDED(CoCreateInstance(&vmic_clsid_enumerator, NULL, CLSCTX_ALL, &vmic_iid_enumerator, (void **)&en))) {
		dev = vmic_open_endpoint(en, vcam->audio_device, &name);
	}
	if (dev) {
		client = vmic_open_client(dev);
	}
	if (client && FAILED(IAudioClient_GetService(client, &vmic_iid_render_client, (void **)&render))) {
		render = NULL;
	}

	if (render) {
		blog(LOG_INFO, "Virtual camera audio: track %zu to '%s'", vcam->audio_mixer + 1, name);
		os_atomic_set_bool(&vcam->render_active, true);
	} else {
		blog(LOG_WARNING, "Virtual camera audio: no virtual audio cable (CABLE Input, VoiceMeeter "
				  "Input) found, video only");
	}
	os_event_signal(vcam->render_ready);

	if (render) {
		vmic_render(vcam, client, render);
	}

	os_atomic_set_bool(&vcam->render_active, false);
	if (render) {
		IAudioRenderClient_Release(render);
	}
	if (client) {
		IAudioClient_Release(client);
	}
	if (dev) {
		IMMDevice_Release(dev);
	}
	if (en) {
		IMMDeviceEnumerator_Release(en);
	}
	bfree(name);
	if (SUCCEEDED(com)) {
		CoUninitialize();
	}
	return NULL;
}

static void vmic_start(struct virtualcam_data *vcam)
{
	if (!vcam->audio_enabled) {
		return;
	}

	struct audio_convert_info aci = {0};
	aci.samples_per_sec = VMIC_RATE;
	aci.format = AUDIO_FORMAT_FLOAT;
	aci.speakers = SPEAKERS_STEREO;
	obs_output_set_audio_conversion(vcam->output, &aci);
	obs_output_set_mixer(vcam->output, vcam->audio_mixer);

	vcam->ring.cap = VMIC_RATE * VMIC_CHANNELS * VMIC_RING_SECONDS;
	vcam->ring.buf = bzalloc(vcam->ring.cap * sizeof(float));
	vcam->ring.head = 0;
	vcam->ring.len = 0;

	os_atomic_set_bool(&vcam->render_stop, false);
	if (os_event_init(&vcam->render_ready, OS_EVENT_TYPE_MANUAL) != 0) {
		return;
	}
	vcam->render_thread_started = pthread_create(&vcam->render_thread, NULL, vmic_render_thread, vcam) == 0;
	/* The frontend asks right after start whether audio runs. */
	if (vcam->render_thread_started) {
		os_event_timedwait(vcam->render_ready, 3000);
	}
}

static void vmic_stop(struct virtualcam_data *vcam)
{
	if (vcam->render_thread_started) {
		os_atomic_set_bool(&vcam->render_stop, true);
		pthread_join(vcam->render_thread, NULL);
		vcam->render_thread_started = false;
	}
	if (vcam->render_ready) {
		os_event_destroy(vcam->render_ready);
		vcam->render_ready = NULL;
	}
	pthread_mutex_lock(&vcam->ring_mutex);
	bfree(vcam->ring.buf);
	vcam->ring.buf = NULL;
	vcam->ring.cap = 0;
	vcam->ring.len = 0;
	pthread_mutex_unlock(&vcam->ring_mutex);
}

static void virtualcam_audio_proc(void *data, calldata_t *cd)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;
	calldata_set_bool(cd, "active", os_atomic_load_bool(&vcam->render_active));
}

static const char *virtualcam_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "Virtual Camera Output";
}

static void virtualcam_destroy(void *data)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;
	video_queue_close(vcam->vq);
	vmic_stop(vcam);
	pthread_mutex_destroy(&vcam->ring_mutex);
	bfree(vcam->audio_device);
	bfree(data);
}

static void *virtualcam_create(obs_data_t *settings, obs_output_t *output)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)bzalloc(sizeof(*vcam));
	vcam->output = output;
	pthread_mutex_init(&vcam->ring_mutex, NULL);
	vmic_read_settings(vcam, settings);

	/* The frontend asks after start whether audio runs, to tell the user
	 * when the camera fell back to video only (#3182). */
	if (output) {
		proc_handler_add(obs_output_get_proc_handler(output), "void virtualcam_audio(out bool active)",
				 virtualcam_audio_proc, vcam);
	}
	return vcam;
}

static void virtualcam_update(void *data, obs_data_t *settings)
{
	vmic_read_settings((struct virtualcam_data *)data, settings);
}

static bool virtualcam_start(void *data)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;
	uint32_t width = obs_output_get_width(vcam->output);
	uint32_t height = obs_output_get_height(vcam->output);

	struct obs_video_info ovi;
	obs_get_video_info(&ovi);

	uint64_t interval = ovi.fps_den * 10000000ULL / ovi.fps_num;

	char res[64];
	snprintf(res, sizeof(res), "%dx%dx%lld", (int)width, (int)height, (long long)interval);

	char *res_file = os_get_config_path_ptr("obs-virtualcam.txt");
	os_quick_write_utf8_file_safe(res_file, res, strlen(res), false, "tmp", NULL);
	bfree(res_file);

	vcam->vq = video_queue_create(width, height, interval);
	if (!vcam->vq) {
		blog(LOG_WARNING, "starting virtual-output failed");
		return false;
	}

	struct video_scale_info vsi = {0};
	vsi.format = VIDEO_FORMAT_NV12;
	vsi.width = width;
	vsi.height = height;
	obs_output_set_video_conversion(vcam->output, &vsi);
	vmic_start(vcam);

	os_atomic_set_bool(&vcam->active, true);
	os_atomic_set_bool(&vcam->stopping, false);
	blog(LOG_INFO, "Virtual output started");
	obs_output_begin_data_capture(vcam->output, 0);
	return true;
}

static void virtualcam_deactive(struct virtualcam_data *vcam)
{
	obs_output_end_data_capture(vcam->output);
	video_queue_close(vcam->vq);
	vcam->vq = NULL;
	vmic_stop(vcam);

	os_atomic_set_bool(&vcam->active, false);
	os_atomic_set_bool(&vcam->stopping, false);

	blog(LOG_INFO, "Virtual output stopped");
}

static void virtualcam_stop(void *data, uint64_t ts)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;
	os_atomic_set_bool(&vcam->stopping, true);

	blog(LOG_INFO, "Virtual output stopping");

	UNUSED_PARAMETER(ts);
}

static void virtual_video(void *param, struct video_data *frame)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)param;

	if (!vcam->vq) {
		return;
	}

	if (!os_atomic_load_bool(&vcam->active)) {
		return;
	}

	if (os_atomic_load_bool(&vcam->stopping)) {
		virtualcam_deactive(vcam);
		return;
	}

	video_queue_write(vcam->vq, frame->data, frame->linesize, frame->timestamp);
}

static void virtual_audio(void *param, struct audio_data *frames)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)param;
	if (!os_atomic_load_bool(&vcam->render_active)) {
		return;
	}
	pthread_mutex_lock(&vcam->ring_mutex);
	if (vcam->ring.buf) {
		vmic_ring_push(&vcam->ring, (const float *)frames->data[0], (size_t)frames->frames * VMIC_CHANNELS);
	}
	pthread_mutex_unlock(&vcam->ring_mutex);
}

struct obs_output_info virtualcam_info = {
	.id = "virtualcam_output",
	.flags = OBS_OUTPUT_AV,
	.get_name = virtualcam_name,
	.create = virtualcam_create,
	.update = virtualcam_update,
	.destroy = virtualcam_destroy,
	.start = virtualcam_start,
	.stop = virtualcam_stop,
	.raw_video = virtual_video,
	.raw_audio = virtual_audio,
};

#undef VMIC_RATE
#undef VMIC_CHANNELS
#undef VMIC_RING_SECONDS
