# soldr: finds soldr (or downloads the version CI pins), and locates the
# cross-compile toolchain it provisions for another OS.
#
# obs_soldr_find(<out-var>): path to a soldr executable. Uses OBS_SOLDR_EXECUTABLE
#   or soldr on PATH; otherwise downloads release v${OBS_SOLDR_VERSION} for this
#   host from github.com/zackees/soldr, checks its SHA256, and unpacks it under
#   OBS_SOLDR_DOWNLOAD_DIR (default: <build>/_deps/soldr-<version>).
#
# obs_soldr_cross_toolchain(<alias> <toolchain-file>): writes a CMake toolchain
#   file for <alias> (win-x64, mac-arm64, mac-x64) that compiles with
#   soldr's LLVM against soldr's SDK for that OS (Apple SDK, or the xwin MSVC
#   CRT + Windows SDK). The compiler, linker, archiver, resource compiler and
#   sysroot come from `soldr cc --target <triple> --print-*` (soldr#2335);
#   soldr's install layout is not an interface and is never parsed here.

include_guard(GLOBAL)

# Keep in step with `version:` of zackees/setup-soldr in .github/workflows/
# (ci/test_soldr_pin.py checks it).
set(OBS_SOLDR_VERSION 0.9.45)
# From soldr-v${OBS_SOLDR_VERSION}-SHA256SUMS.txt on the release.
set(_obs_soldr_sha256_x86_64-unknown-linux-gnu 60ce244f514ab02b20b7e777f48ef29523aac987022b8ca4de58069899b0c4a1)
set(_obs_soldr_sha256_aarch64-unknown-linux-gnu aeb529f797c9975c28f2ba563a39ef7e1df50d6f8147e8346d3df078415550d6)
set(_obs_soldr_sha256_x86_64-apple-darwin b03dfee5d40d6e6ae899eca45d21e432e6576b3c4000d28dff37c2315123f620)
set(_obs_soldr_sha256_aarch64-apple-darwin a0d9e6b6321ccefc8278f51f68d21e85bad794b01674d7d91e3d96a1d8f1cc74)
set(_obs_soldr_sha256_x86_64-pc-windows-msvc bf61af4fcbbcb4491f47b3f15291ebbebd0a481a835104db163232911bf81897)
set(_obs_soldr_sha256_aarch64-pc-windows-msvc f2446be23046fca8276f89811eccaceb239f0dc16fa4a8adc904d3ae640ca96c)

# Cross aliases: Rust/LLVM triple, CMake system name and processor.
set(_obs_cross_win-x64 x86_64-pc-windows-msvc Windows AMD64)
set(_obs_cross_mac-arm64 aarch64-apple-darwin Darwin arm64)
set(_obs_cross_mac-x64 x86_64-apple-darwin Darwin x86_64)
# No win-arm64: util/threading-windows.h (a public header) uses MSVC's ARM64
# __ldar*/__stlr* intrinsics, which clang doesn't provide, and Windows ARM64
# builds with MSVC.
set(OBS_CROSS_ALIASES win-x64 mac-arm64 mac-x64)

function(_obs_soldr_host_triple out)
  # Not CMAKE_HOST_SYSTEM_*: cmake/cross needs this before project(), when
  # those are not set yet.
  cmake_host_system_information(RESULT os QUERY OS_NAME)
  cmake_host_system_information(RESULT arch QUERY OS_PLATFORM)
  string(TOLOWER "${arch}" arch)
  if(arch MATCHES "^(x86_64|amd64)$")
    set(arch x86_64)
  elseif(arch MATCHES "^(aarch64|arm64)$")
    set(arch aarch64)
  else()
    message(FATAL_ERROR "soldr: no release for host processor '${arch}'")
  endif()
  if(os STREQUAL Linux)
    set(${out} ${arch}-unknown-linux-gnu PARENT_SCOPE)
  elseif(os STREQUAL macOS)
    set(${out} ${arch}-apple-darwin PARENT_SCOPE)
  elseif(os STREQUAL Windows)
    set(${out} ${arch}-pc-windows-msvc PARENT_SCOPE)
  else()
    message(FATAL_ERROR "soldr: no release for host OS '${os}'")
  endif()
endfunction()

function(obs_soldr_find out)
  if(OBS_SOLDR_EXECUTABLE AND EXISTS "${OBS_SOLDR_EXECUTABLE}")
    set(${out} "${OBS_SOLDR_EXECUTABLE}" PARENT_SCOPE)
    return()
  endif()
  find_program(OBS_SOLDR_EXECUTABLE soldr)
  if(OBS_SOLDR_EXECUTABLE)
    set(${out} "${OBS_SOLDR_EXECUTABLE}" PARENT_SCOPE)
    return()
  endif()

  _obs_soldr_host_triple(triple)
  set(OBS_SOLDR_DOWNLOAD_DIR "${CMAKE_BINARY_DIR}/_deps/soldr-${OBS_SOLDR_VERSION}" CACHE PATH "Where to unpack soldr")
  set(name "soldr-v${OBS_SOLDR_VERSION}-${triple}.tar.zst")
  set(archive "${OBS_SOLDR_DOWNLOAD_DIR}/${name}")
  set(url "https://github.com/zackees/soldr/releases/download/v${OBS_SOLDR_VERSION}/${name}")
  if(NOT EXISTS "${archive}")
    message(STATUS "soldr: downloading ${url}")
    file(
      DOWNLOAD "${url}"
      "${archive}.part"
      EXPECTED_HASH "SHA256=${_obs_soldr_sha256_${triple}}"
      STATUS status
      TLS_VERIFY ON
    )
    list(GET status 0 code)
    if(NOT code EQUAL 0)
      file(REMOVE "${archive}.part")
      message(FATAL_ERROR "soldr: download of ${url} failed: ${status}")
    endif()
    file(RENAME "${archive}.part" "${archive}")
  endif()
  file(ARCHIVE_EXTRACT INPUT "${archive}" DESTINATION "${OBS_SOLDR_DOWNLOAD_DIR}/bin")
  find_program(OBS_SOLDR_EXECUTABLE soldr HINTS "${OBS_SOLDR_DOWNLOAD_DIR}/bin" NO_DEFAULT_PATH REQUIRED)
  set(${out} "${OBS_SOLDR_EXECUTABLE}" PARENT_SCOPE)
endfunction()

# Asks soldr which tool the cross build should use:
# `soldr cc --target <triple> --print-<what>` (soldr#2335). Those paths are the
# supported interface; soldr provisions the target's LLVM, linker and SDK on
# first use, and its install layout is never parsed here.
function(
  _obs_soldr_cc_print
  out
  soldr
  triple
  what
  workdir
)
  execute_process(
    COMMAND "${soldr}" cc --target ${triple} --print-${what}
    WORKING_DIRECTORY "${workdir}"
    OUTPUT_VARIABLE value
    ERROR_VARIABLE error
    RESULT_VARIABLE result
    OUTPUT_STRIP_TRAILING_WHITESPACE
  )
  if(NOT result EQUAL 0 OR value STREQUAL "")
    message(FATAL_ERROR "soldr: `soldr cc --target ${triple} --print-${what}` failed (${result}): ${error}")
  endif()
  set(${out} "${value}" PARENT_SCOPE)
endfunction()

function(obs_soldr_cross_toolchain alias toolchain_file)
  if(NOT DEFINED _obs_cross_${alias})
    string(REPLACE ";" ", " aliases "${OBS_CROSS_ALIASES}")
    message(FATAL_ERROR "Unknown cross target '${alias}'. Use one of: ${aliases}")
  endif()
  list(GET _obs_cross_${alias} 0 triple)
  list(GET _obs_cross_${alias} 1 system)
  list(GET _obs_cross_${alias} 2 processor)

  obs_soldr_find(soldr)
  get_filename_component(source_root "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/../.." ABSOLUTE)

  # Installs the pinned Rust toolchain (rust-toolchain.toml); newer soldr
  # (soldr#3805) also provisions its host build tools here.
  execute_process(COMMAND "${soldr}" toolchain prepare WORKING_DIRECTORY "${source_root}" RESULT_VARIABLE result)
  if(NOT result EQUAL 0)
    message(FATAL_ERROR "soldr: `soldr toolchain prepare` failed (${result})")
  endif()

  _obs_soldr_cc_print(cc "${soldr}" ${triple} cc "${source_root}")
  _obs_soldr_cc_print(cxx "${soldr}" ${triple} cxx "${source_root}")
  _obs_soldr_cc_print(linker "${soldr}" ${triple} linker "${source_root}")
  _obs_soldr_cc_print(sysroot "${soldr}" ${triple} sysroot "${source_root}")
  _obs_soldr_cc_print(ar "${soldr}" ${triple} ar "${source_root}")

  # CMake drives this clang in its GNU-like mode (the driver is named *-clang,
  # not clang-cl), whose static-archive rule is `ar qc`; on the MSVC targets
  # `--print-ar` is llvm-lib, which speaks lib.exe syntax (`/out:`). Use the
  # ar-CLI archiver from the same LLVM that soldr printed, and keep llvm-lib
  # for the link step that needs MSVC archives.
  set(ar_cli "${ar}")
  set(ranlib "${ar}")
  get_filename_component(ar_dir "${ar}" DIRECTORY)
  if(EXISTS "${ar_dir}/llvm-ar")
    set(ar_cli "${ar_dir}/llvm-ar")
    set(ranlib "${ar_cli}")
  endif()
  if(EXISTS "${ar_dir}/llvm-ranlib")
    set(ranlib "${ar_dir}/llvm-ranlib")
  endif()

  message(STATUS "Cross ${alias}: ${cc}, sysroot ${sysroot}")

  set(content "# Generated by cmake/common/soldr.cmake for ${alias}. Do not edit.\n")
  string(APPEND content "set(CMAKE_SYSTEM_NAME ${system})\nset(CMAKE_SYSTEM_PROCESSOR ${processor})\n")
  # soldr's driver is already targeted and carries the target's include and
  # link flags, so no -target and no hand-written SDK include list is needed.
  foreach(lang C CXX OBJC OBJCXX)
    set(compiler "${cc}")
    if(lang MATCHES "CXX$")
      set(compiler "${cxx}")
    endif()
    string(APPEND content "set(CMAKE_${lang}_COMPILER \"${compiler}\")\n")
  endforeach()
  string(APPEND content "set(CMAKE_LINKER \"${linker}\")\n")
  string(APPEND content "set(CMAKE_AR \"${ar_cli}\")\nset(CMAKE_RANLIB \"${ranlib}\")\n")
  # Compile checks only: nothing links, so CMake's compiler probes must not either.
  string(APPEND content "set(CMAKE_TRY_COMPILE_TARGET_TYPE STATIC_LIBRARY)\n")
  if(system STREQUAL Darwin)
    string(APPEND content "set(CMAKE_OSX_SYSROOT \"${sysroot}\")\n")
    string(APPEND content "set(CMAKE_OSX_ARCHITECTURES ${processor})\n")
    string(APPEND content "set(CMAKE_OSX_DEPLOYMENT_TARGET 13.0)\n")
  else()
    # CMake's Windows-Clang platform requires a resource compiler (CMake 4.2
    # fails without one); soldr ships llvm-rc for the MSVC targets.
    _obs_soldr_cc_print(rc "${soldr}" ${triple} rc "${source_root}")
    string(APPEND content "set(CMAKE_RC_COMPILER \"${rc}\")\n")
  endif()
  string(APPEND content "set(CMAKE_FIND_ROOT_PATH \"${sysroot}\")\n")
  string(APPEND content "set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)\n")
  # Only touch the file when its content changes: rewriting it unchanged
  # (its mtime) kept CMake resetting the cache of an existing build tree.
  file(WRITE "${toolchain_file}.new" "${content}")
  file(COPY_FILE "${toolchain_file}.new" "${toolchain_file}" ONLY_IF_DIFFERENT)
  file(REMOVE "${toolchain_file}.new")
endfunction()
