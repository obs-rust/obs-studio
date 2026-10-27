option(ENABLE_SCRIPTING_PYTHON "Enable Python scripting support" ON)

if(ENABLE_SCRIPTING_PYTHON)
  add_subdirectory(obspython)

  if(OS_WINDOWS)
    find_package(Python 3.8...<3.11 REQUIRED Interpreter Development)
  elseif(OS_LINUX OR OS_FREEBSD OR OS_OPENBSD)
    find_package(Python 3.8 REQUIRED Interpreter Development)
  else()
    find_package(Python 3.8...<3.12 REQUIRED Interpreter Development)
  endif()

  add_custom_command(
    OUTPUT swig/swigpyrun.h
    WORKING_DIRECTORY ${CMAKE_CURRENT_BINARY_DIR}
    PRE_BUILD
    COMMAND ${CMAKE_COMMAND} -E make_directory swig
    COMMAND
      ${CMAKE_COMMAND} -E env "SWIG_LIB=${SWIG_DIR}" ${SWIG_EXECUTABLE} -python
      $<$<PLATFORM_ID:Windows,Darwin>:-py3-stable-abi> -external-runtime swig/swigpyrun.h
    COMMENT "obs-scripting - generating Python 3 SWIG interface headers"
  )

  set_source_files_properties(swig/swigpyrun.h PROPERTIES GENERATED TRUE)

  target_sources(
    obs-scripting
    PRIVATE
      $<$<BOOL:${ENABLE_FRONTEND}>:build/obs-scripting.python-frontend+.h.c>
      $<$<PLATFORM_ID:Windows,Darwin>:build/obs-scripting.python-import+.h.c>
      $<$<PLATFORM_ID:Darwin>:build/obs-scripting.python-import-host-macos+.h.c>
      $<$<PLATFORM_ID:Windows>:build/obs-scripting.python-import-host-windows+.h.c>
      obs-scripting-python-import-host.h
      $<$<PLATFORM_ID:Darwin>:build/obs-scripting.python-host-macos+.h.c>
      $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:build/obs-scripting.python-host-posix+.h.c>
      $<$<PLATFORM_ID:Windows>:build/obs-scripting.python-host-windows+.h.c>
      obs-scripting-python-host.h
      $<$<PLATFORM_ID:Darwin>:hosts/macos/obs-scripting-python-include.h>
      $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:hosts/posix/obs-scripting-python-include.h>
      $<$<PLATFORM_ID:Windows>:hosts/windows/obs-scripting-python-include.h>
      obs-scripting-python-import.h
      obs-scripting-python-startup.h
      build/obs-scripting.python+.h.c
      obs-scripting-python.h
      swig/swigpyrun.h
  )

  # The SWIG-generated swigpyrun.h casts const away (-Wcast-qual, obs-rust/obs-studio#516)
  # and converts function pointers to object pointers (-Wpedantic); the upstream
  # py_error() macro in obs-scripting-python.h also uses __FUNCTION__, which
  # -Wpedantic rejects. The generated glue also declares its entry points with no
  # prototype in scope (-Wmissing-prototypes, obs-rust/obs-studio#2703). None of
  # those files is ours, so this target opts out of those diagnostics instead of
  # the global C flag list (obs-rust/obs-studio#2695, #2703).
  target_compile_options(
    obs-scripting
    PRIVATE
      $<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:-Wno-cast-qual;-Wno-pedantic;-Wno-missing-prototypes;-Wno-strict-prototypes>
  )

  target_compile_definitions(
    obs-scripting
    PRIVATE
      ENABLE_SCRIPTING
      PYTHON_LIB="$<TARGET_LINKER_FILE_NAME:Python::Python>"
      # Windows and macOS load Python at run time (obs-scripting-python-import.c).
      RUNTIME_LINK=$<IF:$<PLATFORM_ID:Windows,Darwin>,1,0>
      $<$<PLATFORM_ID:Windows,Darwin>:Py_NO_ENABLE_SHARED>
    PUBLIC Python_FOUND
  )

  # Each host's obs-scripting-python-include.h (#882).
  target_include_directories(
    obs-scripting
    PRIVATE
      "$<$<PLATFORM_ID:Windows>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/windows>"
      "$<$<PLATFORM_ID:Darwin>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/macos>"
      "$<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:${CMAKE_CURRENT_SOURCE_DIR}/hosts/posix>"
  )

  target_include_directories(
    obs-scripting
    PRIVATE "$<$<PLATFORM_ID:Windows,Darwin>:$<TARGET_PROPERTY:Python::Python,INTERFACE_INCLUDE_DIRECTORIES>>"
  )

  # Function pointers from os_dlsym assigned to data pointers (MSVC C4152).
  set_property(
    SOURCE build/obs-scripting.python-import+.h.c
    APPEND
    PROPERTY COMPILE_OPTIONS "$<$<C_COMPILER_ID:MSVC>:/wd4152>"
  )
  # Python.h's named type definitions in parentheses (MSVC C4115).
  set_property(
    SOURCE
      build/obs-scripting.python+.h.c
      build/obs-scripting.python-frontend+.h.c
      build/obs-scripting.python-import+.h.c
    APPEND
    PROPERTY COMPILE_OPTIONS "$<$<C_COMPILER_ID:MSVC>:/wd4115>"
  )
  # The SWIG runtime obs-scripting-python.h includes: unused parameters,
  # non-constant aggregate initializers (MSVC C4100, C4204).
  set_property(
    SOURCE build/obs-scripting.python+.h.c build/obs-scripting.python-frontend+.h.c
    APPEND
    PROPERTY COMPILE_OPTIONS "$<$<C_COMPILER_ID:MSVC>:/wd4100;/wd4204>"
  )

  target_link_libraries(obs-scripting PRIVATE $<$<PLATFORM_ID:Linux,FreeBSD,OpenBSD>:Python::Python>)

  target_link_options(obs-scripting PRIVATE $<$<PLATFORM_ID:Darwin>:LINKER:-undefined,dynamic_lookup>)
else()
  target_disable_feature(obs-scripting "Python scripting support")
endif()
