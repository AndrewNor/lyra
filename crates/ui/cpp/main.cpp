// Lyra Phase 0 — minimal C++ shim. All logic is in Rust; this only spins
// up the Qt event loop and loads the Rust-registered QML module.
#include <QtGui/QGuiApplication>
#include <QtQml/QQmlApplicationEngine>

int main(int argc, char *argv[]) {
    QGuiApplication app(argc, argv);
    QQmlApplicationEngine engine;
    engine.loadFromModule("ai.drivee.lyra", "Main");
    if (engine.rootObjects().isEmpty())
        return -1;
    return app.exec();
}
