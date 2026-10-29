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

#pragma once

#include <stdbool.h>
#include <stdint.h>

/* The duplicate decisions of obs_source_duplicate() and
 * get_new_filter_name() in obs-source.c, and of obs_scene_duplicate(),
 * dup_child() and dup_group() in obs-scene.c, on plain values
 * (obs-source-duplicate.c):
 * - obs_source_duplicate_type_step(), obs_source_duplicate_scene_step(),
 *   obs_source_duplicate_group_step(), obs_source_duplicate_flags_step()
 *   and obs_source_duplicate_created_step(): the route of
 *   obs_source_duplicate(), one step after each lookup or read;
 * - obs_source_duplicate_scene_type(): the duplicate type it asks
 *   obs_scene_duplicate() for;
 * - obs_source_duplicate_copies_filters(): whether it copies the filters;
 * - obs_source_duplicate_filter_number(): the number get_new_filter_name()
 *   tries next;
 * - obs_scene_duplicate_unique(), obs_scene_duplicate_private() and
 *   obs_scene_duplicate_keeps_unique(): the flags of obs_scene_duplicate();
 * - obs_scene_duplicate_item_route(): how it copies each item;
 * - obs_scene_duplicate_reuse(): which earlier item's source dup_child()
 *   reuses for a repeated source;
 * - obs_scene_duplicate_keeps_name(): whether dup_child() and dup_group()
 *   name the copy;
 * - obs_scene_duplicate_resizes(): whether the new scene is resized.
 *
 * obs-source.c and obs-scene.c keep every lookup (obs_scene_from_source(),
 * obs_group_from_source(), obs_source_get_filter_by_name()), every create,
 * reference and release, obs_data_apply(), obs_scene_add(),
 * duplicate_filters() and duplicate_filter() with their add order,
 * filter_compatible(), duplicate_item_data(), get_child_at_idx(),
 * resize_scene(), every lock, dstr_printf() and every log, each where it
 * was. Source pointers are only compared here, never followed. Nothing here
 * takes a lock, calls into the core or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* obs-source.c checks that these are OBS_SOURCE_TYPE_FILTER,
 * OBS_SOURCE_TYPE_SCENE, OBS_SOURCE_DO_NOT_DUPLICATE and the four
 * enum obs_scene_duplicate_type values. */
#define OBS_SOURCE_DUPLICATE_TYPE_FILTER 1
#define OBS_SOURCE_DUPLICATE_TYPE_SCENE 3
#define OBS_SOURCE_DUPLICATE_DO_NOT_DUPLICATE (1 << 7)
#define OBS_SOURCE_DUPLICATE_DUP_REFS 0
#define OBS_SOURCE_DUPLICATE_DUP_COPY 1
#define OBS_SOURCE_DUPLICATE_DUP_PRIVATE_REFS 2
#define OBS_SOURCE_DUPLICATE_DUP_PRIVATE_COPY 3

/* What obs_source_duplicate() does next. */
enum obs_source_duplicate_step {
	/* Return NULL. */
	OBS_SOURCE_DUPLICATE_NONE = 0,
	/* Return obs_source_get_ref(source). */
	OBS_SOURCE_DUPLICATE_REF = 1,
	/* Look the scene up (obs_scene_from_source()), then ask
	 * obs_source_duplicate_scene_step(). */
	OBS_SOURCE_DUPLICATE_ASK_SCENE = 2,
	/* Look the group up (obs_group_from_source()), then ask
	 * obs_source_duplicate_group_step(). */
	OBS_SOURCE_DUPLICATE_ASK_GROUP = 3,
	/* Return the source of obs_scene_duplicate() of the scene found, with
	 * the type of obs_source_duplicate_scene_type(). */
	OBS_SOURCE_DUPLICATE_SCENE_COPY = 4,
	/* Read info.output_flags, then ask obs_source_duplicate_flags_step(). */
	OBS_SOURCE_DUPLICATE_ASK_FLAGS = 5,
	/* Create a new source from a copy of the settings, then ask
	 * obs_source_duplicate_created_step(). */
	OBS_SOURCE_DUPLICATE_CREATE = 6,
	/* Copy the state of the source into the new one. */
	OBS_SOURCE_DUPLICATE_COPY_STATE = 7,
};

/* How obs_scene_duplicate() copies one item. */
enum obs_scene_duplicate_item {
	/* dup_group(): a copy of the group with the same duplicate type. */
	OBS_SCENE_DUPLICATE_ITEM_GROUP = 0,
	/* dup_child(): a copy of the source, or the copy of an earlier item
	 * with the same source. */
	OBS_SCENE_DUPLICATE_ITEM_CHILD = 1,
	/* new_ref(): a new reference to the same source. */
	OBS_SCENE_DUPLICATE_ITEM_REF = 2,
};

/* What dup_child() takes from one earlier item. */
enum obs_scene_duplicate_reuse {
	/* Nothing: go on to the next earlier item. */
	OBS_SCENE_DUPLICATE_REUSE_NONE = 0,
	/* A new reference to the source of the new scene's item at the earlier
	 * item's index (get_child_at_idx()). */
	OBS_SCENE_DUPLICATE_REUSE_AT_INDEX = 1,
	/* A new reference to the earlier item's own copy, kept by
	 * obs_scene_duplicate(). */
	OBS_SCENE_DUPLICATE_REUSE_COPY = 2,
};

/* type is source->info.type, an enum obs_source_type. Returns an
 * enum obs_source_duplicate_step value: ASK_SCENE for a scene-typed source
 * (a scene or a group), ASK_FLAGS for any other. */
int obs_source_duplicate_type_step(int type);

/* has_scene: obs_scene_from_source() found a scene (not a group).
 * create_private: the copy is private. Returns REF for a public duplicate of
 * a scene (scenes are shared), SCENE_COPY for a private one, ASK_GROUP when
 * no scene was found. */
int obs_source_duplicate_scene_step(bool has_scene, bool create_private);

/* has_group: obs_group_from_source() found a group. Returns SCENE_COPY, or
 * NONE for a scene-typed source that is neither a scene nor a group. */
int obs_source_duplicate_group_step(bool has_group);

/* create_private as above. Returns the enum obs_scene_duplicate_type value
 * a scene copy asks for: PRIVATE_COPY or COPY. */
int obs_source_duplicate_scene_type(bool create_private);

/* output_flags is source->info.output_flags. Returns REF for a source that
 * must not be duplicated (OBS_SOURCE_DO_NOT_DUPLICATE), CREATE otherwise. */
int obs_source_duplicate_flags_step(uint32_t output_flags);

/* has_source: the create returned a source. Returns COPY_STATE, or NONE
 * when there is no new source to copy into. */
int obs_source_duplicate_created_step(bool has_source);

/* type is source->info.type. Whether the new source gets copies of the
 * source's filters: every source but a filter. */
bool obs_source_duplicate_copies_filters(int type);

/* taken: how many names get_new_filter_name() found taken so far (1 after
 * the name itself). Returns the number of the next name to try,
 * "<name> <number>": 2, 3, and so on. */
int obs_source_duplicate_filter_number(int taken);

/* type is the enum obs_scene_duplicate_type value. Whether the items' sources
 * are copied (COPY and PRIVATE_COPY) rather than referenced. */
bool obs_scene_duplicate_unique(int type);

/* type as above. Whether the new scene and its copies are private
 * (PRIVATE_REFS and PRIVATE_COPY). */
bool obs_scene_duplicate_private(int type);

/* unique: obs_scene_duplicate_unique(). is_group: scene->is_group. Whether
 * the items' sources are still copied: a group's are copied only for a
 * private copy (Studio Mode with "Duplicate Sources"), so a public copy of
 * a group keeps its sources. */
bool obs_scene_duplicate_keeps_unique(bool unique, bool is_group, int type);

/* is_group: item->is_group. unique: the flag above. Returns an
 * enum obs_scene_duplicate_item value. */
int obs_scene_duplicate_item_route(bool is_group, bool unique);

/* earlier: the source of an earlier item of the old scene. source: the
 * source of the item dup_child() copies. earlier_copied: the earlier item's
 * copy is in the new scene (its duplicate returned a source and
 * obs_scene_add() took it). Returns an enum obs_scene_duplicate_reuse
 * value; dup_child() asks the earlier items in order and stops at the first
 * that is not NONE. */
int obs_scene_duplicate_reuse(const void *earlier, const void *source, bool earlier_copied);

/* make_private: obs_scene_duplicate_private(). Whether dup_child() and
 * dup_group() give the copy the old source's name (a private copy) rather
 * than a new unique one (NULL). */
bool obs_scene_duplicate_keeps_name(bool make_private);

/* is_group: new_scene->is_group. Whether obs_scene_duplicate() resizes the
 * new scene to its items (resize_scene()). */
bool obs_scene_duplicate_resizes(bool is_group);
