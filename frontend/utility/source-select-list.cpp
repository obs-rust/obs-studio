#include "source-select-list.hpp"

#include <obs.hpp>

#include <algorithm>
#include <iterator>

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

	return matchingSources;
}
