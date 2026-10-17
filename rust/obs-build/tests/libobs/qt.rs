//! Executes real OBS Qt widgets with generated UI and embedded resources.
use obs_build::{NativeBuild, SharedLibrary, frontend, qt};
use std::path::Path;

pub fn exercise(build: &NativeBuild, root: &Path) {
    let source = build.output.join("qt-widget-probe.cpp");
    if std::fs::read_to_string(&source).ok().as_deref() != Some(SOURCE) {
        std::fs::write(&source, SOURCE).unwrap();
    }
    let mut library = SharedLibrary {
        name: "qt-widget-probe".into(),
        sources: vec![source],
        include_dirs: vec![root.into()],
        ..SharedLibrary::default()
    };
    qt::uic(
        &mut library,
        &root.join("frontend/forms/OBSBasicVCamConfig.ui"),
        &build
            .output
            .join("qt-widget-probe/generated/ui_OBSBasicVCamConfig.h"),
    )
    .unwrap();
    qt::rcc(
        &mut library,
        &root.join("frontend/forms/obs.qrc"),
        &build.output.join("qt-widget-probe/generated/qrc_obs.cpp"),
    )
    .unwrap();
    frontend::add_spinbox(build, root, &mut library).unwrap();
    let executable = build.executable(&library).unwrap();
    let result = std::process::Command::new(executable)
        .env("QT_QPA_PLATFORM", "offscreen")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}\n{}",
        result.status,
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    println!("OBS Qt UI, resources and spinbox widget probe passed");
}

const SOURCE: &str = r#"
#include <QApplication>
#include <QDialog>
#include <QPixmap>
#include <QPushButton>
#include <cstring>
#include "ui_OBSBasicVCamConfig.h"
#include "shared/qt/obs-spinbox/SpinBox.hpp"
#include "shared/qt/obs-spinbox/DoubleSpinBox.hpp"

int main(int argc, char **argv)
{
    QApplication app(argc, argv);
    QDialog dialog;
    Ui::OBSBasicVCamConfig ui;
    ui.setupUi(&dialog);
    if (!dialog.findChild<QComboBox *>("outputType") || !dialog.findChild<QComboBox *>("outputSelection"))
        return 91;
    QPixmap icon(":/res/images/obs.png");
    if (icon.isNull())
        return 92;
    OBS::SpinBox spin(&dialog);
    spin.setRange(-10, 100);
    spin.setValue(42);
    OBS::DoubleSpinBox decimal(&dialog);
    decimal.setDecimals(3);
    decimal.setValue(3.125);
    if (spin.value() != 42 || decimal.value() != 3.125 ||
        strcmp(spin.metaObject()->className(), "OBS::SpinBox") != 0)
        return 93;
    dialog.show();
    app.processEvents();
    ui.buttonBox->button(QDialogButtonBox::Ok)->click();
    return dialog.result() == QDialog::Accepted ? 0 : 94;
}
"#;
