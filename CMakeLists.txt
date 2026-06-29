cmake_minimum_required(VERSION 3.28)
project(lyra LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)   # CXX-Qt emits Q_OBJECT headers; moc is mandatory

find_package(Qt6 6.8 REQUIRED COMPONENTS Core Gui Qml Quick QuickControls2 QmlImportScanner)
qt_standard_project_setup(REQUIRES 6.8)

# Corrosion (CMake <-> Cargo). Fetched at configure time (needs network once).
include(FetchContent)
FetchContent_Declare(Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG        v0.5)
FetchContent_MakeAvailable(Corrosion)

# cxx-qt-cmake: provides cxx_qt_import_crate / cxx_qt_import_qml_module.
# Pin to 0.8.1 to match the crates.
find_package(CxxQt 0.8 QUIET)
if(NOT CxxQt_FOUND)
    FetchContent_Declare(CxxQt
        GIT_REPOSITORY https://github.com/KDAB/cxx-qt-cmake.git
        GIT_TAG        0.8.1)
    FetchContent_MakeAvailable(CxxQt)
endif()

# Import the ui crate via Corrosion; cxx-qt wires the Qt link.
# QT_MODULES must be Qt::<Component> (alias targets).
cxx_qt_import_crate(
    MANIFEST_PATH crates/ui/Cargo.toml
    CRATES lyra_ui
    LOCKED
    QT_MODULES Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2
)

# Declare the QML module so CMake knows where plugin_init.o lives.
# SOURCE_CRATE must be the Corrosion-normalised name (hyphens -> underscores).
# The configure log prints: "CXX-Qt Found crate(s): lyra_ui"
cxx_qt_import_qml_module(lyra_ui_qml_module
    URI "ai.drivee.lyra"
    SOURCE_CRATE lyra_ui
)

add_executable(lyra crates/ui/cpp/main.cpp)

# Link only to the QML-module target; it transitively pulls in lyra-ui + Qt.
target_link_libraries(lyra PRIVATE
    lyra_ui_qml_module
    Qt6::Core Qt6::Gui Qt6::Qml Qt6::Quick Qt6::QuickControls2
)

# GNU ld.bfd link-order fix: cpal pulls in libasound and libpipewire-0.3 via
# the alsa and pipewire-sys crates. These must appear AFTER liblyra_ui.a in
# the linker command (GNU ld processes archives left-to-right). Corrosion
# places them before the archive by default; we add them here so CMake appends
# them after the archive's Qt deps. A post-configure build.ninja patch (see
# cmake/fix-link-order.cmake) corrects the remaining ordering issue when using
# ld.bfd. On systems with mold or lld these flags are unnecessary.
target_link_libraries(lyra PRIVATE
    asound
    pipewire-0.3
)

# GNU ld.bfd fails with "no more archived files" on large debug archives.
# Run ranlib before linking to refresh the symbol table index.
find_program(RANLIB_EXE ranlib REQUIRED)
add_custom_target(ranlib_lyra_ui ALL
    COMMAND ${RANLIB_EXE} "${CMAKE_BINARY_DIR}/liblyra_ui.a"
    DEPENDS cargo-build_lyra_ui
    COMMENT "ranlib: refreshing symbol table in liblyra_ui.a"
    VERBATIM
)
add_dependencies(lyra ranlib_lyra_ui)

# For statically-linked Qt: scan and pull in QML plugin initialisers.
qt_import_qml_plugins(lyra)
