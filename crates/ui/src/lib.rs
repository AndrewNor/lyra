//! lyra_ui: thin CXX-Qt bridge between the Rust core and QML/Kirigami.
//! Compiled as a staticlib and linked by CMake/Corrosion into the Qt app.

pub mod bridge;
pub mod library;
pub mod paths;
pub mod player;
