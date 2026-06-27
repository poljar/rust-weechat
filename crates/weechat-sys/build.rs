use std::{env, path::PathBuf};

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

fn main() {
    let bundled =
        env::var(WEECHAT_BUNDLED_ENV).is_ok_and(|bundled| match bundled.to_lowercase().as_ref() {
            "1" | "true" | "yes" => true,
            "0" | "false" | "no" => false,
            _ => panic!("Invalid value for WEECHAT_BUNDLED, must be true/false"),
        });

    let plugin_file = env::var(WEECHAT_PLUGIN_FILE_ENV);

    let bindings = plugin_file
        .map(|file| {
            let path = PathBuf::from(file);
            path.canonicalize().unwrap_or(path)
        })
        .map(|path| {
            println!("cargo::warning=Using system header with path: {path:?}");
            build(path.to_str().unwrap_or_default())
        })
        .unwrap_or_else(|_| {
            println!("cargo::warning=Using system header via wrapper.h");
            build("src/wrapper.h")
        });

    let bindings = match bindings {
        Ok(b) => b,
        Err(err) => {
            if std::env::var("CARGO_FEATURE_VENDOR").is_ok() || bundled {
                println!("cargo::warning=Using vendored header");
                build("src/weechat-plugin.h").expect("Unable to generate bindings")
            } else {
                panic!("\n!!!!!\nUnable to build weechat-plugin bindings and vendored bindings are not enabled: {err}!!!!!\n");
            }
        }
    };

    println!("cargo:rerun-if-env-changed={WEECHAT_BUNDLED_ENV}");
    println!("cargo:rerun-if-env-changed={WEECHAT_PLUGIN_FILE_ENV}");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings.write_to_file(out_path.join("bindings.rs")).expect("Couldn't write bindings!");
}
