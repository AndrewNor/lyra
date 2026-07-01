cmake_minimum_required(VERSION 3.28)
project(lyra LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)   # CXX-Qt emits Q_OBJECT headers; moc is mandatory

# Prefer mold or lld for linking. GNU ld.bfd resolves static archives strictly
# left-to-right, so it fails on the libasound/libpipewire symbols pulled in
# transitively by the Rust static library (liblyra_ui.a) regardless of where we
# list them. mold and lld resolve archives order-independently, so a fresh
# build "just works" when one of them is present.
# Set LYRA_AUTO_LINKER=OFF and pass your own -fuse-ld via CMAKE_EXE_LINKER_FLAGS
# when the environment needs a specific linker (e.g. the Flatpak build forces
# lld from the llvm SDK extension).
option(LYRA_AUTO_LINKER "Auto-select mold/lld linker when available" ON)
if(LYRA_AUTO_LINKER)
    find_program(MOLD_LINKER mold)
    find_program(LLD_LINKER ld.lld)
    if(MOLD_LINKER)
        add_link_options("-fuse-ld=mold")
        message(STATUS "lyra: linking with mold")
    elseif(LLD_LINKER)
        add_link_options("-fuse-ld=lld")
        message(STATUS "lyra: linking with lld")
    else()
        message(STATUS "lyra: using default linker (install mold or lld if linking fails)")
    endif()
endif()

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

# ── Install rules ─────────────────────────────────────────────────────────────
# Used by `cmake --install`, and by the packaging targets (.deb via CPack,
# AppImage, Flatpak) to lay the app out under a standard prefix.
include(GNUInstallDirs)

# The Rust/cxx-qt link produces a binary without a DT_RUNPATH slot, so CMake's
# install-time RPATH rewrite fails. We don't need it: for the .deb the app finds
# Qt/Kirigami on the standard system path, and the AppImage/Flatpak builds manage
# their own bundling and rpaths.
set(CMAKE_SKIP_INSTALL_RPATH ON)

install(TARGETS lyra RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR})
install(FILES packaging/ai.drivee.lyra.desktop
        DESTINATION ${CMAKE_INSTALL_DATADIR}/applications)
install(FILES packaging/ai.drivee.lyra.metainfo.xml
        DESTINATION ${CMAKE_INSTALL_DATADIR}/metainfo)
install(FILES packaging/ai.drivee.lyra.svg
        DESTINATION ${CMAKE_INSTALL_DATADIR}/icons/hicolor/scalable/apps)

# ── CPack (.deb) ──────────────────────────────────────────────────────────────
set(CPACK_PACKAGE_NAME "lyra")
set(CPACK_PACKAGE_VERSION "0.1.0")
set(CPACK_PACKAGE_CONTACT "Andrew Nor <Andrew.n@drivee.ai>")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "Native music player for KDE Plasma")
set(CPACK_PACKAGE_HOMEPAGE_URL "https://github.com/AndrewNor/lyra")
set(CPACK_GENERATOR "DEB")
set(CPACK_DEBIAN_PACKAGE_SECTION "sound")
set(CPACK_DEBIAN_PACKAGE_MAINTAINER "Andrew Nor <Andrew.n@drivee.ai>")
# Let CPack discover the shared-library dependencies from the ELF binary.
set(CPACK_DEBIAN_PACKAGE_SHLIBDEPS ON)
# Kirigami and the QML runtime are dlopen'd, so shlibdeps can't see them —
# name them explicitly. (The Qt/PipeWire/ALSA shared libs are found by
# shlibdeps, including the correct t64 package names, so they're not listed.)
set(CPACK_DEBIAN_PACKAGE_DEPENDS
    "qml6-module-org-kde-kirigami, qml6-module-qtquick-controls, qml6-module-qtquick-layouts")
include(CPack)
