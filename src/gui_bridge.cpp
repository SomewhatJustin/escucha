#include "escucha/gui_bridge.h"

#include <QApplication>
#include <QQmlApplicationEngine>
#include <QUrl>

#include <array>

std::int32_t run_qml_app()
{
  int argc = 1;
  char app_name[] = "escucha";
  std::array<char*, 2> argv = { app_name, nullptr };

  QApplication app(argc, argv.data());
  QQmlApplicationEngine engine;
  engine.load(QUrl(QStringLiteral("qrc:/qt/qml/io/github/escucha/src/qml/Main.qml")));
  return app.exec();
}
