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
#   CRT + Windows SDK).
#
# TODO(soldr cc --target): soldr provisions LLVM and the SDKs for
# `soldr cargo --target` but doesn't expose them as a C compiler for the Apple
# and MSVC targets yet (soldr#2335 shipped GNU/Linux, musl and windows-gnu).
# Until it does, this runs `soldr toolchain prepare` and then finds the
# compiler and SDK under soldr's root by their known layout. Once soldr has
# `soldr cc --target <triple> --print-cc` / `--print-sysroot` for these
# triples, replace _obs_soldr_locate() with those calls.

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

# Cross aliases: Rust/LLVM triple, CMake system name and processor, clang target.
set(_obs_cross_win-x64 x86_64-pc-windows-msvc Windows AMD64 x86_64-pc-windows-msvc)
set(_obs_cross_mac-arm64 aarch64-apple-darwin Darwin arm64 arm64-apple-macos13.0)
set(_obs_cross_mac-x64 x86_64-apple-darwin Darwin x86_64 x86_64-apple-macos13.0)
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

# Newest match of `pattern` (a glob) whose path exists, or "".
function(_obs_soldr_newest out pattern)
  file(GLOB found LIST_DIRECTORIES TRUE "${pattern}")
  list(SORT found COMPARE NATURAL ORDER DESCENDING)
  list(LENGTH found count)
  if(count GREATER 0)
    list(GET found 0 first)
    set(${out} "${first}" PARENT_SCOPE)
  else()
    set(${out} "" PARENT_SCOPE)
  endif()
endfunction()

# Sets <prefix>_LLVM_BIN and <prefix>_SYSROOT from soldr's root (see the TODO
# at the top: this becomes `soldr cc --target` once soldr has it).
function(_obs_soldr_locate root triple prefix)
  _obs_soldr_newest(clang "${root}/bin/llvm-*/hardlinked/bin/clang")
  set(sysroot "")
  if(triple MATCHES "^([a-z0-9_]+)-apple-darwin$")
    _obs_soldr_newest(settings "${root}/bin/apple-sdk/*/darwin-${CMAKE_MATCH_1}/package/sdk/SDKSettings.json")
    if(settings)
      get_filename_component(sysroot "${settings}" DIRECTORY)
    endif()
  else()
    # One xwin tree holds the headers for every Windows architecture.
    _obs_soldr_newest(sysroot "${root}/sdk/x86_64-pc-windows-msvc/xwin/*/xwin")
  endif()
  set(bin "")
  if(clang AND sysroot)
    get_filename_component(bin "${clang}" DIRECTORY)
  else()
    set(sysroot "")
  endif()
  set(${prefix}_LLVM_BIN "${bin}" PARENT_SCOPE)
  set(${prefix}_SYSROOT "${sysroot}" PARENT_SCOPE)
endfunction()

function(obs_soldr_cross_toolchain alias toolchain_file)
  if(NOT DEFINED _obs_cross_${alias})
    string(REPLACE ";" ", " aliases "${OBS_CROSS_ALIASES}")
    message(FATAL_ERROR "Unknown cross target '${alias}'. Use one of: ${aliases}")
  endif()
  list(GET _obs_cross_${alias} 0 triple)
  list(GET _obs_cross_${alias} 1 system)
  list(GET _obs_cross_${alias} 2 processor)
  list(GET _obs_cross_${alias} 3 clang_target)

  obs_soldr_find(soldr)
  get_filename_component(source_root "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/../.." ABSOLUTE)

  # Installs the pinned Rust toolchain (rust-toolchain.toml); newer soldr
  # (soldr#3805) also provisions its host build tools here.
  execute_process(COMMAND "${soldr}" toolchain prepare WORKING_DIRECTORY "${source_root}" RESULT_VARIABLE result)
  if(NOT result EQUAL 0)
    message(FATAL_ERROR "soldr: `soldr toolchain prepare` failed (${result})")
  endif()

  execute_process(
    COMMAND "${soldr}" status
    WORKING_DIRECTORY "${source_root}"
    OUTPUT_VARIABLE status
    RESULT_VARIABLE result
  )
  string(REGEX MATCH "root dir: ([^\n]+)" _ "${status}")
  set(root "${CMAKE_MATCH_1}")
  if(NOT result EQUAL 0 OR root STREQUAL "")
    message(FATAL_ERROR "soldr: could not read soldr's root dir from `soldr status`")
  endif()

  _obs_soldr_locate("${root}" ${triple} found)
  if(NOT found_LLVM_BIN OR NOT found_SYSROOT)
    # soldr fetches the LLVM linker and the target's SDK the first time it
    # links for that target. Link one small test binary to get them.
    message(STATUS "soldr: provisioning LLVM and the SDK for ${triple} (one-time)")
    execute_process(
      COMMAND "${soldr}" rustup target add ${triple}
      WORKING_DIRECTORY "${source_root}"
    )
    execute_process(
      COMMAND "${soldr}" cargo test --no-run --locked -p obs-codec --target ${triple}
      WORKING_DIRECTORY "${source_root}"
      RESULT_VARIABLE result
    )
    _obs_soldr_locate("${root}" ${triple} found)
    if(NOT found_LLVM_BIN OR NOT found_SYSROOT)
      message(
        FATAL_ERROR
        "soldr: no LLVM/SDK for ${triple} under ${root} after provisioning (cargo exit ${result}). "
        "Expected ${root}/bin/llvm-*/hardlinked/bin/clang and "
        "${root}/bin/apple-sdk/*/darwin-<arch>/package/sdk or ${root}/sdk/x86_64-pc-windows-msvc/xwin/*/xwin."
      )
    endif()
  endif()
  message(STATUS "Cross ${alias}: ${clang_target}, LLVM ${found_LLVM_BIN}, sysroot ${found_SYSROOT}")

  set(llvm "${found_LLVM_BIN}")
  set(sysroot "${found_SYSROOT}")
  set(content "# Generated by cmake/common/soldr.cmake for ${alias}. Do not edit.\n")
  string(APPEND content "set(CMAKE_SYSTEM_NAME ${system})\nset(CMAKE_SYSTEM_PROCESSOR ${processor})\n")
  foreach(lang C CXX OBJC OBJCXX)
    set(compiler clang)
    if(lang MATCHES "CXX$")
      set(compiler clang++)
    endif()
    string(APPEND content "set(CMAKE_${lang}_COMPILER \"${llvm}/${compiler}\")\n")
    string(APPEND content "set(CMAKE_${lang}_COMPILER_TARGET ${clang_target})\n")
  endforeach()
  string(APPEND content "set(CMAKE_AR \"${llvm}/llvm-ar\")\nset(CMAKE_RANLIB \"${llvm}/llvm-ranlib\")\n")
  # Compile checks only: nothing links, so CMake's compiler probes must not either.
  string(APPEND content "set(CMAKE_TRY_COMPILE_TARGET_TYPE STATIC_LIBRARY)\n")
  if(system STREQUAL Darwin)
    string(APPEND content "set(CMAKE_OSX_SYSROOT \"${sysroot}\")\n")
    string(APPEND content "set(CMAKE_OSX_ARCHITECTURES ${processor})\n")
    string(APPEND content "set(CMAKE_OSX_DEPLOYMENT_TARGET 13.0)\n")
  else()
    # CMake's Windows-Clang platform requires a resource compiler (CMake 4.2
    # fails without one). soldr's LLVM has no llvm-rc and the check compiles
    # no .rc file, so any existing program satisfies it.
    string(APPEND content "set(CMAKE_RC_COMPILER \"${CMAKE_COMMAND}\")\n")
    # -idirafter, not -isystem: clang's own intrin.h/arm_acle.h must come before
    # the MSVC CRT's, or the ARM64 intrinsics are declared twice.
    set(flags "-fms-compatibility -fms-extensions")
    foreach(
      dir
      crt/include
      sdk/include/ucrt
      sdk/include/um
      sdk/include/shared
      sdk/include/winrt
    )
      string(APPEND flags " -idirafter \\\"${sysroot}/${dir}\\\"")
    endforeach()
    foreach(lang C CXX)
      string(APPEND content "set(CMAKE_${lang}_FLAGS_INIT \"${flags}\")\n")
    endforeach()
  endif()
  string(APPEND content "set(CMAKE_FIND_ROOT_PATH \"${sysroot}\")\n")
  string(APPEND content "set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)\n")
  # Only touch the file when its content changes: rewriting it unchanged
  # (its mtime) kept CMake resetting the cache of an existing build tree.
  file(WRITE "${toolchain_file}.new" "${content}")
  file(COPY_FILE "${toolchain_file}.new" "${toolchain_file}" ONLY_IF_DIFFERENT)
  file(REMOVE "${toolchain_file}.new")
endfunction()
