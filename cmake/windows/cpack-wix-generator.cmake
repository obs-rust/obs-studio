# OBS CMake Windows CPack per-generator settings (#3476)
#
# CPack includes this file (CPACK_PROJECT_CONFIG_FILE) once per generator, with
# CPACK_GENERATOR set to the one running. The portable ZIP keeps its settings;
# the WIX MSI is branded BETA so Add/Remove Programs, the Start menu and the
# installer UI all say so, and ships without the PDBs, which the release
# publishes as a separate -symbols.zip like the ZIP does (#627).

if(CPACK_GENERATOR STREQUAL "WIX")
  set(CPACK_PACKAGE_NAME "OBS-Studio-Rust BETA")
  # An MSI ProductVersion is numeric (major.minor.build); keep the leading
  # numeric part of the canonical OBS version.
  string(REGEX MATCH "^[0-9]+(\\.[0-9]+(\\.[0-9]+)?)?" _obs_msi_version "${CPACK_PACKAGE_VERSION}")
  set(CPACK_PACKAGE_VERSION "${_obs_msi_version}")
  set(CPACK_PRE_BUILD_SCRIPTS "${CMAKE_CURRENT_LIST_DIR}/cpack-wix-strip-pdb.cmake")
endif()
