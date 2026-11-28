# Pin host executables after Qt's AdditionalTargetInfo has copied configuration
# locations. Target libraries still come from the original Darwin dependency SDK.
function(obs_tier2_bind_host_qt host)
  foreach(_tool IN ITEMS moc uic rcc cmake_automoc_parser)
    if(TARGET Qt6::${_tool})
      set(_path "${host}/libexec/${_tool}")
      if(NOT EXISTS "${_path}")
        message(FATAL_ERROR "Missing pinned Linux Qt generator: ${_path}")
      endif()
      file(READ "${_path}" _magic LIMIT 4 HEX)
      if(NOT _magic STREQUAL "7f454c46")
        message(FATAL_ERROR "Qt generator is not a Linux ELF executable: ${_path}")
      endif()
      set_property(TARGET Qt6::${_tool} PROPERTY IMPORTED_LOCATION "${_path}")
      get_target_property(_configs Qt6::${_tool} IMPORTED_CONFIGURATIONS)
      if(NOT _configs)
        set(_configs "")
      endif()
      list(
        APPEND _configs
        RELEASE
        RELWITHDEBINFO
        DEBUG
        MINSIZEREL
        NOCONFIG
        NONE
      )
      foreach(_config IN LISTS _configs)
        string(TOUPPER "${_config}" _config)
        set_property(TARGET Qt6::${_tool} PROPERTY IMPORTED_LOCATION_${_config} "${_path}")
      endforeach()
    endif()
  endforeach()
endfunction()
