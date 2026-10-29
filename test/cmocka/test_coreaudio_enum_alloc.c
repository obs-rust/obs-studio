/*
 * enum_audio_devices() in libobs/audio-monitoring/osx/coreaudio-enum-devices.c
 * allocates the CoreAudio device list with malloc() and read the list into
 * it without checking the result, so a failed allocation handed NULL to
 * AudioObjectGetPropertyData() as the output buffer and, on success, read
 * the ids from NULL (#2862).
 *
 * The test compiles the file into itself over stand-ins for CoreAudio,
 * CoreFoundation and the libobs calls it makes (coreaudio-enum-stubs/,
 * found first as build/libobs-macos.pch.h), so it needs no macOS SDK and
 * the Linux Tier 2 lane runs it. The fake HAL holds a few devices; a read of the device list into NULL is
 * counted and refused with an error, so the old behaviour fails the test
 * instead of crashing it.
 *
 * What the cases pin: the walk reports every output device in order and
 * skips one with no output streams; it stops at the first false; a failed
 * allocation reads nothing and reports nothing, through
 * obs_enum_audio_monitoring_devices(), devices_match() and
 * get_desktop_default_id(); every allocation is freed.
 */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "libobs/audio-monitoring/osx/coreaudio-enum-devices.c.h"

/* ------------------------------------------------------------------------ */
/* The fake HAL. */

struct fake_device {
	AudioDeviceID id;
	const char *uid;
	const char *name;
	UInt32 output_streams_size;
};

static const struct fake_device devices[] = {
	{11, "BuiltInSpeakerDevice", "MacBook Pro Speakers", 8},
	{12, "BuiltInMicrophoneDevice", "MacBook Pro Microphone", 0},
	{13, "BlackHole2ch_UID", "BlackHole 2ch", 8},
};
#define NUM_DEVICES (sizeof(devices) / sizeof(devices[0]))

static AudioDeviceID default_output = 11;

static bool fail_malloc;
static size_t mallocs;
static size_t frees;
static size_t device_list_reads;
static size_t device_list_reads_into_null;
static size_t bmallocs;
static size_t bfrees;

static const struct fake_device *find_device(AudioObjectID id)
{
	for (size_t i = 0; i < NUM_DEVICES; i++) {
		if (devices[i].id == id)
			return &devices[i];
	}
	return NULL;
}

OSStatus AudioObjectGetPropertyDataSize(AudioObjectID id, const AudioObjectPropertyAddress *addr, UInt32 qualifier_size,
					const void *qualifier, UInt32 *out_size)
{
	(void)qualifier_size;
	(void)qualifier;

	if (id == kAudioObjectSystemObject && addr->mSelector == kAudioHardwarePropertyDevices) {
		*out_size = (UInt32)(NUM_DEVICES * sizeof(AudioDeviceID));
		return noErr;
	}

	const struct fake_device *device = find_device(id);
	if (device && addr->mSelector == kAudioDevicePropertyStreams) {
		*out_size = device->output_streams_size;
		return noErr;
	}
	return -1;
}

OSStatus AudioObjectGetPropertyData(AudioObjectID id, const AudioObjectPropertyAddress *addr, UInt32 qualifier_size,
				    const void *qualifier, UInt32 *io_size, void *out_data)
{
	(void)qualifier_size;
	(void)qualifier;

	if (id == kAudioObjectSystemObject) {
		switch (addr->mSelector) {
		case kAudioHardwarePropertyDevices:
			device_list_reads++;
			if (!out_data) {
				device_list_reads_into_null++;
				return -1;
			}
			assert_true(*io_size >= NUM_DEVICES * sizeof(AudioDeviceID));
			for (size_t i = 0; i < NUM_DEVICES; i++)
				((AudioDeviceID *)out_data)[i] = devices[i].id;
			*io_size = (UInt32)(NUM_DEVICES * sizeof(AudioDeviceID));
			return noErr;
		case kAudioHardwarePropertyDefaultOutputDevice:
		case kAudioHardwarePropertyDefaultSystemOutputDevice:
			assert_int_equal(*io_size, sizeof(AudioDeviceID));
			*(AudioDeviceID *)out_data = default_output;
			return noErr;
		default:
			return -1;
		}
	}

	const struct fake_device *device = find_device(id);
	if (!device)
		return -1;
	assert_int_equal(*io_size, sizeof(CFStringRef));
	switch (addr->mSelector) {
	case kAudioDevicePropertyDeviceUID:
		*(CFStringRef *)out_data = device->uid;
		return noErr;
	case kAudioDevicePropertyDeviceNameCFString:
		*(CFStringRef *)out_data = device->name;
		return noErr;
	default:
		return -1;
	}
}

void CFRelease(CFStringRef cf)
{
	(void)cf;
}

char *cfstr_copy_cstr(CFStringRef cfstr, CFStringEncoding cfstr_enc)
{
	assert_int_equal(cfstr_enc, kCFStringEncodingUTF8);
	return bstrdup(cfstr);
}

void *stub_malloc(size_t size)
{
	if (fail_malloc)
		return NULL;
	mallocs++;
	return (malloc)(size);
}

void stub_free(void *ptr)
{
	if (ptr)
		frees++;
	(free)(ptr);
}

/* ------------------------------------------------------------------------ */
/* The libobs calls. */

void blog(int level, const char *format, ...)
{
	va_list args;
	(void)level;
	va_start(args, format);
	vfprintf(stderr, format, args);
	va_end(args);
	fputc('\n', stderr);
}

void *bzalloc(size_t size)
{
	bmallocs++;
	return calloc(1, size);
}

void bfree(void *ptr)
{
	if (ptr)
		bfrees++;
	(free)(ptr);
}

char *bstrdup(const char *str)
{
	size_t len = strlen(str);
	char *copy = (malloc)(len + 1);
	assert_non_null(copy);
	memcpy(copy, str, len + 1);
	bmallocs++;
	return copy;
}

static char tolower_ascii(char c)
{
	return c >= 'A' && c <= 'Z' ? (char)(c - 'A' + 'a') : c;
}

/* As libobs's: ASCII case only. */
char *astrstri(const char *str, const char *find)
{
	size_t len = strlen(find);
	for (; *str; str++) {
		size_t i = 0;
		while (i < len && str[i] && tolower_ascii(str[i]) == tolower_ascii(find[i]))
			i++;
		if (i == len)
			return (char *)str;
	}
	return NULL;
}

/* ------------------------------------------------------------------------ */
/* The cases. */

#define MAX_SEEN 8

struct seen {
	char names[MAX_SEEN][64];
	char ids[MAX_SEEN][64];
	size_t count;
	size_t stop_after;
};

static bool record_device(void *data, const char *name, const char *id)
{
	struct seen *seen = data;
	assert_true(seen->count < MAX_SEEN);
	snprintf(seen->names[seen->count], sizeof(seen->names[0]), "%s", name);
	snprintf(seen->ids[seen->count], sizeof(seen->ids[0]), "%s", id);
	seen->count++;
	return seen->count != seen->stop_after;
}

static int reset(void **state)
{
	(void)state;
	fail_malloc = false;
	mallocs = 0;
	frees = 0;
	device_list_reads = 0;
	device_list_reads_into_null = 0;
	bmallocs = 0;
	bfrees = 0;
	default_output = 11;
	return 0;
}

static void assert_all_freed(void)
{
	assert_int_equal(frees, mallocs);
	assert_int_equal(bfrees, bmallocs);
}

static void test_walk_reports_every_output_device(void **state)
{
	(void)state;
	struct seen seen = {0};

	obs_enum_audio_monitoring_devices(record_device, &seen);

	assert_int_equal(device_list_reads, 1);
	assert_int_equal(mallocs, 1);
	/* The microphone has no output streams. */
	assert_int_equal(seen.count, 2);
	assert_string_equal(seen.names[0], "MacBook Pro Speakers");
	assert_string_equal(seen.ids[0], "BuiltInSpeakerDevice");
	assert_string_equal(seen.names[1], "BlackHole 2ch");
	assert_string_equal(seen.ids[1], "BlackHole2ch_UID");
	assert_all_freed();
}

static void test_walk_stops_at_first_false(void **state)
{
	(void)state;
	struct seen seen = {.stop_after = 1};

	obs_enum_audio_monitoring_devices(record_device, &seen);

	assert_int_equal(seen.count, 1);
	assert_string_equal(seen.ids[0], "BuiltInSpeakerDevice");
	assert_all_freed();
}

static void test_failed_allocation_reads_nothing(void **state)
{
	(void)state;
	struct seen seen = {0};
	fail_malloc = true;

	obs_enum_audio_monitoring_devices(record_device, &seen);

	assert_int_equal(device_list_reads_into_null, 0);
	assert_int_equal(device_list_reads, 0);
	assert_int_equal(seen.count, 0);
	assert_all_freed();
}

static void test_devices_match_by_name(void **state)
{
	(void)state;

	assert_true(devices_match("BlackHole2ch_UID", "BlackHole2ch_UID"));
	assert_true(devices_match("default", "BuiltInSpeakerDevice"));
	assert_false(devices_match("BlackHole2ch_UID", "BuiltInSpeakerDevice"));
	assert_int_equal(device_list_reads_into_null, 0);
	assert_all_freed();
}

static void test_devices_match_failed_allocation(void **state)
{
	(void)state;
	fail_malloc = true;

	assert_false(devices_match("BlackHole2ch_UID", "BlackHole2ch_UID"));
	assert_int_equal(device_list_reads_into_null, 0);
	assert_int_equal(device_list_reads, 0);
	assert_all_freed();
}

static void test_desktop_default_finds_loopback(void **state)
{
	(void)state;
	char *id = NULL;

	/* The system default is the speakers, not a loopback device: the
	 * walk finds BlackHole instead. */
	get_desktop_default_id(&id);
	assert_non_null(id);
	assert_string_equal(id, "BlackHole2ch_UID");
	bfree(id);
	assert_int_equal(device_list_reads, 1);
	assert_all_freed();
}

static void test_desktop_default_failed_allocation(void **state)
{
	(void)state;
	char *id = NULL;
	fail_malloc = true;

	get_desktop_default_id(&id);
	assert_non_null(id);
	assert_string_equal(id, "");
	bfree(id);
	assert_int_equal(device_list_reads_into_null, 0);
	assert_int_equal(device_list_reads, 0);
	assert_all_freed();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(test_walk_reports_every_output_device, reset),
		cmocka_unit_test_setup(test_walk_stops_at_first_false, reset),
		cmocka_unit_test_setup(test_failed_allocation_reads_nothing, reset),
		cmocka_unit_test_setup(test_devices_match_by_name, reset),
		cmocka_unit_test_setup(test_devices_match_failed_allocation, reset),
		cmocka_unit_test_setup(test_desktop_default_finds_loopback, reset),
		cmocka_unit_test_setup(test_desktop_default_failed_allocation, reset),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
