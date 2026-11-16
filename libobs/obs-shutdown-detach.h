#pragma once

#include <stdbool.h>

struct obs_core_data;
struct obs_source;
struct obs_scene;
struct obs_scene_item;

/* What obs_shutdown_detach_scene_items_run() reaches of the core: the source
 * table and its mutex, the source and scene calls and the destruction queue.
 * The bridge in obs-shutdown-detach.c fills it; every member is set. */
struct obs_shutdown_detach_calls {
	void (*lock)(struct obs_core_data *data);
	void (*unlock)(struct obs_core_data *data);
	/* Hands every member of the UUID source table to visit, in table
	 * order. Called with the table locked. */
	void (*each_source)(struct obs_core_data *data, void (*visit)(void *param, struct obs_source *source),
			    void *param);
	struct obs_source *(*get_ref)(struct obs_source *source);
	struct obs_scene *(*scene_from_source)(const struct obs_source *source);
	struct obs_scene *(*group_from_source)(const struct obs_source *source);
	void (*enum_items)(struct obs_scene *scene,
			   bool (*callback)(struct obs_scene *, struct obs_scene_item *, void *), void *param);
	void (*item_remove)(struct obs_scene_item *item);
	void (*release)(struct obs_source *source);
	/* os_task_queue_wait(obs->destruction_task_thread). */
	bool (*wait_destruction)(void);
};

/* Shutdown's detachment of scene and group items
 * (obs_shutdown_detach_scene_items() in obs-shutdown-detach.c), a private
 * adapter, not a new public API. It is Rust in hybrid libobs
 * (obs-core::shutdown_detach) and the original C in the C reference
 * (obs-shutdown-detach-reference.c). Hidden, not EXPORT.
 *
 * Under the table lock it takes a strong reference to every source it can,
 * then releases the lock; it removes every item of each snapshot member that
 * is a scene or a group, then releases every snapshot reference, then waits
 * on the destruction queue until it reports no more work. The Rust reaches
 * the core only through calls, so the table, its mutex and the queue stay
 * C-owned; the C reference reads data and the core directly and ignores
 * them. */
void obs_shutdown_detach_scene_items_run(struct obs_core_data *data, const struct obs_shutdown_detach_calls *calls);
