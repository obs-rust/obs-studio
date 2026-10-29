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

#include "libobs/build/libobs-ported-scene-item-order.pch.h"

/* Ported to rust/obs-scene (item_order): the hybrid libobs drops this
 * file. */

/* The scene item order, reorder and group membership decisions of
 * obs-scene.c, moved: the decisions, on plain values. obs-scene.c keeps the
 * locks, the walks, the links and every call. Declared in
 * obs-scene-item-order.h without EXPORT. */

/* The movement branches of obs_sceneitem_set_order(). */
int obs_scene_item_order_anchor(int movement, bool has_prev, bool has_next)
{
	if (movement == OBS_ORDER_MOVE_DOWN) {
		return has_prev ? OBS_SCENE_ITEM_ORDER_ANCHOR_PREV_PREV : OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM;

	} else if (movement == OBS_ORDER_MOVE_UP) {
		return has_next ? OBS_SCENE_ITEM_ORDER_ANCHOR_NEXT : OBS_SCENE_ITEM_ORDER_ANCHOR_PREV;

	} else if (movement == OBS_ORDER_MOVE_TOP) {
		return OBS_SCENE_ITEM_ORDER_ANCHOR_LAST;

	} else if (movement == OBS_ORDER_MOVE_BOTTOM) {
		return OBS_SCENE_ITEM_ORDER_ANCHOR_BOTTOM;
	}

	return OBS_SCENE_ITEM_ORDER_ANCHOR_NONE;
}

/* The bottom test and the walk length of obs_sceneitem_set_order_position(). */
bool obs_scene_item_order_position(int position, bool has_first, size_t *steps)
{
	if (!has_first || position == 0)
		return true;

	*steps = position > 1 ? (size_t)position - 1 : 0;
	return false;
}

/* obs_sceneitem_get_order_position() and obs_sceneitem_group_ungroup2() have
 * no check: they read item->parent whatever they are given. */
bool obs_scene_item_order_item_attached(bool has_item, bool has_parent)
{
	UNUSED_PARAMETER(has_item);
	UNUSED_PARAMETER(has_parent);
	return true;
}

/* The inner loop of sceneitems_match(). */
bool obs_scene_item_order_match_item(obs_sceneitem_t *const *order, size_t size, const obs_sceneitem_t *item,
				     size_t index, bool *order_matches)
{
	for (size_t i = 0; i < size; i++) {
		if (order[i] != item)
			continue;

		if (index != i)
			*order_matches = false;

		return true;
	}

	return false;
}

/* The compare of sceneitems_match2(): only the same order is refused. */
int obs_scene_item_order_reorder_check(const struct obs_sceneitem_order_info *current, size_t current_size,
				       const struct obs_sceneitem_order_info *order, size_t size)
{
	if (current_size != size)
		return OBS_SCENE_ITEM_ORDER_APPLY;

	for (size_t i = 0; i < size; i++) {
		const struct obs_sceneitem_order_info *new = &order[i];
		const struct obs_sceneitem_order_info *old = &current[i];

		if (new->group != old->group || new->item != old->item)
			return OBS_SCENE_ITEM_ORDER_APPLY;
	}

	return OBS_SCENE_ITEM_ORDER_SAME;
}

/* The two checks of the inner loop of obs_scene_reorder_items2(). */
bool obs_scene_item_order_in_group_run(const struct obs_sceneitem_order_info *order, size_t size, size_t head,
				       size_t index)
{
	if (index >= size)
		return false;

	return order[index].group == order[head].item;
}

/* The two parent checks of obs_sceneitem_group_add_item(). */
bool obs_scene_item_order_add_item_allowed(const obs_scene_t *parent, const obs_scene_t *scene,
					   const obs_scene_t *group_scene)
{
	if (parent != scene)
		return false;

	if (parent == group_scene)
		return false;

	return true;
}

/* obs_sceneitem_group_remove_item() has no parent check. */
bool obs_scene_item_order_remove_item_allowed(bool in_group)
{
	UNUSED_PARAMETER(in_group);
	return true;
}

/* The first check of move_active_child(). */
bool obs_scene_item_order_moves_scene(const obs_scene_t *from, const obs_scene_t *to)
{
	return from != to;
}

/* The second check of move_active_child(). */
bool obs_scene_item_order_item_active(long active_refs)
{
	return active_refs > 0;
}
