#include "plugins/mac-virtualcam/src/audio-plugin/build/audio-plugin.pch.h"

/* OBS Virtual Microphone (obs-rust/obs-studio#3184): an AudioServerPlugIn
 * after Apple's NullAudio sample. One input-only device that coreaudiod
 * offers to every app as a microphone; its samples come from the shared
 * memory ring that the mac-virtualcam output writes (vmic-shm.h). When OBS
 * is not running the device records silence. */

enum {
	kObjectID_PlugIn = kAudioObjectPlugInObject,
	kObjectID_Device = 2,
	kObjectID_Stream = 3,
};

#define VMIC_RATE 48000.0
#define VMIC_CHANNELS 2
#define VMIC_PERIOD 512
#define VMIC_DEVICE_UID "OBSVirtualMicrophone"
#define VMIC_MODEL_UID "OBSVirtualMicrophoneModel"

static pthread_mutex_t vmic_state_mutex = PTHREAD_MUTEX_INITIALIZER;
static AudioServerPlugInHostRef vmic_host;
static UInt32 vmic_refcount = 1;
static UInt32 vmic_io_clients;
static uint64_t vmic_anchor;
static double vmic_ticks_per_frame;

static struct vmic_shm *vmic_ring;
static size_t vmic_ring_size;
static UInt64 vmic_ring_retry;

/* Maps the ring OBS created, if it is there and of a format we read. */
static void vmic_map_ring(void)
{
	int fd = shm_open(VMIC_SHM_NAME, O_RDWR);
	if (fd < 0) {
		return;
	}
	struct stat st;
	if (fstat(fd, &st) == 0 && st.st_size >= (off_t)sizeof(struct vmic_shm)) {
		void *map = mmap(NULL, (size_t)st.st_size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
		if (map != MAP_FAILED) {
			struct vmic_shm *shm = map;
			if (vmic_shm_valid(shm, (size_t)st.st_size) && shm->rate == (uint32_t)VMIC_RATE &&
			    shm->channels == VMIC_CHANNELS) {
				vmic_ring = shm;
				vmic_ring_size = (size_t)st.st_size;
			} else {
				munmap(map, (size_t)st.st_size);
			}
		}
	}
	close(fd);
}

static void vmic_unmap_ring(void)
{
	if (vmic_ring) {
		munmap(vmic_ring, vmic_ring_size);
	}
	vmic_ring = NULL;
	vmic_ring_size = 0;
}

static AudioStreamBasicDescription vmic_format(void)
{
	AudioStreamBasicDescription f = {0};
	f.mSampleRate = VMIC_RATE;
	f.mFormatID = kAudioFormatLinearPCM;
	f.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagsNativeEndian | kAudioFormatFlagIsPacked;
	f.mBytesPerPacket = VMIC_CHANNELS * sizeof(Float32);
	f.mFramesPerPacket = 1;
	f.mBytesPerFrame = VMIC_CHANNELS * sizeof(Float32);
	f.mChannelsPerFrame = VMIC_CHANNELS;
	f.mBitsPerChannel = 32;
	return f;
}

/* Each property function answers one property: with `out` NULL only its
 * size (HasProperty, GetPropertyDataSize), else the data too. */
static OSStatus vmic_put(const void *value, UInt32 value_size, UInt32 cap, UInt32 *size, void *out)
{
	*size = value_size;
	if (!out) {
		return kAudioHardwareNoError;
	}
	if (cap < value_size) {
		return kAudioHardwareBadPropertySizeError;
	}
	memcpy(out, value, value_size);
	return kAudioHardwareNoError;
}

static OSStatus vmic_put_u32(UInt32 value, UInt32 cap, UInt32 *size, void *out)
{
	return vmic_put(&value, sizeof(value), cap, size, out);
}

static OSStatus vmic_put_cfstring(CFStringRef value, UInt32 cap, UInt32 *size, void *out)
{
	return vmic_put(&value, sizeof(value), cap, size, out);
}

/* An object list, truncated to what the caller's buffer holds. */
static OSStatus vmic_put_ids(const AudioObjectID *ids, UInt32 count, UInt32 cap, UInt32 *size, void *out)
{
	if (out && count > cap / sizeof(AudioObjectID)) {
		count = cap / sizeof(AudioObjectID);
	}
	*size = count * (UInt32)sizeof(AudioObjectID);
	if (out && count) {
		memcpy(out, ids, *size);
	}
	return kAudioHardwareNoError;
}

static OSStatus vmic_plugin_property(const AudioObjectPropertyAddress *a, UInt32 qsize, const void *qual, UInt32 cap,
				     UInt32 *size, void *out)
{
	static const AudioObjectID devices[] = {kObjectID_Device};

	switch (a->mSelector) {
	case kAudioObjectPropertyBaseClass:
		return vmic_put_u32(kAudioObjectClassID, cap, size, out);
	case kAudioObjectPropertyClass:
		return vmic_put_u32(kAudioPlugInClassID, cap, size, out);
	case kAudioObjectPropertyOwner:
		return vmic_put_u32(kAudioObjectUnknown, cap, size, out);
	case kAudioObjectPropertyManufacturer:
		return vmic_put_cfstring(CFSTR("OBS Project"), cap, size, out);
	case kAudioObjectPropertyOwnedObjects:
	case kAudioPlugInPropertyDeviceList:
		return vmic_put_ids(devices, 1, cap, size, out);
	case kAudioPlugInPropertyTranslateUIDToDevice: {
		AudioObjectID id = kAudioObjectUnknown;
		if (qual && qsize == sizeof(CFStringRef) &&
		    CFStringCompare(*(const CFStringRef *)qual, CFSTR(VMIC_DEVICE_UID), 0) == kCFCompareEqualTo) {
			id = kObjectID_Device;
		}
		return vmic_put_u32(id, cap, size, out);
	}
	case kAudioPlugInPropertyResourceBundle:
		return vmic_put_cfstring(CFSTR(""), cap, size, out);
	default:
		return kAudioHardwareUnknownPropertyError;
	}
}

static OSStatus vmic_device_property(const AudioObjectPropertyAddress *a, UInt32 cap, UInt32 *size, void *out)
{
	static const AudioObjectID streams[] = {kObjectID_Stream};
	static const AudioObjectID related[] = {kObjectID_Device};
	bool input = a->mScope == kAudioObjectPropertyScopeGlobal || a->mScope == kAudioObjectPropertyScopeInput;

	switch (a->mSelector) {
	case kAudioObjectPropertyBaseClass:
		return vmic_put_u32(kAudioObjectClassID, cap, size, out);
	case kAudioObjectPropertyClass:
		return vmic_put_u32(kAudioDeviceClassID, cap, size, out);
	case kAudioObjectPropertyOwner:
		return vmic_put_u32(kObjectID_PlugIn, cap, size, out);
	case kAudioObjectPropertyName:
		return vmic_put_cfstring(CFSTR("OBS Virtual Microphone"), cap, size, out);
	case kAudioObjectPropertyManufacturer:
		return vmic_put_cfstring(CFSTR("OBS Project"), cap, size, out);
	case kAudioObjectPropertyOwnedObjects:
	case kAudioDevicePropertyStreams:
		return vmic_put_ids(streams, input ? 1 : 0, cap, size, out);
	case kAudioObjectPropertyControlList:
		return vmic_put_ids(streams, 0, cap, size, out);
	case kAudioDevicePropertyDeviceUID:
		return vmic_put_cfstring(CFSTR(VMIC_DEVICE_UID), cap, size, out);
	case kAudioDevicePropertyModelUID:
		return vmic_put_cfstring(CFSTR(VMIC_MODEL_UID), cap, size, out);
	case kAudioDevicePropertyTransportType:
		return vmic_put_u32(kAudioDeviceTransportTypeVirtual, cap, size, out);
	case kAudioDevicePropertyRelatedDevices:
		return vmic_put_ids(related, 1, cap, size, out);
	case kAudioDevicePropertyClockDomain:
	case kAudioDevicePropertyLatency:
	case kAudioDevicePropertySafetyOffset:
	case kAudioDevicePropertyIsHidden:
	case kAudioDevicePropertyDeviceCanBeDefaultSystemDevice:
		return vmic_put_u32(0, cap, size, out);
	case kAudioDevicePropertyDeviceIsAlive:
		return vmic_put_u32(1, cap, size, out);
	case kAudioDevicePropertyDeviceIsRunning:
		return vmic_put_u32(vmic_io_clients > 0, cap, size, out);
	case kAudioDevicePropertyDeviceCanBeDefaultDevice:
		return vmic_put_u32(input ? 1 : 0, cap, size, out);
	case kAudioDevicePropertyNominalSampleRate: {
		Float64 rate = VMIC_RATE;
		return vmic_put(&rate, sizeof(rate), cap, size, out);
	}
	case kAudioDevicePropertyAvailableNominalSampleRates: {
		AudioValueRange range = {VMIC_RATE, VMIC_RATE};
		return vmic_put(&range, sizeof(range), cap, size, out);
	}
	case kAudioDevicePropertyZeroTimeStampPeriod:
		return vmic_put_u32(VMIC_PERIOD, cap, size, out);
	case kAudioDevicePropertyPreferredChannelsForStereo: {
		UInt32 channels[2] = {1, 2};
		return vmic_put(channels, sizeof(channels), cap, size, out);
	}
	default:
		return kAudioHardwareUnknownPropertyError;
	}
}

static OSStatus vmic_stream_property(const AudioObjectPropertyAddress *a, UInt32 cap, UInt32 *size, void *out)
{
	switch (a->mSelector) {
	case kAudioObjectPropertyBaseClass:
		return vmic_put_u32(kAudioObjectClassID, cap, size, out);
	case kAudioObjectPropertyClass:
		return vmic_put_u32(kAudioStreamClassID, cap, size, out);
	case kAudioObjectPropertyOwner:
		return vmic_put_u32(kObjectID_Device, cap, size, out);
	case kAudioObjectPropertyOwnedObjects:
		return vmic_put_ids(NULL, 0, cap, size, out);
	case kAudioObjectPropertyName:
		return vmic_put_cfstring(CFSTR("OBS Virtual Microphone"), cap, size, out);
	case kAudioStreamPropertyIsActive:
	case kAudioStreamPropertyDirection: /* 1: input */
	case kAudioStreamPropertyStartingChannel:
		return vmic_put_u32(1, cap, size, out);
	case kAudioStreamPropertyTerminalType:
		return vmic_put_u32(kAudioStreamTerminalTypeMicrophone, cap, size, out);
	case kAudioStreamPropertyLatency:
		return vmic_put_u32(0, cap, size, out);
	case kAudioStreamPropertyVirtualFormat:
	case kAudioStreamPropertyPhysicalFormat: {
		AudioStreamBasicDescription f = vmic_format();
		return vmic_put(&f, sizeof(f), cap, size, out);
	}
	case kAudioStreamPropertyAvailableVirtualFormats:
	case kAudioStreamPropertyAvailablePhysicalFormats: {
		AudioStreamRangedDescription d;
		d.mFormat = vmic_format();
		d.mSampleRateRange.mMinimum = VMIC_RATE;
		d.mSampleRateRange.mMaximum = VMIC_RATE;
		return vmic_put(&d, sizeof(d), cap, size, out);
	}
	default:
		return kAudioHardwareUnknownPropertyError;
	}
}

static OSStatus vmic_property(AudioObjectID id, const AudioObjectPropertyAddress *a, UInt32 qsize, const void *qual,
			      UInt32 cap, UInt32 *size, void *out)
{
	switch (id) {
	case kObjectID_PlugIn:
		return vmic_plugin_property(a, qsize, qual, cap, size, out);
	case kObjectID_Device:
		return vmic_device_property(a, cap, size, out);
	case kObjectID_Stream:
		return vmic_stream_property(a, cap, size, out);
	default:
		return kAudioHardwareBadObjectError;
	}
}

/* The driver interface. */

static HRESULT vmic_QueryInterface(void *driver, REFIID uuid, LPVOID *out);
static ULONG vmic_AddRef(void *driver);
static ULONG vmic_Release(void *driver);

static OSStatus vmic_Initialize(AudioServerPlugInDriverRef d, AudioServerPlugInHostRef host)
{
	(void)d;
	vmic_host = host;
	mach_timebase_info_data_t tb;
	mach_timebase_info(&tb);
	double ticks_per_second = 1e9 * (double)tb.denom / (double)tb.numer;
	vmic_ticks_per_frame = ticks_per_second / VMIC_RATE;
	return kAudioHardwareNoError;
}

static OSStatus vmic_CreateDevice(AudioServerPlugInDriverRef d, CFDictionaryRef desc,
				  const AudioServerPlugInClientInfo *client, AudioObjectID *out)
{
	(void)d;
	(void)desc;
	(void)client;
	(void)out;
	return kAudioHardwareUnsupportedOperationError;
}

static OSStatus vmic_DestroyDevice(AudioServerPlugInDriverRef d, AudioObjectID id)
{
	(void)d;
	(void)id;
	return kAudioHardwareUnsupportedOperationError;
}

static OSStatus vmic_AddDeviceClient(AudioServerPlugInDriverRef d, AudioObjectID id,
				     const AudioServerPlugInClientInfo *client)
{
	(void)d;
	(void)id;
	(void)client;
	return kAudioHardwareNoError;
}

static OSStatus vmic_RemoveDeviceClient(AudioServerPlugInDriverRef d, AudioObjectID id,
					const AudioServerPlugInClientInfo *client)
{
	(void)d;
	(void)id;
	(void)client;
	return kAudioHardwareNoError;
}

static OSStatus vmic_PerformDeviceConfigurationChange(AudioServerPlugInDriverRef d, AudioObjectID id, UInt64 action,
						      void *info)
{
	(void)d;
	(void)id;
	(void)action;
	(void)info;
	return kAudioHardwareNoError;
}

static OSStatus vmic_AbortDeviceConfigurationChange(AudioServerPlugInDriverRef d, AudioObjectID id, UInt64 action,
						    void *info)
{
	(void)d;
	(void)id;
	(void)action;
	(void)info;
	return kAudioHardwareNoError;
}

static Boolean vmic_HasProperty(AudioServerPlugInDriverRef d, AudioObjectID id, pid_t pid,
				const AudioObjectPropertyAddress *a)
{
	(void)d;
	(void)pid;
	UInt32 size = 0;
	return vmic_property(id, a, 0, NULL, 0, &size, NULL) == kAudioHardwareNoError;
}

static OSStatus vmic_IsPropertySettable(AudioServerPlugInDriverRef d, AudioObjectID id, pid_t pid,
					const AudioObjectPropertyAddress *a, Boolean *out)
{
	(void)d;
	(void)pid;
	UInt32 size = 0;
	OSStatus err = vmic_property(id, a, 0, NULL, 0, &size, NULL);
	if (err == kAudioHardwareNoError) {
		*out = false;
	}
	return err;
}

static OSStatus vmic_GetPropertyDataSize(AudioServerPlugInDriverRef d, AudioObjectID id, pid_t pid,
					 const AudioObjectPropertyAddress *a, UInt32 qsize, const void *qual,
					 UInt32 *out)
{
	(void)d;
	(void)pid;
	return vmic_property(id, a, qsize, qual, 0, out, NULL);
}

static OSStatus vmic_GetPropertyData(AudioServerPlugInDriverRef d, AudioObjectID id, pid_t pid,
				     const AudioObjectPropertyAddress *a, UInt32 qsize, const void *qual, UInt32 cap,
				     UInt32 *size, void *out)
{
	(void)d;
	(void)pid;
	pthread_mutex_lock(&vmic_state_mutex);
	OSStatus err = vmic_property(id, a, qsize, qual, cap, size, out);
	pthread_mutex_unlock(&vmic_state_mutex);
	return err;
}

static OSStatus vmic_SetPropertyData(AudioServerPlugInDriverRef d, AudioObjectID id, pid_t pid,
				     const AudioObjectPropertyAddress *a, UInt32 qsize, const void *qual, UInt32 size,
				     const void *data)
{
	(void)d;
	(void)id;
	(void)pid;
	(void)a;
	(void)qsize;
	(void)qual;
	(void)size;
	(void)data;
	return kAudioHardwareUnsupportedOperationError;
}

static OSStatus vmic_StartIO(AudioServerPlugInDriverRef d, AudioObjectID id, UInt32 client)
{
	(void)d;
	(void)client;
	if (id != kObjectID_Device) {
		return kAudioHardwareBadObjectError;
	}
	pthread_mutex_lock(&vmic_state_mutex);
	if (vmic_io_clients++ == 0) {
		vmic_anchor = mach_absolute_time();
		vmic_map_ring();
	}
	pthread_mutex_unlock(&vmic_state_mutex);
	return kAudioHardwareNoError;
}

static OSStatus vmic_StopIO(AudioServerPlugInDriverRef d, AudioObjectID id, UInt32 client)
{
	(void)d;
	(void)client;
	if (id != kObjectID_Device) {
		return kAudioHardwareBadObjectError;
	}
	pthread_mutex_lock(&vmic_state_mutex);
	if (vmic_io_clients > 0 && --vmic_io_clients == 0) {
		vmic_unmap_ring();
	}
	pthread_mutex_unlock(&vmic_state_mutex);
	return kAudioHardwareNoError;
}

static OSStatus vmic_GetZeroTimeStamp(AudioServerPlugInDriverRef d, AudioObjectID id, UInt32 client,
				      Float64 *sample_time, UInt64 *host_time, UInt64 *seed)
{
	(void)d;
	(void)id;
	(void)client;
	struct vmic_zero_timestamp ts =
		vmic_clock_zero(vmic_anchor, mach_absolute_time(), vmic_ticks_per_frame, VMIC_PERIOD);
	*sample_time = ts.sample_time;
	*host_time = ts.host_time;
	*seed = ts.seed;
	return kAudioHardwareNoError;
}

static OSStatus vmic_WillDoIOOperation(AudioServerPlugInDriverRef d, AudioObjectID id, UInt32 client, UInt32 op,
				       Boolean *will_do, Boolean *in_place)
{
	(void)d;
	(void)id;
	(void)client;
	*will_do = op == kAudioServerPlugInIOOperationReadInput;
	*in_place = true;
	return kAudioHardwareNoError;
}

static OSStatus vmic_BeginIOOperation(AudioServerPlugInDriverRef d, AudioObjectID id, UInt32 client, UInt32 op,
				      UInt32 frames, const AudioServerPlugInIOCycleInfo *cycle)
{
	(void)d;
	(void)id;
	(void)client;
	(void)op;
	(void)frames;
	(void)cycle;
	return kAudioHardwareNoError;
}

static OSStatus vmic_DoIOOperation(AudioServerPlugInDriverRef d, AudioObjectID id, AudioObjectID stream, UInt32 client,
				   UInt32 op, UInt32 frames, const AudioServerPlugInIOCycleInfo *cycle,
				   void *main_buffer, void *secondary_buffer)
{
	(void)d;
	(void)id;
	(void)stream;
	(void)client;
	(void)cycle;
	(void)secondary_buffer;
	if (op != kAudioServerPlugInIOOperationReadInput) {
		return kAudioHardwareNoError;
	}

	/* OBS may start after an app opened the microphone: look for the
	 * ring about once a second, without ever blocking the IO thread. */
	if (!vmic_ring && (vmic_ring_retry += frames) >= (UInt64)VMIC_RATE &&
	    pthread_mutex_trylock(&vmic_state_mutex) == 0) {
		vmic_ring_retry = 0;
		vmic_map_ring();
		pthread_mutex_unlock(&vmic_state_mutex);
	}

	if (vmic_ring) {
		vmic_shm_read(vmic_ring, (float *)main_buffer, frames);
	} else {
		memset(main_buffer, 0, (size_t)frames * VMIC_CHANNELS * sizeof(Float32));
	}
	return kAudioHardwareNoError;
}

static OSStatus vmic_EndIOOperation(AudioServerPlugInDriverRef d, AudioObjectID id, UInt32 client, UInt32 op,
				    UInt32 frames, const AudioServerPlugInIOCycleInfo *cycle)
{
	(void)d;
	(void)id;
	(void)client;
	(void)op;
	(void)frames;
	(void)cycle;
	return kAudioHardwareNoError;
}

static AudioServerPlugInDriverInterface vmic_interface = {
	NULL,
	vmic_QueryInterface,
	vmic_AddRef,
	vmic_Release,
	vmic_Initialize,
	vmic_CreateDevice,
	vmic_DestroyDevice,
	vmic_AddDeviceClient,
	vmic_RemoveDeviceClient,
	vmic_PerformDeviceConfigurationChange,
	vmic_AbortDeviceConfigurationChange,
	vmic_HasProperty,
	vmic_IsPropertySettable,
	vmic_GetPropertyDataSize,
	vmic_GetPropertyData,
	vmic_SetPropertyData,
	vmic_StartIO,
	vmic_StopIO,
	vmic_GetZeroTimeStamp,
	vmic_WillDoIOOperation,
	vmic_BeginIOOperation,
	vmic_DoIOOperation,
	vmic_EndIOOperation,
};
static AudioServerPlugInDriverInterface *vmic_interface_ptr = &vmic_interface;
static AudioServerPlugInDriverRef vmic_driver = &vmic_interface_ptr;

static HRESULT vmic_QueryInterface(void *driver, REFIID uuid, LPVOID *out)
{
	CFUUIDRef requested = CFUUIDCreateFromUUIDBytes(NULL, uuid);
	HRESULT result = E_NOINTERFACE;
	if (driver == vmic_driver && requested &&
	    (CFEqual(requested, IUnknownUUID) || CFEqual(requested, kAudioServerPlugInDriverInterfaceUUID))) {
		vmic_AddRef(driver);
		*out = vmic_driver;
		result = S_OK;
	}
	if (requested) {
		CFRelease(requested);
	}
	return result;
}

static ULONG vmic_AddRef(void *driver)
{
	(void)driver;
	pthread_mutex_lock(&vmic_state_mutex);
	ULONG n = ++vmic_refcount;
	pthread_mutex_unlock(&vmic_state_mutex);
	return n;
}

static ULONG vmic_Release(void *driver)
{
	(void)driver;
	pthread_mutex_lock(&vmic_state_mutex);
	ULONG n = vmic_refcount > 0 ? --vmic_refcount : 0;
	pthread_mutex_unlock(&vmic_state_mutex);
	return n;
}

/* The CFPlugIn factory named in Info.plist. */
__attribute__((visibility("default"))) void *OBSVirtualMicrophoneCreate(CFAllocatorRef allocator, CFUUIDRef type)
{
	(void)allocator;
	return CFEqual(type, kAudioServerPlugInTypeUUID) ? vmic_driver : NULL;
}

#undef VMIC_RATE
#undef VMIC_CHANNELS
#undef VMIC_PERIOD
#undef VMIC_DEVICE_UID
#undef VMIC_MODEL_UID
