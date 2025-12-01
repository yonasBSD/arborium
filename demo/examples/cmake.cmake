# CMakeLists.txt example
cmake_minimum_required(VERSION 3.20)
project(MyProject VERSION 1.0.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Find packages
find_package(Threads REQUIRED)
find_package(OpenSSL REQUIRED)

# Add library
add_library(mylib STATIC
    src/lib.cpp
    src/utils.cpp
)

target_include_directories(mylib PUBLIC
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
    $<INSTALL_INTERFACE:include>
)

# Add executable
add_executable(myapp src/main.cpp)
target_link_libraries(myapp PRIVATE
    mylib
    Threads::Threads
    OpenSSL::SSL
)

# Install rules
install(TARGETS myapp mylib
    RUNTIME DESTINATION bin
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
)
