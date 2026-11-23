/******************************************************************************
    Copyright (C) 2026 pkv <pkv@obsproject.com>
    This file is part of obs-vst3.
    It uses the Steinberg VST3 SDK, which is licensed under MIT license.
    See https://github.com/steinbergmedia/vst3sdk for details.
    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.
    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.
    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "plugins/obs-vst3/vst3-scan/build/obs-vst3-scanner.pch.hpp"

VST3Scanner *list;
std::string vst3ConfigPath = "";

static bool vst3CacheSave()
{
	if (!list) {
		return false;
	}

	if (vst3ConfigPath.empty()) {
		return false;
	}

	obs_data_t *root = obs_data_create();
	obs_data_array_t *arr = obs_data_array_create();

	for (const auto &p : list->pluginList) {
		obs_data_t *obj = obs_data_create();
		obs_data_set_string(obj, "name", p.name.c_str());
		obs_data_set_string(obj, "id", p.id.c_str());
		obs_data_set_string(obj, "path", p.path.c_str());
		obs_data_set_string(obj, "pluginName", p.pluginName.c_str());
		obs_data_set_bool(obj, "discardable", p.discardable);
		obs_data_set_bool(obj, "isFX", p.isFX);
		obs_data_array_push_back(arr, obj);
		obs_data_release(obj);
	}

	obs_data_set_int(root, "version", 1);
	obs_data_set_array(root, "plugins", arr);
	obs_data_array_release(arr);

	bool saved = obs_data_save_json_safe(root, vst3ConfigPath.c_str(), "tmp", "bak");
	obs_data_release(root);

	return saved;
}

static bool vst3CacheLoad()
{
	if (vst3ConfigPath.empty()) {
		return false;
	}

	obs_data_t *root = obs_data_create_from_json_file_safe(vst3ConfigPath.c_str(), "bak");

	if (!root) {
		return false;
	}

	obs_data_array_t *jsonPluginList = obs_data_get_array(root, "plugins");
	if (!jsonPluginList) {
		obs_data_release(root);
		return false;
	}

	list->pluginList.clear();
	list->classCount.clear();

	std::unordered_map<std::string, ModuleCache> modules;

	size_t jsonPluginNumber = obs_data_array_count(jsonPluginList);
	for (size_t i = 0; i < jsonPluginNumber; ++i) {
		obs_data_t *obj = obs_data_array_item(jsonPluginList, i);
		VST3ClassInfo classInfo;

		classInfo.name = obs_data_get_string(obj, "name");
		classInfo.id = obs_data_get_string(obj, "id");
		classInfo.path = obs_data_get_string(obj, "path");
		classInfo.pluginName = obs_data_get_string(obj, "pluginName");
		classInfo.discardable = obs_data_get_bool(obj, "discardable");
		classInfo.isFX = obs_data_get_bool(obj, "isFX");

		obs_data_release(obj);

		if (classInfo.path.empty()) {
			continue;
		}

		try {
			if (!std::filesystem::exists(std::filesystem::u8path(classInfo.path))) {
				continue;
			}
		} catch (const std::filesystem::filesystem_error &e) {
			blog(LOG_ERROR, "[VST3 Scanner] Cannot access %s: %s", classInfo.path.c_str(), e.what());
			continue;
		}

		auto &m = modules[classInfo.path];
		if (classInfo.discardable) {
			m.discardable = true;
		}

		m.classes.push_back(std::move(classInfo));
	}

	obs_data_array_release(jsonPluginList);
	obs_data_release(root);

	list->updateModulesList(modules);

	for (auto &[modulePath, m] : modules) {
		// if a module has the flag kClassesDiscardable, the SDK compels us to do a full load from binary, duh ...
		if (m.discardable) {
			list->addModuleClasses(modulePath);
		} else {
			for (auto &classInfo : m.classes) {
				list->pluginList.push_back(classInfo);
				++list->classCount[modulePath];
			}
		}
	}

	list->sort();

	return !list->pluginList.empty();
}

static bool scanVst3List()
{
	try {
		using clock = std::chrono::steady_clock;
		auto start = clock::now();

		bool loadedFromCache = vst3CacheLoad();
		if (!loadedFromCache) {
			if (!list->scanForVST3Plugins()) {
				blog(LOG_ERROR, "[VST3 Scanner] Error when scanning for VST3.\n");
			}
		}

		size_t effectCount = 0;
		for (const auto &plugin : list->pluginList) {
			if (!plugin.isFX) {
				continue;
			}
			++effectCount;
		}
		auto end = clock::now();
		auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(end - start).count();

		fprintf(stderr, "[VST3 Scanner] %s in %lld ms, found %zu plugins\n",
			loadedFromCache ? "Loaded cache & non-cacheable VST3s" : "Completed scan",
			static_cast<long long>(ms), effectCount);
		fflush(stderr);
		return vst3CacheSave();
	} catch (const std::exception &e) {
		blog(LOG_ERROR, "[VST3 Scanner] Scan aborted by an exception: %s\n", e.what());
		return false;
	} catch (...) {
		blog(LOG_ERROR, "[VST3 Scanner] Scan aborted by an unknown exception\n");
		return false;
	}
}

bool retrieveVst3List()
{
	list = new VST3Scanner();
	if (!list->hasVST3()) {
		blog(LOG_ERROR, "[VST3 Scanner] No VST3 were found\n");
		fprintf(stdout, "NO_VST3\n");
		fflush(stdout);
		return false;
	}

	return scanVst3List();
}

int vst3_scanner_main(const char *cachePath)
{
	setvbuf(stderr, nullptr, _IONBF, 0);

	if (!cachePath || !*cachePath) {
		fprintf(stderr, "Usage: obs-vst3-scanner <vst3list.json>\n");
		return EXIT_FAILURE;
	}
	vst3ConfigPath = cachePath;

	const auto directory = std::filesystem::u8path(vst3ConfigPath).parent_path().u8string();
	if (!directory.empty() && os_mkdirs(directory.c_str()) == MKDIR_ERROR) {
		fprintf(stderr, "Cannot create cache directory: %s\n", directory.c_str());
		return EXIT_FAILURE;
	}

	if (!retrieveVst3List()) {
		fprintf(stderr, "Scan failed\n");
		delete list;
		return EXIT_FAILURE;
	}

	fprintf(stdout, "VST3_SCAN_DONE\n");
	std::fflush(stdout);
	delete list;
	return EXIT_SUCCESS;
}
