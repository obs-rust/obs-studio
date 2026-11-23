#include "libobs/build/libobs.pch.h"

void obs_reset_source_uuids(void)
{
	pthread_mutex_lock(&obs->data.sources_mutex);

	/* Move all sources to a new hash table */
	struct obs_context_data *ht = (struct obs_context_data *)obs->data.sources;
	struct obs_context_data *new_ht = NULL;

	struct obs_context_data *ctx, *tmp;
	HASH_ITER (hh_uuid, ht, ctx, tmp) {
		HASH_DELETE(hh_uuid, ht, ctx);

		bfree((void *)(uintptr_t)ctx->uuid);
		ctx->uuid = os_generate_uuid();

		HASH_ADD_UUID(new_ht, uuid, ctx);
	}

	/* The old table will be automatically freed once the last element has
	 * been removed, so we can simply overwrite the pointer. */
	obs->data.sources = (struct obs_source *)new_ht;

	pthread_mutex_unlock(&obs->data.sources_mutex);
}
