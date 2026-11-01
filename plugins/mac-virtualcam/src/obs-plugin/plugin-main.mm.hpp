#include "plugins/mac-virtualcam/src/obs-plugin/build/mac-virtualcam.pch.hpp"

OBS_DECLARE_MODULE()
OBS_MODULE_USE_DEFAULT_LOCALE("mac-virtualcam", "en-US")

MODULE_EXPORT const char *obs_module_description(void)
{
	return "macOS virtual webcam output";
}

NSString *const OBSDalDestination = @"/Library/CoreMediaIO/Plug-Ins/DAL";

static bool cmio_extension_supported()
{
	if (@available(macOS 13.0, *)) {
		return true;
	} else {
		return false;
	}
}

struct virtualcam_data {
	obs_output_t *output;
	obs_video_info videoInfo;
	CVPixelBufferPoolRef pool;

	// CMIO Extension (available with macOS 13)
	CMSimpleQueueRef queue;
	CMIODeviceID deviceID;
	CMIOStreamID streamID;
	CMFormatDescriptionRef formatDescription;
	id extensionDelegate;

	// Legacy DAL (deprecated since macOS 12.3)
	OBSDALMachServer *machServer;

	// OBS Virtual Microphone (#3184): the ring the HAL plug-in reads
	bool audio_enabled;
	size_t audio_mixer;
	struct vmic_shm *mic;
	size_t mic_size;
};

#define VMIC_DRIVER_PATH "/Library/Audio/Plug-Ins/HAL/OBSVirtualMicrophone.driver"
#define VMIC_RATE 48000
#define VMIC_CHANNELS 2

static void vmic_read_settings(struct virtualcam_data *vcam, obs_data_t *settings)
{
	vcam->audio_enabled = !obs_data_has_user_value(settings, "audio_enabled") ||
			      obs_data_get_bool(settings, "audio_enabled");
	long long track = obs_data_get_int(settings, "audio_track");
	vcam->audio_mixer = track >= 1 && track <= MAX_AUDIO_MIXES ? (size_t)(track - 1) : 0;
}

/* Creates (or reuses) the ring for the HAL plug-in. The segment is never
 * unlinked, so a plug-in that mapped it keeps reading after OBS restarts the
 * camera; it is readable by coreaudiod's own user. */
/* The bundle versions of an installed or bundled OBSVirtualMicrophone.driver;
 * both nil when there is none. */
static void vmic_driver_versions(NSString *path, NSString **version, NSString **build)
{
	NSDictionary *info =
		[NSDictionary dictionaryWithContentsOfURL:
				      [NSURL fileURLWithPath:[path stringByAppendingString:@"/Contents/Info.plist"]]];
	*version = [info valueForKey:@"CFBundleShortVersionString"];
	*build = [info valueForKey:(NSString *)kCFBundleVersionKey];
}

/* Installs or updates the OBS Virtual Microphone from OBS.app's Resources
 * (#3341): one admin prompt copies it into /Library/Audio/Plug-Ins/HAL and
 * restarts coreaudiod so apps see the device. A cancelled prompt leaves the
 * camera video-only. */
static void vmic_ensure_driver(void)
{
	NSString *installed = @VMIC_DRIVER_PATH;
	NSString *bundled =
		[[[NSBundle mainBundle] resourcePath] stringByAppendingPathComponent:@"OBSVirtualMicrophone.driver"];
	NSString *installedVersion, *installedBuild, *bundledVersion, *bundledBuild;
	vmic_driver_versions(installed, &installedVersion, &installedBuild);
	vmic_driver_versions(bundled, &bundledVersion, &bundledBuild);

	enum vmic_driver_action action = vmic_driver_action(installedVersion.UTF8String, installedBuild.UTF8String,
							    bundledVersion.UTF8String, bundledBuild.UTF8String);
	if (action != VMIC_DRIVER_INSTALL && action != VMIC_DRIVER_UPDATE) {
		return;
	}

	NSString *cmd = [NSString
		stringWithFormat:
			@"do shell script \"mkdir -p '%@' && rm -rf '%@' && cp -R '%@' '%@' && "
			@"launchctl kickstart -k system/com.apple.audio.coreaudiod\" with administrator privileges",
			[installed stringByDeletingLastPathComponent], installed, bundled, installed];
	NSDictionary *errorDict = nil;
	[[[NSAppleScript alloc] initWithSource:cmd] executeAndReturnError:&errorDict];
	if (errorDict) {
		blog(LOG_WARNING, "Virtual camera audio: the OBS Virtual Microphone was not %s: %s",
		     action == VMIC_DRIVER_INSTALL ? "installed" : "updated",
		     [[errorDict objectForKey:NSAppleScriptErrorMessage] UTF8String]);
	} else {
		blog(LOG_INFO, "Virtual camera audio: %s the OBS Virtual Microphone %s (%s)",
		     action == VMIC_DRIVER_INSTALL ? "installed" : "updated", bundledVersion.UTF8String,
		     bundledBuild.UTF8String);
	}
}

static void vmic_start(struct virtualcam_data *vcam)
{
	if (!vcam->audio_enabled) {
		return;
	}
	vmic_ensure_driver();
	if (access(VMIC_DRIVER_PATH, F_OK) != 0) {
		blog(LOG_WARNING, "Virtual camera audio: the OBS Virtual Microphone is not installed (" VMIC_DRIVER_PATH
				  "), video only");
		return;
	}

	size_t size = vmic_shm_size(VMIC_CHANNELS, VMIC_RATE);
	int fd = shm_open(VMIC_SHM_NAME, O_RDWR | O_CREAT, 0666);
	if (fd < 0) {
		blog(LOG_WARNING, "Virtual camera audio: shm_open failed (%d), video only", errno);
		return;
	}
	fchmod(fd, 0666);
	/* macOS sizes a segment only once, when it is new. */
	struct stat st;
	if (fstat(fd, &st) == 0 && st.st_size == 0) {
		ftruncate(fd, (off_t)size);
	}
	if (fstat(fd, &st) != 0 || (size_t)st.st_size < size) {
		blog(LOG_WARNING, "Virtual camera audio: cannot size the ring (%d), video only", errno);
		close(fd);
		return;
	}
	void *map = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
	close(fd);
	if (map == MAP_FAILED) {
		blog(LOG_WARNING, "Virtual camera audio: mmap failed (%d), video only", errno);
		return;
	}
	vcam->mic = (struct vmic_shm *)map;
	vcam->mic_size = size;
	vmic_shm_init(vcam->mic, VMIC_RATE, VMIC_CHANNELS, VMIC_RATE);

	struct audio_convert_info aci = {};
	aci.samples_per_sec = VMIC_RATE;
	aci.format = AUDIO_FORMAT_FLOAT;
	aci.speakers = SPEAKERS_STEREO;
	obs_output_set_audio_conversion(vcam->output, &aci);
	obs_output_set_mixer(vcam->output, vcam->audio_mixer);
	blog(LOG_INFO, "Virtual camera audio: track %zu to the OBS Virtual Microphone", vcam->audio_mixer + 1);
}

static void vmic_stop(struct virtualcam_data *vcam)
{
	if (vcam->mic) {
		munmap(vcam->mic, vcam->mic_size);
	}
	vcam->mic = NULL;
	vcam->mic_size = 0;
}

static void virtualcam_audio_proc(void *data, calldata_t *cd)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;
	calldata_set_bool(cd, "active", vcam->mic != NULL);
}

@interface SystemExtensionActivationDelegate : NSObject <OSSystemExtensionRequestDelegate> {
@private
	struct virtualcam_data *_vcam;
}

@property (getter=isInstalled) BOOL installed;
@property NSString *lastErrorMessage;
- (instancetype)init __unavailable;
@end

@implementation SystemExtensionActivationDelegate

- (id)initWithVcam:(virtualcam_data *)vcam
{
	self = [super init];

	if (self) {
		_vcam = vcam;
		_installed = NO;
	}

	return self;
}

- (OSSystemExtensionReplacementAction)request:(nonnull OSSystemExtensionRequest *)request
		  actionForReplacingExtension:(nonnull OSSystemExtensionProperties *)existing
				withExtension:(nonnull OSSystemExtensionProperties *)ext
{
	NSString *infoString = [NSString
		stringWithFormat:
			@"mac-camera-extension: Replacement requested. Existing version: %@ (%@), new version: %@ (%@). Replacing...",
			existing.bundleShortVersion, existing.bundleVersion, ext.bundleShortVersion, ext.bundleVersion];
	blog(LOG_INFO, "%s", infoString.UTF8String);
	return OSSystemExtensionReplacementActionReplace;
}

- (void)request:(nonnull OSSystemExtensionRequest *)request didFailWithError:(nonnull NSError *)error
{
	NSString *errorMessage;
	int severity;

	switch (error.code) {
	case OSSystemExtensionErrorUnsupportedParentBundleLocation:
		self.lastErrorMessage =
			[NSString stringWithUTF8String:obs_module_text("Error.SystemExtension.WrongLocation")];
		errorMessage = self.lastErrorMessage;
		severity = LOG_WARNING;
		break;
	default:
		self.lastErrorMessage = error.localizedDescription;
		errorMessage = [NSString stringWithFormat:@"OSSystemExtensionErrorCode %ld (\"%s\")", error.code,
							  error.localizedDescription.UTF8String];
		severity = LOG_ERROR;
		break;
	}

	blog(severity, "mac-camera-extension: %s", errorMessage.UTF8String);
}

- (void)request:(nonnull OSSystemExtensionRequest *)request didFinishWithResult:(OSSystemExtensionRequestResult)result
{
	switch (result) {
	case OSSystemExtensionRequestCompleted:
		self.installed = YES;
		blog(LOG_INFO, "macOS Camera Extension activated successfully.");
		break;
	case OSSystemExtensionRequestWillCompleteAfterReboot:
		self.lastErrorMessage =
			[NSString stringWithUTF8String:obs_module_text("Error.SystemExtension.CompleteAfterReboot")];
		blog(LOG_INFO, "macOS Camera Extension will activate after reboot.");
		break;
	}
}

- (void)requestNeedsUserApproval:(nonnull OSSystemExtensionRequest *)request
{
	self.installed = NO;
	blog(LOG_INFO, "macOS Camera Extension user approval required.");
}

@end

static void install_cmio_system_extension(struct virtualcam_data *vcam)
{
	OSSystemExtensionRequest *request = [OSSystemExtensionRequest
		activationRequestForExtension:@OBS_CAMERA_EXTENSION_ID
					queue:dispatch_get_main_queue()];
	request.delegate = vcam->extensionDelegate;

	[[OSSystemExtensionManager sharedManager] submitRequest:request];
}

typedef enum { OBSDalPluginNotInstalled, OBSDalPluginInstalled, OBSDalPluginNeedsUpdate } dal_plugin_status;

static dal_plugin_status check_dal_plugin()
{
	NSFileManager *fileManager = [NSFileManager defaultManager];

	NSString *dalPluginFileName = [OBSDalDestination stringByAppendingString:@"/obs-mac-virtualcam.plugin"];

	BOOL dalPluginInstalled = [fileManager fileExistsAtPath:dalPluginFileName];

	if (dalPluginInstalled) {
		NSDictionary *dalPluginInfoPlist = [NSDictionary
			dictionaryWithContentsOfURL:
				[NSURL fileURLWithPath:
						[OBSDalDestination
							stringByAppendingString:
								@"/obs-mac-virtualcam.plugin/Contents/Info.plist"]]];

		NSString *dalPluginVersion = [dalPluginInfoPlist valueForKey:@"CFBundleShortVersionString"];
		NSString *dalPluginBuild = [dalPluginInfoPlist valueForKey:@"CFBundleVersion"];

		NSString *obsVersion =
			[[[NSBundle mainBundle] infoDictionary] objectForKey:@"CFBundleShortVersionString"];
		NSString *obsBuild =
			[[[NSBundle mainBundle] infoDictionary] objectForKey:(NSString *)kCFBundleVersionKey];
		BOOL dalPluginUpdateNeeded =
			!([dalPluginVersion isEqualToString:obsVersion] && [dalPluginBuild isEqualToString:obsBuild]);

		return dalPluginUpdateNeeded ? OBSDalPluginNeedsUpdate : OBSDalPluginInstalled;
	}

	return OBSDalPluginNotInstalled;
}

static bool install_dal_plugin(bool update)
{
	NSFileManager *fileManager = [NSFileManager defaultManager];
	BOOL dalPluginDirExists = [fileManager fileExistsAtPath:OBSDalDestination];

	NSURL *bundleURL = [[NSBundle mainBundle] bundleURL];
	NSString *pluginPath = @"Contents/Resources/obs-mac-virtualcam.plugin";

	NSURL *pluginUrl = [bundleURL URLByAppendingPathComponent:pluginPath];
	NSString *dalPluginSourcePath = [pluginUrl path];

	NSString *createPluginDirCmd =
		(!dalPluginDirExists) ? [NSString stringWithFormat:@"mkdir -p '%@' && ", OBSDalDestination] : @"";
	NSString *deleteOldPluginCmd = (update) ? [NSString stringWithFormat:@"rm -rf '%@' && ", OBSDalDestination]
						: @"";
	NSString *copyPluginCmd =
		[NSString stringWithFormat:@"cp -R '%@' '%@'", dalPluginSourcePath, OBSDalDestination];

	if ([fileManager fileExistsAtPath:dalPluginSourcePath]) {
		NSString *copyCmd =
			[NSString stringWithFormat:@"do shell script \"%@%@%@\" with administrator privileges",
						   createPluginDirCmd, deleteOldPluginCmd, copyPluginCmd];

		NSDictionary *errorDict;
		NSAppleScript *scriptObject = [[NSAppleScript alloc] initWithSource:copyCmd];
		[scriptObject executeAndReturnError:&errorDict];
		if (errorDict != nil) {
			const char *errorMessage = [[errorDict objectForKey:@"NSAppleScriptErrorMessage"] UTF8String];

			blog(LOG_INFO, "[macOS] VirtualCam DAL Plugin Installation status: %s", errorMessage);
			return false;
		} else {
			return true;
		}
	} else {
		blog(LOG_INFO, "[macOS] VirtualCam DAL Plugin not shipped with OBS");
		return false;
	}
}

static bool uninstall_dal_plugin()
{
	NSAppleScript *scriptObject = [[NSAppleScript alloc]
		initWithSource:
			[NSString
				stringWithFormat:
					@"do shell script \"rm -rf %@/obs-mac-virtualcam.plugin\" with administrator privileges",
					OBSDalDestination]];

	NSDictionary *errorDict;

	[scriptObject executeAndReturnError:&errorDict];
	if (errorDict) {
		blog(LOG_INFO, "[macOS] VirtualCam DAL Plugin could not be uninstalled: %s",
		     [[errorDict objectForKey:NSAppleScriptErrorMessage] UTF8String]);
		return false;
	} else {
		return true;
	}
}

FourCharCode convert_video_format_to_mac(enum video_format format, enum video_range_type range)
{
	switch (format) {
	case VIDEO_FORMAT_I420:
		return (range == VIDEO_RANGE_FULL) ? kCVPixelFormatType_420YpCbCr8PlanarFullRange
						   : kCVPixelFormatType_420YpCbCr8Planar;
	case VIDEO_FORMAT_NV12:
		return (range == VIDEO_RANGE_FULL) ? kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
						   : kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange;
	case VIDEO_FORMAT_UYVY:
		return (range == VIDEO_RANGE_FULL) ? kCVPixelFormatType_422YpCbCr8FullRange
						   : kCVPixelFormatType_422YpCbCr8;
	case VIDEO_FORMAT_P010:
		return (range == VIDEO_RANGE_FULL) ? kCVPixelFormatType_420YpCbCr10BiPlanarFullRange
						   : kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange;
	default:
		// Zero indicates that the format is not supported on macOS
		// Note that some formats do have an associated constant, but
		// constructing such formats fails with kCVReturnInvalidPixelFormat.
		return 0;
	}
}

static const char *virtualcam_output_get_name(void *type_data)
{
	(void)type_data;
	return obs_module_text("Plugin_Name");
}

static void *virtualcam_output_create(obs_data_t *settings, obs_output_t *output)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)bzalloc(sizeof(*vcam));

	vcam->output = output;
	vmic_read_settings(vcam, settings);

	/* The frontend asks after start whether audio runs, to tell the user
	 * when the camera fell back to video only (#3182). */
	if (output) {
		proc_handler_add(obs_output_get_proc_handler(output), "void virtualcam_audio(out bool active)",
				 virtualcam_audio_proc, vcam);
	}

	if (cmio_extension_supported()) {
		vcam->extensionDelegate = [[SystemExtensionActivationDelegate alloc] initWithVcam:vcam];
		install_cmio_system_extension(vcam);
	} else {
		vcam->machServer = [[OBSDALMachServer alloc] init];
	}

	return vcam;
}

static void virtualcam_output_destroy(void *data)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;

	if (cmio_extension_supported()) {
		vcam->extensionDelegate = nil;
	} else {
		vcam->machServer = nil;
	}
	vmic_stop(vcam);

	bfree(vcam);
}

static void virtualcam_output_update(void *data, obs_data_t *settings)
{
	vmic_read_settings((struct virtualcam_data *)data, settings);
}

static bool virtualcam_output_start(void *data)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;

	dal_plugin_status dal_status = check_dal_plugin();

	if (cmio_extension_supported()) {
		if (dal_status != OBSDalPluginNotInstalled) {
			if (!uninstall_dal_plugin()) {
				obs_output_set_last_error(vcam->output, obs_module_text("Error.DAL.NotUninstalled"));
				return false;
			}
		}

		SystemExtensionActivationDelegate *delegate = vcam->extensionDelegate;

		if (!delegate.installed) {
			if (delegate.lastErrorMessage) {
				obs_output_set_last_error(
					vcam->output,
					[NSString stringWithFormat:@"%s\n\n%@",
								   obs_module_text(
									   "Error.SystemExtension.InstallationError"),
								   delegate.lastErrorMessage]
						.UTF8String);
			} else {
				if (@available(macOS 15.0, *)) {
					obs_output_set_last_error(
						vcam->output,
						obs_module_text("Error.SystemExtension.NotInstalled.MacOS15"));
				} else {
					obs_output_set_last_error(
						vcam->output, obs_module_text("Error.SystemExtension.NotInstalled"));
				}
			}

			return false;
		}
	} else {
		bool success = false;
		if (dal_status == OBSDalPluginNotInstalled) {
			success = install_dal_plugin(false);
		} else if (dal_status == OBSDalPluginNeedsUpdate) {
			success = install_dal_plugin(true);
		} else {
			success = true;
		}

		if (!success) {
			obs_output_set_last_error(vcam->output, "Error.DAL.NotInstalled");
			return false;
		}
	}

	obs_get_video_info(&vcam->videoInfo);

	FourCharCode video_format = convert_video_format_to_mac(vcam->videoInfo.output_format, vcam->videoInfo.range);

	struct video_scale_info conversion = {};
	conversion.width = vcam->videoInfo.output_width;
	conversion.height = vcam->videoInfo.output_height;
	conversion.colorspace = vcam->videoInfo.colorspace;
	conversion.range = vcam->videoInfo.range;

	if (!video_format) {
		// Selected output format is not supported natively by CoreVideo, CPU conversion necessary
		blog(LOG_WARNING, "Selected output format (%s) not supported by CoreVideo, enabling CPU transcoding...",
		     get_video_format_name(vcam->videoInfo.output_format));

		conversion.format = VIDEO_FORMAT_NV12;
		video_format = convert_video_format_to_mac(conversion.format, conversion.range);
	} else {
		conversion.format = vcam->videoInfo.output_format;
	}
	obs_output_set_video_conversion(vcam->output, &conversion);

	NSDictionary *pAttr = @{};
	NSDictionary *pbAttr = @{
		(id)kCVPixelBufferPixelFormatTypeKey: @(video_format),
		(id)kCVPixelBufferWidthKey: @(vcam->videoInfo.output_width),
		(id)kCVPixelBufferHeightKey: @(vcam->videoInfo.output_height),
		(id)kCVPixelBufferIOSurfacePropertiesKey: @{}
	};
	CVReturn status = CVPixelBufferPoolCreate(kCFAllocatorDefault, (__bridge CFDictionaryRef)pAttr,
						  (__bridge CFDictionaryRef)pbAttr, &vcam->pool);

	if (status != kCVReturnSuccess) {
		blog(LOG_ERROR, "unable to allocate pixel buffer pool (error %d)", status);
		return false;
	}

	if (cmio_extension_supported()) {
		UInt32 size;
		UInt32 used;

		CMIOObjectPropertyAddress address{.mSelector = kCMIOHardwarePropertyDevices,
						  .mScope = kCMIOObjectPropertyScopeGlobal,
						  .mElement = kCMIOObjectPropertyElementMain};
		CMIOObjectGetPropertyDataSize(kCMIOObjectSystemObject, &address, 0, NULL, &size);
		NSMutableData *cmioDevices = [NSMutableData dataWithLength:size];
		void *device_data = [cmioDevices mutableBytes];
		CMIOObjectGetPropertyData(kCMIOObjectSystemObject, &address, 0, NULL, size, &used, device_data);

		vcam->deviceID = 0;
		NSString *OBSVirtualCamUUIDString = [[NSBundle bundleWithIdentifier:@"com.obsproject.mac-virtualcam"]
			objectForInfoDictionaryKey:@"OBSCameraDeviceUUID"];
		CFUUIDRef OBSVirtualCamUUID =
			CFUUIDCreateFromString(kCFAllocatorDefault, (CFStringRef)OBSVirtualCamUUIDString);

		size_t num_elements = size / sizeof(CMIOObjectID);
		for (size_t i = 0; i < num_elements; i++) {
			CMIOObjectID cmioDevice;
			[cmioDevices getBytes:&cmioDevice
					range:NSMakeRange(i * sizeof(CMIOObjectID), sizeof(CMIOObjectID))];

			address.mSelector = kCMIODevicePropertyDeviceUID;
			UInt32 device_name_size;
			CMIOObjectGetPropertyDataSize(cmioDevice, &address, 0, NULL, &device_name_size);
			CFStringRef uid;
			CMIOObjectGetPropertyData(cmioDevice, &address, 0, NULL, device_name_size, &used, &uid);
			CFUUIDRef deviceUUID = CFUUIDCreateFromString(kCFAllocatorDefault, uid);

			if (CFEqual(OBSVirtualCamUUID, deviceUUID)) {
				vcam->deviceID = cmioDevice;
				CFRelease(uid);
				CFRelease(deviceUUID);
				break;
			} else {
				CFRelease(uid);
				CFRelease(deviceUUID);
			}
		}
		CFRelease(OBSVirtualCamUUID);

		if (!vcam->deviceID) {
			obs_output_set_last_error(vcam->output,
						  obs_module_text("Error.SystemExtension.CameraUnavailable"));
			return false;
		}

		address.mSelector = kCMIODevicePropertyStreams;
		CMIOObjectGetPropertyDataSize(vcam->deviceID, &address, 0, NULL, &size);
		NSMutableData *streamIds = [NSMutableData dataWithLength:size];
		void *stream_data = [streamIds mutableBytes];
		CMIOObjectGetPropertyData(vcam->deviceID, &address, 0, NULL, size, &used, stream_data);

		if (size < (2 * sizeof(CMIOStreamID))) {
			obs_output_set_last_error(vcam->output,
						  obs_module_text("Error.SystemExtension.CameraNotStarted"));
			return false;
		}
		[streamIds getBytes:&vcam->streamID range:NSMakeRange(sizeof(CMIOStreamID), sizeof(CMIOStreamID))];

		CMIOStreamCopyBufferQueue(vcam->streamID, [](CMIOStreamID, void *, void *) {}, NULL, &vcam->queue);
		CMVideoFormatDescriptionCreate(kCFAllocatorDefault, video_format, vcam->videoInfo.output_width,
					       vcam->videoInfo.output_height, NULL, &vcam->formatDescription);

		OSStatus result = CMIODeviceStartStream(vcam->deviceID, vcam->streamID);

		if (result != noErr) {
			obs_output_set_last_error(vcam->output,
						  obs_module_text("Error.SystemExtension.CameraNotStarted"));
			return false;
		}
	} else {
		[vcam->machServer run];
	}

	vmic_start(vcam);

	if (!obs_output_begin_data_capture(vcam->output, 0)) {
		vmic_stop(vcam);
		return false;
	}

	return true;
}

static void virtualcam_output_stop(void *data, uint64_t ts)
{
	UNUSED_PARAMETER(ts);

	struct virtualcam_data *vcam = (struct virtualcam_data *)data;

	obs_output_end_data_capture(vcam->output);
	if (cmio_extension_supported()) {
		CMIODeviceStopStream(vcam->deviceID, vcam->streamID);
		CFRelease(vcam->formatDescription);
	} else {
		[vcam->machServer stop];
	}
	CVPixelBufferPoolRelease(vcam->pool);
	vmic_stop(vcam);
}

static void virtualcam_output_raw_audio(void *data, struct audio_data *frames)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;
	if (vcam->mic) {
		vmic_shm_write(vcam->mic, (const float *)frames->data[0], frames->frames, mach_absolute_time());
	}
}

static void virtualcam_output_raw_video(void *data, struct video_data *frame)
{
	struct virtualcam_data *vcam = (struct virtualcam_data *)data;

	CVPixelBufferRef frameRef = nil;
	CVReturn status = CVPixelBufferPoolCreatePixelBuffer(kCFAllocatorDefault, vcam->pool, &frameRef);

	if (status != kCVReturnSuccess) {
		blog(LOG_ERROR, "unable to allocate pixel buffer (error %d)", status);
		return;
	}

	// Copy all planes into pixel buffer
	size_t planeCount = CVPixelBufferGetPlaneCount(frameRef);
	CVPixelBufferLockBaseAddress(frameRef, 0);

	if (planeCount == 0) {
		uint8_t *src = frame->data[0];
		uint8_t *dst = (uint8_t *)CVPixelBufferGetBaseAddress(frameRef);

		size_t destBytesPerRow = CVPixelBufferGetBytesPerRow(frameRef);
		size_t srcBytesPerRow = frame->linesize[0];
		size_t height = CVPixelBufferGetHeight(frameRef);

		// Sometimes CVPixelBufferCreate will create a pixel buffer that's a different
		// size than necessary to hold the frame (probably for some optimization reason).
		// If that is the case this will do a row-by-row copy into the buffer.
		if (destBytesPerRow == srcBytesPerRow) {
			memcpy(dst, src, destBytesPerRow * height);
		} else {
			for (int line = 0; (size_t)line < height; line++) {
				memcpy(dst, src, srcBytesPerRow);
				src += srcBytesPerRow;
				dst += destBytesPerRow;
			}
		}
	} else {
		for (size_t plane = 0; plane < planeCount; plane++) {
			uint8_t *src = frame->data[plane];

			if (!src) {
				blog(LOG_WARNING, "Video data from OBS contains less planes than CVPixelBuffer");
				break;
			}

			uint8_t *dst = (uint8_t *)CVPixelBufferGetBaseAddressOfPlane(frameRef, plane);

			size_t destBytesPerRow = CVPixelBufferGetBytesPerRowOfPlane(frameRef, plane);
			size_t srcBytesPerRow = frame->linesize[plane];
			size_t height = CVPixelBufferGetHeightOfPlane(frameRef, plane);

			if (destBytesPerRow == srcBytesPerRow) {
				memcpy(dst, src, destBytesPerRow * height);
			} else {
				for (int line = 0; (size_t)line < height; line++) {
					memcpy(dst, src, srcBytesPerRow);
					src += srcBytesPerRow;
					dst += destBytesPerRow;
				}
			}
		}
	}

	CVPixelBufferUnlockBaseAddress(frameRef, 0);

	if (cmio_extension_supported()) {
		CMSampleBufferRef sampleBuffer;
		CMSampleTimingInfo timingInfo{.presentationTimeStamp = CMTimeMake(frame->timestamp, NSEC_PER_SEC)};

		CMSampleBufferCreateForImageBuffer(kCFAllocatorDefault, frameRef, true, NULL, NULL,
						   vcam->formatDescription, &timingInfo, &sampleBuffer);
		CMSimpleQueueEnqueue(vcam->queue, sampleBuffer);
	} else {
		// Share pixel buffer with clients
		[vcam->machServer sendPixelBuffer:frameRef
					timestamp:frame->timestamp
				     fpsNumerator:vcam->videoInfo.fps_num
				   fpsDenominator:vcam->videoInfo.fps_den];
	}

	CVPixelBufferRelease(frameRef);
}

struct obs_output_info virtualcam_output_info = {
	.id = "virtualcam_output",
	.flags = OBS_OUTPUT_AV,
	.get_name = virtualcam_output_get_name,
	.create = virtualcam_output_create,
	.destroy = virtualcam_output_destroy,
	.start = virtualcam_output_start,
	.stop = virtualcam_output_stop,
	.raw_video = virtualcam_output_raw_video,
	.raw_audio = virtualcam_output_raw_audio,
	.update = virtualcam_output_update,
};

bool obs_module_load(void)
{
	obs_register_output(&virtualcam_output_info);

	return true;
}

#undef VMIC_DRIVER_PATH
#undef VMIC_RATE
#undef VMIC_CHANNELS
