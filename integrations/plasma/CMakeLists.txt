cmake_minimum_required(VERSION 3.21)
project(lconnect_mini_plasma VERSION 0.1.0 LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
include(GNUInstallDirs)
include(CTest)
find_package(Qt6 6.6 REQUIRED COMPONENTS Core Qml)
qt_policy(SET QTP0001 NEW)
find_program(NODE_EXECUTABLE node REQUIRED)
set(MODEL_SOURCE "${CMAKE_CURRENT_SOURCE_DIR}/../common/mini.ts")
set(TSC "${CMAKE_CURRENT_SOURCE_DIR}/../../desktop/node_modules/typescript/bin/tsc")
if(NOT EXISTS "${TSC}")
 message(FATAL_ERROR "Install desktop workspace dependencies before building the shared TypeScript model")
endif()
set(MODEL_DIR "${CMAKE_CURRENT_BINARY_DIR}/common")
add_custom_command(OUTPUT "${MODEL_DIR}/mini.mjs"
 COMMAND ${CMAKE_COMMAND} -E make_directory "${MODEL_DIR}"
 COMMAND ${NODE_EXECUTABLE} "${TSC}" "${MODEL_SOURCE}" --strict --target ES2017 --module ES2020 --declaration --skipLibCheck --outDir "${MODEL_DIR}"
 COMMAND ${CMAKE_COMMAND} -E copy "${MODEL_DIR}/mini.js" "${MODEL_DIR}/mini.mjs"
 DEPENDS "${MODEL_SOURCE}" VERBATIM)
add_custom_target(shared_model ALL DEPENDS "${MODEL_DIR}/mini.mjs")
qt_add_library(lconnect_mini_feed SHARED)
qt_add_qml_module(lconnect_mini_feed URI net.inkyquill.lconnect VERSION 1.0
 OUTPUT_DIRECTORY "${CMAKE_CURRENT_BINARY_DIR}/qml/net/inkyquill/lconnect"
 SOURCES native/feed.h native/feed.cpp)
target_include_directories(lconnect_mini_feed PRIVATE native)
target_link_libraries(lconnect_mini_feed PUBLIC Qt6::Core Qt6::Qml)
target_compile_options(lconnect_mini_feed PRIVATE -Wall -Wextra -Werror)
set_target_properties(lconnect_mini_feed lconnect_mini_feedplugin PROPERTIES INSTALL_RPATH "$ORIGIN")
set(PACKAGE_DIR "${CMAKE_CURRENT_BINARY_DIR}/package")
add_custom_target(widget_package ALL
 COMMAND ${CMAKE_COMMAND} -E copy_directory "${CMAKE_CURRENT_SOURCE_DIR}/package" "${PACKAGE_DIR}"
 COMMAND ${CMAKE_COMMAND} -E make_directory "${PACKAGE_DIR}/contents/code"
 COMMAND ${CMAKE_COMMAND} -E copy "${MODEL_DIR}/mini.mjs" "${PACKAGE_DIR}/contents/code/mini.mjs"
 DEPENDS shared_model)
set(QML_INSTALL_DIR "${CMAKE_INSTALL_LIBDIR}/qt6/qml/net/inkyquill/lconnect" CACHE PATH "QML module destination")
install(TARGETS lconnect_mini_feed lconnect_mini_feedplugin LIBRARY DESTINATION "${QML_INSTALL_DIR}")
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/qml/net/inkyquill/lconnect/qmldir" "${CMAKE_CURRENT_BINARY_DIR}/qml/net/inkyquill/lconnect/lconnect_mini_feed.qmltypes" DESTINATION "${QML_INSTALL_DIR}")
install(DIRECTORY "${PACKAGE_DIR}/" DESTINATION "${CMAKE_INSTALL_DATADIR}/plasma/plasmoids/net.inkyquill.lconnect.mini")
if(BUILD_TESTING)
 find_package(Qt6 REQUIRED COMPONENTS Quick Test)
 find_package(Python3 REQUIRED COMPONENTS Interpreter)
 configure_file(tests/tst_config.qml.in "${CMAKE_CURRENT_BINARY_DIR}/qml-tests/tst_config.qml" @ONLY)
 add_test(NAME config_keyboard COMMAND ${Python3_EXECUTABLE} "${CMAKE_CURRENT_SOURCE_DIR}/tests/test-config.py" "${CMAKE_CURRENT_BINARY_DIR}")
 set_tests_properties(config_keyboard PROPERTIES TIMEOUT 40)
 qt_add_executable(feed_test tests/feed_test.cpp native/feed.cpp)
 # This C++ transport test does not import the applet QML. QML tests run
 # separately with installed runtime modules, without static plugin linking.
 set_target_properties(feed_test PROPERTIES QT_QML_MODULE_NO_IMPORT_SCAN TRUE)
 target_include_directories(feed_test PRIVATE native)
 target_link_libraries(feed_test PRIVATE Qt6::Core Qt6::Qml Qt6::Quick Qt6::Test)
 target_compile_options(feed_test PRIVATE -Wall -Wextra -Werror)
 add_test(NAME feed COMMAND feed_test)
 set_tests_properties(feed PROPERTIES ENVIRONMENT "QT_QPA_PLATFORM=offscreen" TIMEOUT 45)
 add_test(NAME shared_model COMMAND ${NODE_EXECUTABLE} --test "${CMAKE_CURRENT_SOURCE_DIR}/../common/tests/mini.test.mjs")
 set_tests_properties(shared_model PROPERTIES ENVIRONMENT "LCONNECT_MINI_MODEL=${MODEL_DIR}/mini.mjs")
endif()
