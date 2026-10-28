cmake_minimum_required(VERSION 3.20)

project(goblock VERSION 1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED True)

# Default to Debug; override with -DCMAKE_BUILD_TYPE=Release
if(NOT CMAKE_BUILD_TYPE AND NOT CMAKE_CONFIGURATION_TYPES)
    set(CMAKE_BUILD_TYPE Debug CACHE STRING "Build type" FORCE)
endif()

file(GLOB_RECURSE SOURCES CONFIGURE_DEPENDS "src/*.cpp")
file(GLOB_RECURSE HEADERS CONFIGURE_DEPENDS "src/*.h")

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)

# Forward slashes keep the generated string literals valid on Windows
file(TO_CMAKE_PATH "${PROJECT_SOURCE_DIR}" PROJECT_SOURCE_DIR_POSIX)
set(SHADERS_DIR_POSIX "${PROJECT_SOURCE_DIR_POSIX}/shaders")
set(FONT_PATH_POSIX "${PROJECT_SOURCE_DIR_POSIX}/assets/fonts/inter-medium.ttf")
configure_file(${CMAKE_CURRENT_SOURCE_DIR}/config.h.in ${CMAKE_CURRENT_BINARY_DIR}/config.h)

# Dependencies (vcpkg or system)
find_package(OpenGL REQUIRED)
find_package(GLEW REQUIRED)
find_package(glfw3 REQUIRED)
find_package(GLUT REQUIRED)
find_package(glm CONFIG REQUIRED)

add_executable(goblock ${SOURCES} ${HEADERS})

target_include_directories(goblock PRIVATE
    ${CMAKE_CURRENT_BINARY_DIR}
    ${CMAKE_CURRENT_SOURCE_DIR}/include
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

target_link_libraries(goblock PRIVATE
    glfw
    GLEW::GLEW
    OpenGL::GL
    GLUT::GLUT
    glm::glm
)

# Copy runtime DLLs next to the executable on Windows (vcpkg dynamic builds)
if(WIN32)
    add_custom_command(TARGET goblock POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
            $<TARGET_RUNTIME_DLLS:goblock> $<TARGET_FILE_DIR:goblock>
        COMMAND_EXPAND_LISTS
    )

    # MinGW runtime DLLs are not covered by TARGET_RUNTIME_DLLS
    if(MINGW)
        get_filename_component(MINGW_BIN_DIR "${CMAKE_CXX_COMPILER}" DIRECTORY)
        foreach(runtime_dll libgcc_s_seh-1.dll libstdc++-6.dll libwinpthread-1.dll)
            if(EXISTS "${MINGW_BIN_DIR}/${runtime_dll}")
                add_custom_command(TARGET goblock POST_BUILD
                    COMMAND ${CMAKE_COMMAND} -E copy_if_different
                        "${MINGW_BIN_DIR}/${runtime_dll}" $<TARGET_FILE_DIR:goblock>
                )
            endif()
        endforeach()
    endif()
endif()
