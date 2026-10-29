/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>
                          Philippe Groarke <philippe.groarke@gmail.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#pragma once

#include <stdbool.h>
#include <stddef.h>

#include "obs.h"

/* The scene item order, reorder and group membership decisions of
 * obs-scene.c, on plain values (obs-scene-item-order.c):
 * - obs_scene_item_order_anchor(): where obs_sceneitem_set_order() puts the
 *   item back;
 * - obs_scene_item_order_position(): where obs_sceneitem_set_order_position()
 *   puts it;
 * - obs_scene_item_order_item_attached(): whether
 *   obs_sceneitem_get_order_position() and obs_sceneitem_group_ungroup2()
 *   go on with an item;
 * - obs_scene_item_order_match_item(): one step of the check of
 *   obs_scene_reorder_items();
 * - obs_scene_item_order_reorder_check() and
 *   obs_scene_item_order_in_group_run(): the check and the group runs of
 *   obs_scene_reorder_items2();
 * - obs_scene_item_order_add_item_allowed() and
 *   obs_scene_item_order_remove_item_allowed(): the gates of
 *   obs_sceneitem_group_add_item() and obs_sceneitem_group_remove_item();
 * - obs_scene_item_order_moves_scene() and
 *   obs_scene_item_order_item_active(): the two steps of the gate of
 *   move_active_child().
 *
 * obs-scene.c keeps every lock, every scene reference, every list walk and
 * every prev, next, first_item and parent write (detach_sceneitem() and
 * attach_sceneitem()), build_current_order_info(), move_would_recurse(),
 * the group transforms, resize_group(), the hotkey moves, every signal and
 * every log. Item pointers are only compared here, never followed. Nothing
 * here takes a lock, calls into the core or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* Where obs_sceneitem_set_order() attaches the item after detaching it. */
enum obs_scene_item_order_anchor {
	/* Not attached again. */
	OBS_SCENE_ITEM_ORDER_ANCHOR_NONE = 0,
	/* At the bottom (attach_sceneitem() with no previous item). */
	OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM = 1,
	/* After the item below the one it was above (prev->prev). */
	OBS_SCENE_ITEM_ORDER_ANCHOR_PREV_PREV = 2,
	/* After the item it was below (next). */
	OBS_SCENE_ITEM_ORDER_ANCHOR_NEXT = 3,
	/* After the item it was above (prev, which may be NULL). */
	OBS_SCENE_ITEM_ORDER_ANCHOR_PREV = 4,
	/* After the last item, found from next, or prev without one. */
	OBS_SCENE_ITEM_ORDER_ANCHOR_LAST = 5,
	/* Not detached at all: the item stays where it is. Only the Rust port
	 * gives it, for a movement outside the enum (#2895); the C gives
	 * OBS_SCENE_ITEM_ORDER_ANCHOR_NONE there. */
	OBS_SCENE_ITEM_ORDER_ANCHOR_STAY = 6,
};

/* What obs_scene_reorder_items2() does with an order. */
enum obs_scene_item_order_check {
	/* The order is the current one: nothing to do. */
	OBS_SCENE_ITEM_ORDER_SAME = 0,
	/* Relink the scene in this order. */
	OBS_SCENE_ITEM_ORDER_APPLY = 1,
	/* The order is not the scene's items rearranged, each once: refused.
	 * Only the Rust port gives it (#2895); the C relinks such an order. */
	OBS_SCENE_ITEM_ORDER_INVALID = 2,
};

/* movement: the enum obs_order_movement value, as an int. has_prev and
 * has_next: whether the item had an item before (below) and after (above)
 * it, read before the detach.
 *
 * Returns an enum obs_scene_item_order_anchor value: down goes after
 * prev->prev, or the bottom without a prev; up after next, or after prev
 * without a next; top after the last item; bottom at the bottom; any other
 * movement nowhere (the Rust port: where it is). */
int obs_scene_item_order_anchor(int movement, bool has_prev, bool has_next);

/* position: the position asked for. has_first: whether the scene still has
 * a first item after the detach.
 *
 * Returns true when the item goes to the bottom: an empty scene, or
 * position 0. Otherwise stores in *steps how many items to walk from the
 * first item to the one the item goes after (the walk stops early at the
 * last item): position - 1 for a position above 1, else 0. */
bool obs_scene_item_order_position(int position, bool has_first, size_t *steps);

/* has_item: the item is not NULL. has_parent: it is in a scene (only read
 * when has_item).
 *
 * Returns whether the caller goes on with the item: always in the C, which
 * then reads through it; only for an item in a scene in the Rust port. */
bool obs_scene_item_order_item_attached(bool has_item, bool has_parent);

/* One step of the check of obs_scene_reorder_items(): order and size are
 * the order asked for, item the scene's item at index from the bottom.
 *
 * Returns whether item is in the order. Clears *order_matches when the
 * first entry holding it is not at index. */
bool obs_scene_item_order_match_item(obs_sceneitem_t *const *order, size_t size, const obs_sceneitem_t *item,
				     size_t index, bool *order_matches);

/* current and current_size: build_current_order_info()'s (group, item)
 * pairs of the scene (current may be NULL when current_size is 0). order and
 * size: the order asked for.
 *
 * Returns an enum obs_scene_item_order_check value: SAME for the current
 * pairs, else APPLY in the C; INVALID in the Rust port unless the order's
 * items are the current ones, each once. */
int obs_scene_item_order_reorder_check(const struct obs_sceneitem_order_info *current, size_t current_size,
				       const struct obs_sceneitem_order_info *order, size_t size);

/* order and size: the order asked for. head: the index of a group's entry
 * (head < size). index: the entry after head that the run is at, asked once
 * per entry, in the group's lock, after the entry before it was moved.
 *
 * Returns whether entry index goes into the group: it is in the order and
 * has the group's item as its group. The first false ends the run. */
bool obs_scene_item_order_in_group_run(const struct obs_sceneitem_order_info *order, size_t size, size_t head,
				       size_t index);

/* parent: the item's parent, read once. scene: the group's parent scene.
 * group_scene: the group's own scene.
 *
 * Returns whether obs_sceneitem_group_add_item() moves the item: parent is
 * scene, then (only then compared) parent is not group_scene. */
bool obs_scene_item_order_add_item_allowed(const obs_scene_t *parent, const obs_scene_t *scene,
					   const obs_scene_t *group_scene);

/* in_group: the item's parent is the group's own scene.
 *
 * Returns whether obs_sceneitem_group_remove_item() moves the item: always
 * in the C; only an item in the group in the Rust port. */
bool obs_scene_item_order_remove_item_allowed(bool in_group);

/* from and to: the scenes an item moves between (either may be NULL).
 *
 * Returns whether move_active_child() goes on to read the item's active
 * references: the scenes differ. */
bool obs_scene_item_order_moves_scene(const obs_scene_t *from, const obs_scene_t *to);

/* active_refs: the item's active references, read with
 * os_atomic_load_long() only once the scenes differ.
 *
 * Returns whether the source's active child moves with the item: it has an
 * active reference. */
bool obs_scene_item_order_item_active(long active_refs);
