#include "source-select-list.hpp"

#include <obs.hpp>

#include <algorithm>
#include <iterator>
#include <utility>

std::vector<obs_weak_source_t *> SourceSelectExistingList(const std::vector<OBSWeakSource> &sources,
							  const QString &typeId, bool recent)
{
	std::vector<obs_weak_source_t *> matchingSources{};
	std::copy_if(sources.begin(), sources.end(), std::back_inserter(matchingSources),
		     [&typeId, recent](obs_weak_source_t *weak) {
			     OBSSource source = OBSGetStrongRef(weak);

			     if (!source || obs_source_removed(source)) {
				     return false;
			     }

			     const char *id = obs_source_get_unversioned_id(source);
			     QString sourceTypeId = QString(id);

			     if (sourceTypeId.compare("group") == 0) {
				     return false;
			     }

			     if (recent) {
				     // Skip listing scenes in recent sources list
				     if (sourceTypeId.compare("scene") == 0) {
					     return false;
				     }

				     return true;
			     }

			     if (typeId.compare(sourceTypeId) == 0) {
				     return true;
			     }

			     return false;
		     });

	// The Recently Added list keeps creation order.
	if (recent) {
		return matchingSources;
	}

	std::vector<std::pair<QString, obs_weak_source_t *>> named;
	named.reserve(matchingSources.size());
	for (obs_weak_source_t *weak : matchingSources) {
		OBSSource source = OBSGetStrongRef(weak);
		named.emplace_back(QString(source ? obs_source_get_name(source) : ""), weak);
	}

	std::stable_sort(named.begin(), named.end(), [](const auto &a, const auto &b) {
		return a.first.compare(b.first, Qt::CaseInsensitive) < 0;
	});

	for (size_t i = 0; i < named.size(); i++) {
		matchingSources[i] = named[i].second;
	}

	return matchingSources;
}
