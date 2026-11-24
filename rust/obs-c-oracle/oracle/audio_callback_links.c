/* Test-only link stand-ins: obs_source_audio_render() and
 * obs_source_enum_active_tree() under libobs's own names, for the test
 * binaries that link the obs-core shims without libobs.
 *
 * The audio tick's shim (rust/obs-core/src/ffi/audio_callback.rs) exports
 * audio_callback(), whose libobs calls name them. The MSVC, MinGW and macOS
 * linkers want every symbol an object names resolved, used or not, so
 * obs-tests-core needs a definition; libobs has the real ones.
 * obs_source_active() is the real Rust export of obs-source (#5401), whose
 * counter adapters oracle/source_showing_active.c compiles, and
 * obs_source_removed() is its export too (#5587), whose flag adapter
 * oracle/source_removed.c compiles.
 * obs_source_get_ref() and obs_source_release() (oracle/view.c),
 * obs_source_get_name() (the obs-missing-files tests),
 * audio_output_get_sample_rate() and audio_output_get_channels()
 * (obs-media-io) and the core-audio seam's accessors
 * (oracle/audio_callback.c) already have theirs.
 *
 * The tests never reach these: they run the shim with Calls of their own. */

#include "obs.h"
#include "obs-internal.h"

void obs_source_audio_render(obs_source_t *source, uint32_t mixers, size_t channels, size_t sample_rate, size_t size)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(mixers);
	UNUSED_PARAMETER(channels);
	UNUSED_PARAMETER(sample_rate);
	UNUSED_PARAMETER(size);
}

void obs_source_enum_active_tree(obs_source_t *source, obs_source_enum_proc_t enum_callback, void *param)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(enum_callback);
	UNUSED_PARAMETER(param);
}
