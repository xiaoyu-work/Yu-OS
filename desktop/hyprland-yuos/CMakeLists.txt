cmake_minimum_required(VERSION 3.21)

project(yuos-compositor-adapter LANGUAGES CXX)

if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux")
    message(FATAL_ERROR "Yu OS compositor authority requires Linux")
endif()

set(YUOS_COMPOSITOR_LIBRARY "" CACHE FILEPATH "Built OS-private Rust authority static library")
if(NOT EXISTS "${YUOS_COMPOSITOR_LIBRARY}")
    message(FATAL_ERROR "Build yuos-compositor and provide YUOS_COMPOSITOR_LIBRARY")
endif()

find_package(PkgConfig REQUIRED)
pkg_check_modules(YUOS_WAYLAND REQUIRED IMPORTED_TARGET wayland-server>=1.22.91)
find_package(Threads REQUIRED)

add_library(yuos_compositor_rust STATIC IMPORTED GLOBAL)
set_target_properties(yuos_compositor_rust PROPERTIES
    IMPORTED_LOCATION "${YUOS_COMPOSITOR_LIBRARY}"
)

add_library(yuos_compositor_adapter STATIC YuOs.cpp)
set_target_properties(yuos_compositor_adapter PROPERTIES POSITION_INDEPENDENT_CODE ON)
target_compile_features(yuos_compositor_adapter PUBLIC cxx_std_20)
target_include_directories(yuos_compositor_adapter PUBLIC "${CMAKE_CURRENT_SOURCE_DIR}")
target_compile_options(yuos_compositor_adapter PRIVATE -Wall -Wextra -Wpedantic)
target_link_libraries(yuos_compositor_adapter PUBLIC
    yuos_compositor_rust PkgConfig::YUOS_WAYLAND Threads::Threads ${CMAKE_DL_LIBS} m
)

if(PROJECT_IS_TOP_LEVEL)
    enable_testing()
    add_executable(yuos_compositor_abi test/abi.cpp)
    target_link_libraries(yuos_compositor_abi PRIVATE yuos_compositor_adapter)
    add_test(NAME native-authority-refusal COMMAND yuos_compositor_abi)
endif()
