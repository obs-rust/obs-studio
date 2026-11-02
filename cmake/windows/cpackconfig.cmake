# OBS CMake Windows CPack configuration module

include_guard(GLOBAL)

include(cpackconfig_common)

# Add GPLv2 license file to CPack
set(CPACK_RESOURCE_FILE_LICENSE "${CMAKE_SOURCE_DIR}/frontend/data/license/gplv2.txt")
set(CPACK_PACKAGE_VERSION "${OBS_VERSION_CANONICAL}")
set(CPACK_PACKAGE_FILE_NAME "${CPACK_PACKAGE_NAME}-${CPACK_PACKAGE_VERSION}-windows-${CMAKE_VS_PLATFORM_NAME}")
set(CPACK_INCLUDE_TOPLEVEL_DIRECTORY FALSE)
# ZIP is the portable package every build makes. The release also runs
# `cpack -G WIX` (ci/windows_msi.py) for the BETA MSI installer (#3476); the
# WiX .NET tool is only installed there, so plain builds stay ZIP-only.
set(CPACK_GENERATOR ZIP)
set(CPACK_THREADS 0)
set(CPACK_PROJECT_CONFIG_FILE "${CMAKE_CURRENT_LIST_DIR}/cpack-wix-generator.cmake")

# WiX MSI settings. The upgrade code stays fixed forever so each new MSI
# replaces the installed one; the product code is new for every build.
set(CPACK_WIX_VERSION 4)
set(CPACK_WIX_UPGRADE_GUID "CACB86B3-8F27-4041-8774-A50EA32C8B7D")
set(CPACK_WIX_INSTALL_SCOPE perMachine)
set(CPACK_WIX_UI_REF WixUI_InstallDir)
set(CPACK_WIX_PRODUCT_ICON "${CMAKE_SOURCE_DIR}/frontend/cmake/windows/obs-studio.ico")
set(CPACK_WIX_PROGRAM_MENU_FOLDER "OBS-Studio-Rust BETA")
set(CPACK_PACKAGE_INSTALL_DIRECTORY "OBS-Studio-Rust BETA")
set(CPACK_PACKAGE_EXECUTABLES obs64 "OBS-Studio-Rust BETA")
set(CPACK_WIX_PROPERTY_ARPURLINFOABOUT "https://github.com/obs-rust/obs-studio")
if(CMAKE_VS_PLATFORM_NAME MATCHES "^[Aa][Rr][Mm]64$")
  set(CPACK_WIX_ARCHITECTURE arm64)
else()
  set(CPACK_WIX_ARCHITECTURE x64)
endif()

# Ship the Visual C++ runtime (vcruntime140*.dll, msvcp140*.dll, ...) next to
# obs64.exe so the zip starts on machines without the redistributable (#626).
set(CMAKE_INSTALL_SYSTEM_RUNTIME_DESTINATION "${OBS_EXECUTABLE_DESTINATION}")
set(CMAKE_INSTALL_SYSTEM_RUNTIME_COMPONENT Runtime)
include(InstallRequiredSystemLibraries)

include(CPack)
