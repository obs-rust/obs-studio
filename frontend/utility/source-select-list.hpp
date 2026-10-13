#pragma once

#include <obs.hpp>

#include <QString>

#include <vector>

/* The existing sources the Add Source dialog lists, in the order it lists
 * them. `sources` is every source the dialog knows about (obs_enum_sources
 * order, then the scenes). With `recent` set it returns every source except
 * groups and scenes, otherwise the sources whose unversioned id is
 * `typeId`, sorted by name (ignoring case). Removed and expired sources
 * are skipped. */
std::vector<obs_weak_source_t *> SourceSelectExistingList(const std::vector<OBSWeakSource> &sources,
							  const QString &typeId, bool recent);
