use crate::harness::{output, root};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if entry.file_type()?.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
fn licenses(destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)?;
    let vendor = root().join("vendor/codediff.nvim");
    let target = destination.join("codediff.nvim");
    fs::create_dir_all(&target)?;
    for name in [
        "LICENSE",
        "ATTRIBUTION.md",
        "UPSTREAM.md",
        "libvscode-diff/vendor/utf8proc_LICENSE.md",
    ] {
        fs::copy(
            vendor.join(name),
            target.join(Path::new(name).file_name().unwrap()),
        )?;
    }
    copy_tree(
        &root().join("crates/app/assets/fonts"),
        &destination.join("fonts"),
    )?;
    copy_tree(
        &root().join("crates/app/assets/icons"),
        &destination.join("icons"),
    )?;
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root())
        .args(["metadata", "--format-version", "1", "--locked"]);
    let metadata: Value = serde_json::from_slice(&output(cmd)?)?;
    for package in metadata["packages"]
        .as_array()
        .context("Cargo packages missing")?
    {
        let manifest = Path::new(package["manifest_path"].as_str().unwrap());
        for entry in fs::read_dir(manifest.parent().unwrap())? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if entry.file_type()?.is_file()
                && ["LICENSE", "LICENCE", "COPYING", "NOTICE"]
                    .iter()
                    .any(|prefix| name.starts_with(prefix))
            {
                let target = destination.join(format!(
                    "{}-{}",
                    package["name"].as_str().unwrap(),
                    package["version"].as_str().unwrap()
                ));
                fs::create_dir_all(&target)?;
                fs::copy(entry.path(), target.join(name.as_ref()))?;
            }
        }
    }
    let info=metadata["packages"].as_array().unwrap().iter().map(|p|serde_json::json!({"name":p["name"],"version":p["version"],"license":p["license"],"repository":p["repository"]})).collect::<Vec<_>>();
    fs::write(
        destination.join("dependencies.json"),
        serde_json::to_vec_pretty(&info)?,
    )?;
    Ok(())
}
pub fn run(debug: bool, output_dir: Option<PathBuf>) -> Result<()> {
    let shell = xshell::Shell::new()?;
    let _cwd = shell.push_dir(root());
    if debug {
        xshell::cmd!(shell, "cargo build --workspace --locked").run()?;
    } else {
        xshell::cmd!(shell, "cargo build --workspace --locked --release").run()?;
    }
    let binaries = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root().join("target"))
        .join(if debug { "debug" } else { "release" });
    let destination = output_dir.unwrap_or_else(|| root().join("target/package"));
    fs::create_dir_all(&destination)?;
    let staging = tempfile::Builder::new()
        .prefix(".package-")
        .tempdir_in(&destination)?;
    let app = staging.path().join(if cfg!(target_os = "macos") {
        "Terminator.app"
    } else {
        "terminator"
    });
    let executables = if cfg!(target_os = "macos") {
        app.join("Contents/MacOS")
    } else {
        app.clone()
    };
    fs::create_dir_all(&executables)?;
    for name in ["terminator", "terminator-daemon", "terminator-hook"] {
        fs::copy(binaries.join(name), executables.join(name))?;
    }
    if cfg!(target_os = "macos") {
        let mut info = plist::Dictionary::new();
        for (key, value) in [
            ("CFBundleIdentifier", "dev.terminator.app"),
            ("CFBundleName", "Terminator"),
            ("CFBundleDisplayName", "Terminator"),
            ("CFBundleExecutable", "terminator"),
            ("CFBundlePackageType", "APPL"),
            ("CFBundleVersion", "1"),
            ("CFBundleShortVersionString", env!("CARGO_PKG_VERSION")),
            ("LSMinimumSystemVersion", "12.0"),
        ] {
            info.insert(key.into(), value.into());
        }
        info.insert("NSHighResolutionCapable".into(), true.into());
        plist::Value::Dictionary(info).to_file_xml(app.join("Contents/Info.plist"))?;
        let resources = app.join("Contents/Resources");
        fs::create_dir_all(&resources)?;
        fs::copy(root().join("LICENSE"), resources.join("LICENSE"))?;
        copy_tree(&root().join("docs"), &resources.join("docs"))?;
        licenses(&resources.join("licenses"))?;
        let mut sign = Command::new("codesign");
        sign.args(["--force", "--deep", "--sign", "-"]).arg(&app);
        output(sign)?;
        let target = destination.join("Terminator.app");
        let backup = destination.join("Terminator.app.previous");
        ensure!(
            !backup.exists(),
            "Previous package backup already exists: {}",
            backup.display()
        );
        if target.exists() {
            fs::rename(&target, &backup)?;
        }
        if let Err(error) = fs::rename(&app, &target) {
            if backup.exists() {
                fs::rename(&backup, &target)?;
            }
            return Err(error.into());
        }
        if backup.exists() {
            fs::remove_dir_all(backup)?;
        }
        println!("{}", target.display());
    } else {
        fs::write(
            app.join("terminator.desktop"),
            "[Desktop Entry]\nType=Application\nName=Terminator\nExec=terminator\nTerminal=false\nCategories=Development;TerminalEmulator;\n",
        )?;
        fs::copy(root().join("README.md"), app.join("README.md"))?;
        licenses(&app.join("licenses"))?;
        let file = fs::File::create(staging.path().join("terminator-linux.tar.gz"))?;
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(encoder);
        tar.append_dir_all("terminator", &app)?;
        tar.into_inner()?.finish()?;
        let target = destination.join("terminator-linux.tar.gz");
        fs::rename(staging.path().join("terminator-linux.tar.gz"), &target)?;
        println!("{}", target.display());
    }
    Ok(())
}
