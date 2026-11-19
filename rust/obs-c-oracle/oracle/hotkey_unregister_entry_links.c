/* Cargo-only link support. Compile the real single-call adapter; the existing
 * pair-call dependency records its opaque ID in the thread-local oracle world.
 * Native Tier 2 fixtures instead link the unchanged real pair helper and verify
 * member removal, hash deletion and freeing against their fixture-owned core. */
#include "obs-internal.h"

#include "hotkey-unregister-entry-fields.inc"

void oracle_hotkey_unregister_entry_pair(size_t id);
void obs_hotkey_unregister_pair_call(obs_hotkey_pair_id id);
void obs_hotkey_unregister_pair_call(obs_hotkey_pair_id id)
{
	oracle_hotkey_unregister_entry_pair(id);
}
