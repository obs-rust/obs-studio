# FallbackWarning glue and dialog retain their original optional Rust owner.
# Executable, moc, sanitizers and CTest metadata are in build/test-graph.d.
if(OS_LINUX AND (NOT DEFINED ENABLE_UI OR ENABLE_UI) AND (NOT DEFINED ENABLE_FRONTEND OR ENABLE_FRONTEND))
  if(NOT TARGET Qt6::Widgets OR NOT TARGET Qt6::Test)
    find_package(Qt6 QUIET COMPONENTS Widgets Test)
  endif()
  if(TARGET Qt6::Widgets AND TARGET Qt6::Test)
    add_library(native-fallback-warning-frontend INTERFACE)
    add_library(OBS::fallback-warning-test-frontend ALIAS native-fallback-warning-frontend)
    target_compile_definitions(
      native-fallback-warning-frontend
      INTERFACE $<$<TARGET_EXISTS:obs_frontend-static>:OBS_FRONTEND_RUST>
    )
    target_link_libraries(native-fallback-warning-frontend INTERFACE $<TARGET_NAME_IF_EXISTS:obs_frontend-static>)
  endif()
endif()
