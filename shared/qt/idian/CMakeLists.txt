cmake_minimum_required(VERSION 3.28...3.30)

find_package(Qt6 REQUIRED Core Widgets Svg)

add_library(idian STATIC)
add_library(OBS::idian ALIAS idian)

target_sources(
  idian
  PRIVATE
    build/idian.core+.hpp.cpp
    include/Idian/CheckBox.hpp
    include/Idian/CollapsibleGroup.hpp
    include/Idian/ComboBox.hpp
    include/Idian/ExpandButton.hpp
    include/Idian/Idian.hpp
    include/Idian/InlineButton.hpp
    include/Idian/ListHeader.hpp
    include/Idian/Row.hpp
    include/Idian/RowDoubleSpinBox.hpp
    include/Idian/RowInfo.hpp
    include/Idian/RowList.hpp
    include/Idian/RowSpinBox.hpp
    include/Idian/StateEventFilter.hpp
    include/Idian/ToggleSwitch.hpp
    include/Idian/Utils.hpp
)
target_sources(idian PUBLIC include/Idian/Idian.hpp)

set_target_properties(idian PROPERTIES AUTOMOC ON FOLDER "frontend")

target_include_directories(idian PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}/include")

target_link_libraries(idian PUBLIC Qt::Core Qt::Widgets Qt::Svg)

# #2625: this library's C++ sources state their conversions at the call sites,
# so the pins that hide them are not needed here. The global -Wno-conversion /
# -Wno-sign-conversion / -Wno-float-conversion only leave the flag lists once
# every target is ratcheted.
target_compile_options(
  idian
  PRIVATE "$<$<COMPILE_LANG_AND_ID:CXX,AppleClang,Clang,GNU>:-Wconversion;-Wsign-conversion;-Wfloat-conversion>"
)
