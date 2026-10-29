/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

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

#include "libobs/build/libobs-ported-source-duplicate.pch.h"

/* Ported to rust/obs-source (source_duplicate): the hybrid libobs drops this
 * file. */

/* The duplicate decisions of obs_source_duplicate() and get_new_filter_name()
 * in obs-source.c and of obs_scene_duplicate(), dup_child() and dup_group()
 * in obs-scene.c, moved: the decisions, on plain values. obs-source.c and
 * obs-scene.c keep the lookups, the creates, the references and every other
 * call. Declared in obs-source-duplicate.h without EXPORT. */

/* The scene type check of obs_source_duplicate(). */
int obs_source_duplicate_type_step(int type)
{
	if (type == OBS_SOURCE_DUPLICATE_TYPE_SCENE)
		return OBS_SOURCE_DUPLICATE_ASK_SCENE;

	return OBS_SOURCE_DUPLICATE_ASK_FLAGS;
}

/* A scene that is public is shared; a private one is copied. */
int obs_source_duplicate_scene_step(bool has_scene, bool create_private)
{
	if (has_scene && !create_private)
		return OBS_SOURCE_DUPLICATE_REF;
	if (!has_scene)
		return OBS_SOURCE_DUPLICATE_ASK_GROUP;

	return OBS_SOURCE_DUPLICATE_SCENE_COPY;
}

/* A group is copied; any other scene-typed source has no duplicate. */
int obs_source_duplicate_group_step(bool has_group)
{
	if (!has_group)
		return OBS_SOURCE_DUPLICATE_NONE;

	return OBS_SOURCE_DUPLICATE_SCENE_COPY;
}

int obs_source_duplicate_scene_type(bool create_private)
{
	return create_private ? OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY : OBS_SOURCE_DUPLICATE_DUP_COPY;
}

int obs_source_duplicate_flags_step(uint32_t output_flags)
{
	if ((output_flags & OBS_SOURCE_DUPLICATE_DO_NOT_DUPLICATE) != 0)
		return OBS_SOURCE_DUPLICATE_REF;

	return OBS_SOURCE_DUPLICATE_CREATE;
}

/* The C has no check here: it copies into whatever the create returned. */
int obs_source_duplicate_created_step(bool has_source)
{
	(void)has_source;
	return OBS_SOURCE_DUPLICATE_COPY_STATE;
}

bool obs_source_duplicate_copies_filters(int type)
{
	return type != OBS_SOURCE_DUPLICATE_TYPE_FILTER;
}

/* "%s %d" with ++inc + 1: "<name> 2" after the name itself. */
int obs_source_duplicate_filter_number(int taken)
{
	return taken + 1;
}

bool obs_scene_duplicate_unique(int type)
{
	return type == OBS_SOURCE_DUPLICATE_DUP_COPY || type == OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY;
}

bool obs_scene_duplicate_private(int type)
{
	return type == OBS_SOURCE_DUPLICATE_DUP_PRIVATE_REFS || type == OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY;
}

bool obs_scene_duplicate_keeps_unique(bool unique, bool is_group, int type)
{
	if (is_group && type != OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY)
		return false;

	return unique;
}

int obs_scene_duplicate_item_route(bool is_group, bool unique)
{
	if (is_group)
		return OBS_SCENE_DUPLICATE_ITEM_GROUP;
	else if (unique)
		return OBS_SCENE_DUPLICATE_ITEM_CHILD;
	else
		return OBS_SCENE_DUPLICATE_ITEM_REF;
}

/* The first earlier item with the same source gives the new scene's item at
 * its index. The C does not look at whether that item's copy made it into
 * the new scene. */
int obs_scene_duplicate_reuse(const void *earlier, const void *source, bool earlier_copied)
{
	(void)earlier_copied;

	if (earlier == source)
		return OBS_SCENE_DUPLICATE_REUSE_AT_INDEX;

	return OBS_SCENE_DUPLICATE_REUSE_NONE;
}

bool obs_scene_duplicate_keeps_name(bool make_private)
{
	return make_private;
}

bool obs_scene_duplicate_resizes(bool is_group)
{
	return is_group;
}
