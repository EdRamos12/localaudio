cmake_minimum_required(VERSION 3.22.1)
project(localaudiojni)

set(OPUS_BUILD_SHARED_LIBRARY OFF CACHE BOOL "" FORCE)
set(OPUS_BUILD_TESTING OFF CACHE BOOL "" FORCE)
set(OPUS_BUILD_PROGRAMS OFF CACHE BOOL "" FORCE)
set(OPUS_INSTALL_PKG_CONFIG_MODULE OFF CACHE BOOL "" FORCE)
set(BUILD_TESTING OFF CACHE BOOL "" FORCE)

add_subdirectory(opus)

add_library(localaudiojni SHARED decoder.cpp)
target_link_libraries(localaudiojni opus)
target_include_directories(localaudiojni PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/opus/include)
