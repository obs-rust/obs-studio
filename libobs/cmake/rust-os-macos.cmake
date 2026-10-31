# Corrosion 0.5.2 injects the CLT SDK's libraries ahead of CMake's selected
# sysroot. If CLT is newer than Xcode, its .tbd files may name architectures
# Xcode's linker cannot read (#777). Let the linker use the selected SDK;
# preserve any other native library search paths on the imported target.
get_target_property(_obs_rust_link_dirs libobs_rust-static INTERFACE_LINK_DIRECTORIES)
if(_obs_rust_link_dirs)
  list(REMOVE_ITEM _obs_rust_link_dirs "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk/usr/lib")
  set_property(TARGET libobs_rust-static PROPERTY INTERFACE_LINK_DIRECTORIES "${_obs_rust_link_dirs}")
endif()

target_link_options(${OBS_LIBOBS_RUST_TARGET} PRIVATE "LINKER:-unexported_symbols_list,${_obs_rust_unexports}")
set_property(TARGET ${OBS_LIBOBS_RUST_TARGET} APPEND PROPERTY LINK_DEPENDS "${_obs_rust_unexports}")
