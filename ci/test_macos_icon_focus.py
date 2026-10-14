#!/usr/bin/env -S uv run --script
"""Compile actual frontend icon initialization against observable focus receivers (#725)."""

from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent

MODEL = r"""
#include <algorithm>
#include <cstdio>
#include <vector>
#include <string>
#define QT_VERSION_CHECK(a,b,c) (((a)<<16)|((b)<<8)|(c))
#define QT_VERSION QT_VERSION_CHECK(6,11,1)
#define SIGNAL(x) #x
constexpr int LOG_INFO=1, LOG_ERROR=2;
bool logging_open=false; int verified_logs=0;
void blog(int,const char*,...) { if(logging_open) ++verified_logs; }
void create_log_file(int&) { logging_open=true; }
struct QIcon {
    bool empty=true;
    QIcon()=default;
    QIcon(const char*) : empty(false) {}
    bool isNull() const { return empty; }
    static QIcon fromTheme(const char*, QIcon fallback) { return fallback; }
};
struct QGuiApplication;
struct QObject {
    int events=0;
    virtual ~QObject()=default;
    static bool disconnect(QGuiApplication*,const char*,QObject*,const char*);
};
struct QPlatformIntegration { virtual ~QPlatformIntegration()=default; };
struct CocoaIntegration : QObject, QPlatformIntegration {};
struct QGuiApplication : QObject {
    QIcon icon;
    std::vector<QObject*> focus;
    void setWindowIcon(QIcon value) { icon=value; }
    QIcon windowIcon() const { return icon; }
    static std::string platformName() { return "cocoa"; }
    void emitFocus() { for(auto *receiver:focus) if(receiver) ++receiver->events; }
};
QGuiApplication *qApp=nullptr;
bool QObject::disconnect(QGuiApplication *sender,const char *signal,QObject *receiver,const char*) {
    std::string normalized=signal ? signal : "focusWindowChanged(QWindow*)";
    normalized.erase(std::remove(normalized.begin(),normalized.end(),' '),normalized.end());
    if(normalized!="focusWindowChanged(QWindow*)") return false;
    bool removed=false;
    for(auto &entry:sender->focus) if(entry && (!receiver || entry==receiver)) {entry=nullptr; removed=true;}
    return removed;
}
struct QGuiApplicationPrivate {
    static QPlatformIntegration *integration;
    static QPlatformIntegration *platformIntegration() { return integration; }
};
QPlatformIntegration *QGuiApplicationPrivate::integration=nullptr;
"""


class MacIconFocus(unittest.TestCase):
    def compile_initialization(self, apple, logging=False):
        app = (ROOT / "frontend/OBSApp.cpp").read_text()
        start = app.index("sleepInhibitor = os_inhibit_sleep_create")
        start = app.index(";", start) + 1
        initialization = app[start : app.index("setDesktopFileName", start)]
        platform = (ROOT / "frontend/utility/platform-osx.mm").read_text()
        marker = "// Application icon policy (#725)."
        policy = (
            platform[platform.index(marker) : platform.index("bool isInBundle()")]
            if marker in platform
            else ""
        )
        post_log = ""
        if logging:
            main = (ROOT / "frontend/obs-main.cpp").read_text()
            post_log = main[
                main.rindex("\t\tif (!created_log) {") : main.index(
                    "\t\tprogram.checkForUncleanShutdown();"
                )
            ]
        source = (
            MODEL
            + policy
            + "\nstruct App : QGuiApplication { void initialize() {\n"
            + initialization
            + "\n} };\n"
            + r"""
int main() {
    App app; qApp=&app;
    CocoaIntegration cocoa; QObject menu, user;
    QGuiApplicationPrivate::integration=&cocoa;
    app.focus={&cocoa,&menu,&user};
    app.initialize(); app.emitFocus();
"""
            + ("bool created_log=false; int logFile=0;\n" + post_log if logging else "")
            + r"""
    std::printf("%d %d %d %d\n",!app.windowIcon().isNull(),cocoa.events,menu.events,user.events);
}
"""
        )
        if logging:
            source = source.replace(
                'std::printf("%d %d %d %d\\n",',
                'std::printf("%d %d %d %d %d\\n",verified_logs,',
            )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cpp = root / "icon.cpp"
            cpp.write_text(source)
            command = ["c++", "-std=c++17", "-Wall", "-Wextra", "-Werror"]
            if apple:
                command += ["-D__APPLE__"]
            compiled = subprocess.run(
                command + [str(cpp), "-o", str(root / "icon")],
                capture_output=True,
                text=True,
            )
            self.assertEqual(compiled.returncode, 0, compiled.stderr)
            return list(
                map(
                    int,
                    subprocess.check_output([str(root / "icon")], text=True).split(),
                )
            )

    def test_macos_explicit_icon_removes_only_the_generic_icon_focus_callback(self):
        self.assertEqual(self.compile_initialization(True), [1, 0, 1, 1])

    def test_other_platform_icon_and_focus_receivers_are_unchanged(self):
        self.assertEqual(self.compile_initialization(False), [1, 1, 1, 1])

    def test_macos_icon_policy_is_verified_after_persistent_logging_opens(self):
        self.assertEqual(
            self.compile_initialization(True, logging=True), [1, 1, 0, 1, 1]
        )


if __name__ == "__main__":
    unittest.main()
