/* SPDX-License-Identifier: GPL-2.0-or-later */
/* The VideoToolbox encoder list is enumerated once, from
 * obs_module_post_load(), on the module thread. It must not be handed to a
 * background queue from obs_module_load(): the enumeration runs
 * SecStaticCodeCheckValidityWithErrors over every registered encoder, and when
 * that overlaps another thread loading modules (CEF) the process traps in
 * Security's CodeDirectory::validateSlot (obs-rust/obs-studio#3168). */
#include <stdio.h>
#include <VideoToolbox/VideoToolbox.h>

static unsigned async_dispatches;
static unsigned enumerations;

/* Stands in for VTCopyVideoEncoderList: an empty list keeps
 * obs_module_post_load() away from obs_register_encoder(). */
static OSStatus counted_encoder_list(CFAllocatorRef allocator, CFArrayRef *list)
{
	(void)allocator;
	enumerations++;
	*list = CFArrayCreate(NULL, NULL, 0, &kCFTypeArrayCallBacks);
	return noErr;
}

/* Runs the block where it is dispatched, so a load-time dispatch is visible in
 * async_dispatches instead of deadlocking the test on the wait below. */
#define dispatch_group_async(group, queue, block)          \
	do {                                               \
		(void)(group);                             \
		(void)(queue);                             \
		async_dispatches++;                        \
		dispatch_block_t run = (block);            \
		run();                                     \
	} while (0)

#define VTCopyVideoEncoderList counted_encoder_list
#include "../../plugins/mac-videotoolbox/encoder.c"

static int load_does_not_enumerate(void)
{
	int failures = 0;

	obs_module_load();
	if (async_dispatches != 0) {
		fprintf(stderr,
			"load_does_not_enumerate: obs_module_load dispatched the encoder "
			"list enumeration to a background queue\n");
		failures++;
	}
	if (enumerations != 0) {
		fprintf(stderr,
			"load_does_not_enumerate: obs_module_load enumerated the encoder "
			"list itself\n");
		failures++;
	}
	return failures;
}

static int post_load_enumerates_once(void)
{
	int failures = 0;

	obs_module_post_load();
	if (enumerations != 1) {
		fprintf(stderr,
			"post_load_enumerates_once: expected 1 enumeration on the module "
			"thread, got %u\n",
			enumerations);
		failures++;
	}
	return failures;
}

int main(void)
{
	const int failures = load_does_not_enumerate() + post_load_enumerates_once();
	printf("encoder_list_once: failures=%d\n", failures);
	return failures;
}
