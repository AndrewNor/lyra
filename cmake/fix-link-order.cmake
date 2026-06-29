# fix-link-order.cmake
# Post-configure patch for build.ninja:
# GNU ld.bfd requires shared libs AFTER static archives that use them.
# Corrosion places crate-native libs (-lasound, -lpipewire-0.3) BEFORE
# liblyra_ui.a in the generated LINK_LIBRARIES. This script fixes the order.
#
# Usage (run after cmake configures):
#   cmake -P cmake/fix-link-order.cmake
# Or call it from the main CMakeLists via execute_process after generation.

set(NINJA_FILE "${CMAKE_CURRENT_LIST_DIR}/../build/build.ninja")
if(NOT EXISTS "${NINJA_FILE}")
    message(WARNING "fix-link-order: build.ninja not found at ${NINJA_FILE}")
    return()
endif()

file(READ "${NINJA_FILE}" NINJA_CONTENT)

# Move -lasound and -lpipewire-0.3 to after liblyra_ui.a
# Pattern: initializers.o  -lasound  -lpipewire-0.3  liblyra_ui.a
# Replace: initializers.o  liblyra_ui.a  -lasound  -lpipewire-0.3
string(REPLACE
    "initializers.o  -lasound  -lpipewire-0.3  liblyra_ui.a"
    "initializers.o  liblyra_ui.a  -lasound  -lpipewire-0.3"
    NINJA_CONTENT "${NINJA_CONTENT}"
)

file(WRITE "${NINJA_FILE}" "${NINJA_CONTENT}")
message(STATUS "fix-link-order: patched build.ninja link order")
