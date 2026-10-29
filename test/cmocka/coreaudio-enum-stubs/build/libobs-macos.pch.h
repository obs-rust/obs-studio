/* Test stand-in for libobs/build/libobs-macos.pch.h, for
 * test_coreaudio_enum_alloc: the CoreAudio, CoreFoundation and libobs names
 * audio-monitoring/osx/coreaudio-enum-devices.c takes from it, with no Apple
 * or libobs header, so the file builds without the macOS SDK. The test
 * defines the functions declared here. malloc and free are routed to it
 * too, so it can fail the device list's allocation. */

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

/* CoreAudio and CoreFoundation. */
typedef uint32_t UInt32;
typedef int32_t OSStatus;
typedef UInt32 AudioObjectID;
typedef AudioObjectID AudioDeviceID;
typedef UInt32 AudioObjectPropertySelector;
typedef UInt32 AudioObjectPropertyScope;
typedef UInt32 AudioObjectPropertyElement;

typedef struct AudioObjectPropertyAddress {
	AudioObjectPropertySelector mSelector;
	AudioObjectPropertyScope mScope;
	AudioObjectPropertyElement mElement;
} AudioObjectPropertyAddress;

/* A CFString is its UTF-8 bytes here. */
typedef const char *CFStringRef;
typedef UInt32 CFStringEncoding;

enum {
	noErr = 0,
	kAudioObjectSystemObject = 1,
	kAudioObjectPropertyElementMain = 0,
	kAudioObjectPropertyScopeGlobal = 0x676C6F62,                 /* 'glob' */
	kAudioDevicePropertyScopeOutput = 0x6F757470,                 /* 'outp' */
	kAudioHardwarePropertyDevices = 0x64657623,                   /* 'dev#' */
	kAudioHardwarePropertyDefaultOutputDevice = 0x644F7574,       /* 'dOut' */
	kAudioHardwarePropertyDefaultSystemOutputDevice = 0x734F7574, /* 'sOut' */
	kAudioDevicePropertyStreams = 0x73746D23,                     /* 'stm#' */
	kAudioDevicePropertyDeviceUID = 0x75696420,                   /* 'uid ' */
	kAudioDevicePropertyDeviceNameCFString = 0x6C6E616D,          /* 'lnam' */
	kCFStringEncodingUTF8 = 0x08000100,
};

OSStatus AudioObjectGetPropertyDataSize(AudioObjectID id, const AudioObjectPropertyAddress *addr, UInt32 qualifier_size,
					const void *qualifier, UInt32 *out_size);
OSStatus AudioObjectGetPropertyData(AudioObjectID id, const AudioObjectPropertyAddress *addr, UInt32 qualifier_size,
				    const void *qualifier, UInt32 *io_size, void *out_data);
void CFRelease(CFStringRef cf);

/* libobs. */
#define UNUSED_PARAMETER(param) (void)param
enum { LOG_ERROR = 100, LOG_WARNING = 200, LOG_INFO = 300, LOG_DEBUG = 400 };
void blog(int level, const char *format, ...);
void *bzalloc(size_t size);
void bfree(void *ptr);
char *bstrdup(const char *str);
char *astrstri(const char *str, const char *find);
char *cfstr_copy_cstr(CFStringRef cfstr, CFStringEncoding cfstr_enc);

typedef bool (*obs_enum_audio_device_cb)(void *data, const char *name, const char *id);
void obs_enum_audio_monitoring_devices(obs_enum_audio_device_cb cb, void *data);
bool devices_match(const char *id1, const char *id2);
void get_desktop_default_id(char **p_id);

#include "libobs/audio-monitoring/osx/mac-helpers.h"

/* The device list's allocation. */
void *stub_malloc(size_t size);
void stub_free(void *ptr);
#define malloc(size) stub_malloc(size)
#define free(ptr) stub_free(ptr)
