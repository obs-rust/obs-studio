target_link_options(${OBS_LIBOBS_RUST_TARGET} PRIVATE "LINKER:--version-script=${_obs_rust_export_map}")
set_property(TARGET ${OBS_LIBOBS_RUST_TARGET} APPEND PROPERTY LINK_DEPENDS "${_obs_rust_export_map}")
# The obs-data glue calls json_delete from inside the staticlib.
# jansson is first linked earlier, in libobs/CMakeLists.txt. On
# Ubuntu, GNU ld --as-needed drops that first occurrence because
# nothing references jansson yet, and every cmocka executable then
# fails to link. Link it again so a later occurrence follows the
# archive.
if(OS_LINUX)
  target_link_libraries(${OBS_LIBOBS_RUST_TARGET} PRIVATE jansson::jansson)
endif()

# Cross-language ThinLTO (#3217): the Rust staticlib carries LLVM bitcode
# (-Clinker-plugin-lto) and lld optimizes it together with the libobs C
# objects (-flto=thin), so the per-frame FFI shims can inline across the
# boundary. Opt-in, ELF hosts with Clang + lld only, and the LLVM majors must match
# or lld cannot read rustc's bitcode. The exported ABI stays the version
# script's.
if(OBS_CROSS_LTO)
  if(NOT CMAKE_C_COMPILER_ID STREQUAL "Clang")
    message(FATAL_ERROR "OBS_CROSS_LTO needs Clang (C compiler: ${CMAKE_C_COMPILER_ID})")
  endif()
  # Corrosion's FindRust parses `rustc -vV` into Rust_LLVM_VERSION_MAJOR.
  set(_obs_rustc_llvm "${Rust_LLVM_VERSION_MAJOR}")
  string(REGEX MATCH "^[0-9]+" _obs_clang_major "${CMAKE_C_COMPILER_VERSION}")
  if(NOT _obs_rustc_llvm STREQUAL _obs_clang_major)
    message(
      FATAL_ERROR
      "OBS_CROSS_LTO: rustc uses LLVM ${_obs_rustc_llvm}, Clang is LLVM ${_obs_clang_major}; the majors must match"
    )
  endif()
  if(TARGET libobs_rust)
    corrosion_add_target_rustflags(libobs_rust -Clinker-plugin-lto)
  endif()
  foreach(_target IN ITEMS libobs-objects ${OBS_LIBOBS_RUST_TARGET})
    target_compile_options(${_target} PRIVATE -flto=thin)
  endforeach()
  # lld, unlike GNU ld, rejects version-script names the link does not define
  # (the shared list names Windows-only ones such as DllMain).
  target_link_options(${OBS_LIBOBS_RUST_TARGET} PRIVATE -flto=thin -fuse-ld=lld LINKER:--undefined-version)
  message(STATUS "libobs: cross-language ThinLTO with the Rust staticlib (LLVM ${_obs_clang_major})")
endif()
