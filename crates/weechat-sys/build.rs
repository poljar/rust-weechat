use std::{env, fs, path::PathBuf};

use bindgen::{BindgenError, Bindings};

const WEECHAT_BUNDLED_ENV: &str = "WEECHAT_BUNDLED";
const WEECHAT_PLUGIN_FILE_ENV: &str = "WEECHAT_PLUGIN_FILE";

fn build(file: &str) -> Result<Bindings, BindgenError> {
    const INCLUDED_TYPES: &[&str] = &[
        "t_weechat_plugin",
        "t_gui_buffer",
        "t_gui_nick",
        "t_gui_nick_group",
        "t_hook",
        "t_hdata",
    ];
    const INCLUDED_VARS: &[&str] = &[
        "WEECHAT_PLUGIN_API_VERSION",
        "WEECHAT_HASHTABLE_INTEGER",
        "WEECHAT_HASHTABLE_STRING",
        "WEECHAT_HASHTABLE_POINTER",
        "WEECHAT_HASHTABLE_BUFFER",
        "WEECHAT_HASHTABLE_TIME",
        "WEECHAT_HOOK_SIGNAL_STRING",
        "WEECHAT_HOOK_SIGNAL_INT",
        "WEECHAT_HOOK_SIGNAL_POINTER",
    ];
    let mut builder =
        bindgen::Builder::default().parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    builder = builder.header(file);

    for t in INCLUDED_TYPES {
        builder = builder.allowlist_type(t);
    }

    for v in INCLUDED_VARS {
        builder = builder.allowlist_var(v);
    }

    builder.generate()
}

fn header_api_version(file: &str) -> Option<String> {
    let contents = fs::read_to_string(file).ok()?;
    contents.lines().find_map(|line| {
        let line = line.trim();
        let version = line.strip_prefix("#define WEECHAT_PLUGIN_API_VERSION ")?;
        Some(version.trim_matches('"').to_string())
    })
}

fn report_header(label: &str, file: &str) {
    let path = PathBuf::from(file).canonicalize().unwrap_or_else(|_| PathBuf::from(file));
    match header_api_version(path.to_str().unwrap_or(file)) {
        Some(version) => println!(
            "cargo::warning=Using {label} WeeChat header: {} (API {version})",
            path.display()
        ),
        None => println!(
            "cargo::warning=Using {label} WeeChat header: {} (API version unknown)",
            path.display()
        ),
    }
}

fn main() {
    let bundled =
        env::var(WEECHAT_BUNDLED_ENV).is_ok_and(|bundled| match bundled.to_lowercase().as_ref() {
            "1" | "true" | "yes" => true,
            "0" | "false" | "no" => false,
            _ => panic!("Invalid value for WEECHAT_BUNDLED, must be true/false"),
        });

    let plugin_file = env::var(WEECHAT_PLUGIN_FILE_ENV);

    let bindings = if bundled {
        report_header("vendored", "src/weechat-plugin.h");
        build("src/weechat-plugin.h").expect("Unable to generate bindings")
    } else {
        match plugin_file {
            Ok(file) => {
                let path = PathBuf::from(file).canonicalize().expect("Can't canonicalize path");
                report_header("configured", path.to_str().unwrap_or_default());
                build(path.to_str().unwrap_or_default()).unwrap_or_else(|_| {
                    panic!("Unable to generate bindings with the provided {:?}", path)
                })
            }
            Err(_) => {
                report_header("system", "src/wrapper.h");
                build("src/wrapper.h").expect(
                    "Unable to generate bindings with the system weechat-plugin.h. \
                     Install the WeeChat development headers, set WEECHAT_PLUGIN_FILE \
                     to the full path of weechat-plugin.h, or set WEECHAT_BUNDLED=true \
                     to explicitly use the bundled header.",
                )
            }
        }
    };

    println!("cargo:rerun-if-env-changed={WEECHAT_BUNDLED_ENV}");
    println!("cargo:rerun-if-env-changed={WEECHAT_PLUGIN_FILE_ENV}");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("bindings.rs")).expect("Couldn't write bindings!");
}
