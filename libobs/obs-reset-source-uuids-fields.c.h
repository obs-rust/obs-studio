#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_reset_source_uuids() (#5520): the
 * sources mutex, the uuid table's head and links, the uthash unlink and add
 * of one context, its uuid's free and replacement, and the store of the new
 * head. The table, its layout, the allocator and the uuid generator stay in
 * C; Rust decides which reads and calls are made, and in which order. A
 * table head the walk owns (the working table and the new one) is passed by
 * address, as the original's locals are. */

void obs_reset_source_uuids_lock(void);
void obs_reset_source_uuids_lock(void)
{
	pthread_mutex_lock(&obs->data.sources_mutex);
}

void obs_reset_source_uuids_unlock(void);
void obs_reset_source_uuids_unlock(void)
{
	pthread_mutex_unlock(&obs->data.sources_mutex);
}

/* The core's uuid table, as a context. */
struct obs_context_data *obs_reset_source_uuids_head(void);
struct obs_context_data *obs_reset_source_uuids_head(void)
{
	return (struct obs_context_data *)obs->data.sources;
}

struct obs_context_data *obs_reset_source_uuids_next(const struct obs_context_data *ctx);
struct obs_context_data *obs_reset_source_uuids_next(const struct obs_context_data *ctx)
{
	return (struct obs_context_data *)ctx->hh_uuid.next;
}

/* HASH_DELETE of ctx from the table whose head is *table. */
void obs_reset_source_uuids_delete(struct obs_context_data **table, struct obs_context_data *ctx);
void obs_reset_source_uuids_delete(struct obs_context_data **table, struct obs_context_data *ctx)
{
	struct obs_context_data *ht = *table;
	HASH_DELETE(hh_uuid, ht, ctx);
	*table = ht;
}

void obs_reset_source_uuids_free_uuid(struct obs_context_data *ctx);
void obs_reset_source_uuids_free_uuid(struct obs_context_data *ctx)
{
	bfree((void *)(uintptr_t)ctx->uuid);
}

void obs_reset_source_uuids_new_uuid(struct obs_context_data *ctx);
void obs_reset_source_uuids_new_uuid(struct obs_context_data *ctx)
{
	ctx->uuid = os_generate_uuid();
}

/* HASH_ADD_UUID of ctx, under its uuid, to the table whose head is *table. */
void obs_reset_source_uuids_add(struct obs_context_data **table, struct obs_context_data *ctx);
void obs_reset_source_uuids_add(struct obs_context_data **table, struct obs_context_data *ctx)
{
	struct obs_context_data *ht = *table;
	HASH_ADD_UUID(ht, uuid, ctx);
	*table = ht;
}

void obs_reset_source_uuids_store(struct obs_context_data *table);
void obs_reset_source_uuids_store(struct obs_context_data *table)
{
	obs->data.sources = (struct obs_source *)table;
}
