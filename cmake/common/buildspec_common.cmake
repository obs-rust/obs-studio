# OBS common build dependencies module

include_guard(GLOBAL)

# _obs_download_dependency: Downloads url to destination, checking its SHA256.
# A dropped connection or timeout is retried (3 attempts, 10 s then 20 s
# apart), so one slow request doesn't fail a release build.
function(_obs_download_dependency url destination hash)
  if(NOT DEFINED OBS_DEPENDENCY_DOWNLOAD_RETRY_DELAY)
    set(OBS_DEPENDENCY_DOWNLOAD_RETRY_DELAY 10)
  endif()
  set(attempts 3)
  foreach(attempt RANGE 1 ${attempts})
    message(STATUS "Downloading ${url} (attempt ${attempt} of ${attempts})")
    # No EXPECTED_HASH: on a failed transfer it raises an error of its own,
    # which fails the configure even when a retry succeeds.
    file(DOWNLOAD "${url}" "${destination}.tmp" STATUS download_status)
    list(GET download_status 0 error_code)
    list(GET download_status 1 error_message)
    if(NOT error_code)
      file(SHA256 "${destination}.tmp" actual_hash)
      string(TOLOWER "${hash}" expected_hash)
      if(NOT actual_hash STREQUAL expected_hash)
        set(error_code 1)
        set(error_message "SHA256 mismatch: expected ${hash}, got ${actual_hash}")
      endif()
    endif()
    if(NOT error_code)
      file(RENAME "${destination}.tmp" "${destination}")
      message(STATUS "Downloading ${url} - done")
      return()
    endif()
    file(REMOVE "${destination}.tmp")
    if(attempt LESS attempts)
      math(EXPR delay "${OBS_DEPENDENCY_DOWNLOAD_RETRY_DELAY} * ${attempt}")
      message(STATUS "Download failed (${error_message}); retrying in ${delay} s")
      execute_process(COMMAND "${CMAKE_COMMAND}" -E sleep ${delay})
    endif()
  endforeach()
  message(FATAL_ERROR "Unable to download ${url}, failed with error: ${error_message}")
endfunction()

# _check_deps_version: Checks for obs-deps VERSION file in prefix paths
function(_check_deps_version version)
  set(found FALSE PARENT_SCOPE)

  foreach(path IN LISTS CMAKE_PREFIX_PATH)
    if(EXISTS "${path}/share/obs-deps/VERSION")
      if(dependency STREQUAL qt6 AND NOT EXISTS "${path}/lib/cmake/Qt6/Qt6Config.cmake")
        set(found FALSE PARENT_SCOPE)
        continue()
      endif()

      file(READ "${path}/share/obs-deps/VERSION" _check_version)
      string(REPLACE "\n" "" _check_version "${_check_version}")
      string(REPLACE "-" "." _check_version "${_check_version}")
      string(REPLACE "-" "." version "${version}")

      if(_check_version VERSION_EQUAL version)
        set(found TRUE PARENT_SCOPE)
        break()
      elseif(_check_version VERSION_LESS version)
        message(
          AUTHOR_WARNING
          "Older ${label} version detected in ${path}: \n"
          "Found ${_check_version}, require ${version}"
        )
        list(REMOVE_ITEM CMAKE_PREFIX_PATH "${path}")
        list(APPEND CMAKE_PREFIX_PATH "${path}")

        continue()
      else()
        message(
          AUTHOR_WARNING
          "Newer ${label} version detected in ${path}: \n"
          "Found ${_check_version}, require ${version}"
        )
        set(found TRUE PARENT_SCOPE)
        break()
      endif()
    endif()
  endforeach()

  return(PROPAGATE found CMAKE_PREFIX_PATH)
endfunction()

function(_get_dependency_data variable_name)
  file(READ "${CMAKE_CURRENT_SOURCE_DIR}/CMakePresets.json" preset_data)

  string(JSON configure_presets GET ${preset_data} "configurePresets")

  string(JSON preset_count LENGTH "${configure_presets}")
  math(EXPR preset_count "${preset_count}-1")

  foreach(index RANGE 0 ${preset_count})
    string(JSON preset_member_data GET "${configure_presets}" ${index})
    string(JSON preset_name GET ${preset_member_data} "name")

    if(preset_name STREQUAL dependencies)
      string(JSON vendor_data GET ${preset_member_data} "vendor")
      string(JSON vendor_data GET ${vendor_data} "obsproject.com/obs-studio")
      string(JSON dependency_data GET ${vendor_data} "dependencies")
      break()
    else()
      continue()
    endif()
  endforeach()

  set(${variable_name} "${dependency_data}")

  return(PROPAGATE ${variable_name})
endfunction()

# _check_dependencies: Fetch and extract pre-built OBS build dependencies
function(_check_dependencies)
  set(dependencies_list ${ARGV})
  _get_dependency_data(dependency_data)

  foreach(dependency IN LISTS dependencies_list)
    if(dependency STREQUAL cef AND NOT ENABLE_BROWSER)
      continue()
    endif()
    if(dependency STREQUAL cef AND arch STREQUAL universal)
      if(CMAKE_OSX_ARCHITECTURES MATCHES ".+;.+")
        continue()
      elseif(CMAKE_OSX_ARCHITECTURES MATCHES "(arm64|x86_64)")
        set(arch ${CMAKE_OSX_ARCHITECTURES})
      else()
        set(arch ${CMAKE_HOST_SYSTEM_PROCESSOR})
      endif()
      set(platform macos-${arch})
    endif()

    string(JSON data GET ${dependency_data} ${dependency})
    string(JSON version GET ${data} version)
    string(JSON platform_version ERROR_VARIABLE platform_version_error GET ${data} versions ${platform})
    if(NOT platform_version_error)
      set(version ${platform_version})
    endif()
    string(JSON platform_release ERROR_VARIABLE platform_release_error GET ${data} releases ${platform})
    string(JSON hash GET ${data} hashes ${platform})
    string(JSON url GET ${data} baseUrl)
    string(JSON label GET ${data} label)
    string(JSON revision ERROR_VARIABLE error GET ${data} revision ${platform})

    message(STATUS "Setting up ${label} (${arch})")

    set(file "${${dependency}_filename}")
    set(destination "${${dependency}_destination}")
    string(REPLACE "VERSION" "${version}" file "${file}")
    string(REPLACE "VERSION" "${version}" destination "${destination}")
    string(REPLACE "ARCH" "${arch}" file "${file}")
    string(REPLACE "ARCH" "${arch}" destination "${destination}")
    if(revision)
      string(REPLACE "_REVISION" "_v${revision}" file "${file}")
      string(REPLACE "-REVISION" "-v${revision}" file "${file}")
    else()
      string(REPLACE "_REVISION" "" file "${file}")
      string(REPLACE "-REVISION" "" file "${file}")
    endif()

    if(EXISTS "${dependencies_dir}/.dependency_${dependency}_${arch}.sha256")
      file(
        READ "${dependencies_dir}/.dependency_${dependency}_${arch}.sha256"
        OBS_DEPENDENCY_${dependency}_${arch}_HASH
      )
    endif()

    set(skip FALSE)
    if(dependency STREQUAL prebuilt OR dependency STREQUAL qt6)
      if(OBS_DEPENDENCY_${dependency}_${arch}_HASH STREQUAL ${hash})
        _check_deps_version(${version})

        if(found)
          set(skip TRUE)
        endif()
      endif()
    elseif(dependency STREQUAL cef)
      if(NOT ENABLE_BROWSER)
        set(skip TRUE)
      elseif(OBS_DEPENDENCY_${dependency}_${arch}_HASH STREQUAL ${hash} AND (CEF_ROOT_DIR AND EXISTS "${CEF_ROOT_DIR}"))
        set(skip TRUE)
      endif()
    endif()

    if(skip)
      message(STATUS "Setting up ${label} (${arch}) - skipped")
      continue()
    endif()

    if(dependency STREQUAL cef)
      if(platform_release_error)
        set(url ${url}/${file})
      else()
        # The pin's release tag replaces baseUrl's last segment, so a target
        # can move to a newer branch than `version` names (#2418).
        string(REGEX REPLACE "/[^/]+$" "" url_parent "${url}")
        set(url ${url_parent}/${platform_release}/${file})
      endif()
    else()
      set(url ${url}/${version}/${file})
    endif()

    if(NOT EXISTS "${dependencies_dir}/${file}")
      _obs_download_dependency("${url}" "${dependencies_dir}/${file}" "${hash}")
    endif()

    if(NOT OBS_DEPENDENCY_${dependency}_${arch}_HASH STREQUAL ${hash})
      file(REMOVE_RECURSE "${dependencies_dir}/${destination}")
    endif()

    if(NOT EXISTS "${dependencies_dir}/${destination}")
      file(MAKE_DIRECTORY "${dependencies_dir}/${destination}")
      if(dependency STREQUAL obs-studio)
        file(ARCHIVE_EXTRACT INPUT "${dependencies_dir}/${file}" DESTINATION "${dependencies_dir}")
      else()
        file(ARCHIVE_EXTRACT INPUT "${dependencies_dir}/${file}" DESTINATION "${dependencies_dir}/${destination}")
      endif()
    endif()

    file(WRITE "${dependencies_dir}/.dependency_${dependency}_${arch}.sha256" "${hash}")

    if(dependency STREQUAL prebuilt)
      set(VLC_PATH "${dependencies_dir}/${destination}" CACHE PATH "VLC source code directory" FORCE)
      list(APPEND CMAKE_PREFIX_PATH "${dependencies_dir}/${destination}")
    elseif(dependency STREQUAL qt6)
      list(APPEND CMAKE_PREFIX_PATH "${dependencies_dir}/${destination}")
    elseif(dependency STREQUAL cef)
      set(CEF_ROOT_DIR "${dependencies_dir}/${destination}" CACHE PATH "CEF root directory" FORCE)
    endif()

    message(STATUS "Setting up ${label} (${arch}) - done")
  endforeach()

  list(REMOVE_DUPLICATES CMAKE_PREFIX_PATH)

  set(CMAKE_PREFIX_PATH ${CMAKE_PREFIX_PATH} CACHE PATH "CMake prefix search path" FORCE)
endfunction()
