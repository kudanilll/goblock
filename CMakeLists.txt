# Set policy CMP0072 to prefer GLVND
cmake_policy(SET CMP0072 NEW)

# Minimum version of CMake required
cmake_minimum_required(VERSION 3.10)

# Project name and version
project(goblock VERSION 1.0)

# Specify C++ standard
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED True)

# Debug configuration
set(CMAKE_BUILD_TYPE Debug)  # Tambahkan ini untuk informasi debug

# Include source and header files
file(GLOB_RECURSE SOURCES "src/*.cpp")
file(GLOB_RECURSE HEADERS "include/*.h")

# Output directory for build
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
set(SRC_DIR ${CMAKE_SOURCE_DIR}/src)
set(INCLUDE_DIR ${CMAKE_SOURCE_DIR}/include)

configure_file(${CMAKE_CURRENT_SOURCE_DIR}/config.h.in ${CMAKE_CURRENT_BINARY_DIR}/config.h)
include_directories(${CMAKE_CURRENT_BINARY_DIR})

# Include directories for headers
include_directories(${INCLUDE_DIR})

# Add the executable
add_executable(goblock ${SOURCES} ${HEADERS} ${GLUT_INCLUDE_DIRS})

# Find required packages
find_package(OpenGL REQUIRED)
find_package(GLEW REQUIRED)
find_package(glfw3 REQUIRED)
find_package(GLUT REQUIRED)

# Find GLM manually
find_path(GLM_INCLUDE_DIR glm/glm.hpp)
if (NOT GLM_INCLUDE_DIR)
    message(FATAL_ERROR "GLM not found!")
endif()
include_directories(${GLM_INCLUDE_DIR})

# Link required libraries
target_link_libraries(goblock PRIVATE
    glfw
    GLEW::GLEW
    OpenGL::GL
    ${GLUT_LIBRARIES}
)
