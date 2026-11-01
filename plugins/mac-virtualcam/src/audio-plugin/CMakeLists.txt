cmake_minimum_required(VERSION 3.28...3.30)

# The OBS Virtual Microphone HAL plug-in (obs-rust/obs-studio#3184), installed
# to /Library/Audio/Plug-Ins/HAL along with the camera extension.
add_library(obs-audio-plugin MODULE)
add_library(OBS::audio-plugin ALIAS obs-audio-plugin)

target_sources(obs-audio-plugin PRIVATE build/audio-plugin.macos+.h.c)

target_link_libraries(
  obs-audio-plugin
  PRIVATE
    OBS::mach-protocol
    "$<LINK_LIBRARY:FRAMEWORK,CoreAudio.framework>"
    "$<LINK_LIBRARY:FRAMEWORK,CoreFoundation.framework>"
)

set_target_properties_obs(
  obs-audio-plugin
  PROPERTIES OUTPUT_NAME OBSVirtualMicrophone
             FOLDER plugins
             PREFIX ""
             BUNDLE TRUE BUNDLE_EXTENSION driver MACOSX_BUNDLE_INFO_PLIST
                         "${CMAKE_CURRENT_SOURCE_DIR}/cmake/macos/Info.plist.in"
             LIBRARY_OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/../../"
             MACOSX_BUNDLE_SHORT_VERSION_STRING "${OBS_VERSION_CANONICAL}"
             MACOSX_BUNDLE_BUNDLE_VERSION "${OBS_BUILD_NUMBER}"
)

set_property(TARGET obs-audio-plugin PROPERTY OSX_ARCHITECTURES x86_64 arm64)

set_target_xcode_properties(
  obs-audio-plugin
  PROPERTIES PRODUCT_NAME OBSVirtualMicrophone
             PRODUCT_BUNDLE_IDENTIFIER com.obsproject.obs-virtual-microphone
             CURRENT_PROJECT_VERSION ${OBS_BUILD_NUMBER}
             MARKETING_VERSION ${OBS_VERSION_CANONICAL}
)
