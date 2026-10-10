cmake_minimum_required(VERSION 3.22.1)

project(scanner_sensors_jni C)

add_library(scanner_sensors_jni SHARED
    scanner_core_jni.c
)

target_include_directories(scanner_sensors_jni PRIVATE
    "${CMAKE_CURRENT_LIST_DIR}/../../../../../../native"
)

target_link_options(scanner_sensors_jni PRIVATE
    "-Wl,-z,max-page-size=16384"
    "-Wl,-z,common-page-size=16384"
)

target_link_libraries(scanner_sensors_jni
    dl
    log
)
