#![recursion_limit = "256"]

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{parse_macro_input, Error};

#[proc_macro_attribute]
pub fn weechat_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as syn::AttributeArgs);

    if !args.is_empty() {
        return Error::new(Span::call_site(), "no attributes are supported")
            .to_compile_error()
            .into();
    }

    let item = parse_macro_input!(item as syn::ItemFn);

    let syn::ItemFn { sig, block, .. } = item;

    let test_name = sig.ident;
    let test_body = block;

    let module_name = Ident::new(&format!("__{test_name}"), Span::call_site());

    quote! {
        #[test]
        fn #test_name() {
            use ::std::fs;

            #[cfg(all(unix, not(target_os = "macos")))]
            mod consts {
                pub const COMPILED_LIB_FILENAME: &str =
                    concat!("lib", env!("CARGO_CRATE_NAME"), ".so");

                pub const TARGET_LIB_FILENAME: &str =
                    concat!("__", stringify!(#test_name), ".so");
            }

            #[cfg(target_os = "macos")]
            mod consts {
                pub const COMPILED_LIB_FILENAME: &str =
                    concat!("lib", env!("CARGO_CRATE_NAME"), ".dylib");

                pub const TARGET_LIB_FILENAME: &str =
                    concat!("__", stringify!(#test_name), ".so");
            }

            #[cfg(target_os = "windows")]
            mod consts {
                pub const COMPILED_LIB_FILENAME: &str =
                    concat!(env!("CARGO_CRATE_NAME"), ".dll");

                pub const TARGET_LIB_FILENAME: &str =
                    concat!("__", stringify!(#test_name), ".dll");
            }

            let root = ::std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

            let from_path = root
                .join("target")
                .join("debug")
                .join(consts::COMPILED_LIB_FILENAME);

            if !from_path.exists() {
                panic!(
                    "Compiled library not found in '{}'. Please run `cargo \
                     build` before running the tests.",
                    from_path.display()
                )
            }

            let command = format!("/plugin load {}", from_path.to_str().unwrap());

            let out = ::std::process::Command::new("weechat-headless")
                .args(["--temp-dir", "--no-plugins"])
                .args(["--run-command", &command])
                .env("RUST_BACKTRACE", "1")
                // .args(["--run-command", "/quit"])
                .output()
                .expect("Couldn't find `nvim` binary in $PATH!");

            // panic!("HELLO {:?}", out);

            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                // Remove the last 2 lines from stderr for a cleaner error msg.
                // let lines = stderr.lines().collect::<Vec<_>>();
                // let len = lines.len();
                // let stderr = &lines[..lines.len() - 2].join("\n");
                // // The first 31 bytes are `thread '<unnamed>' panicked at `
                // let (_, stderr) = stderr.split_at(31);
                panic!("{}", stderr)
            }
        }

        struct #module_name;

        impl weechat::Plugin for #module_name {
            fn init(weechat: &weechat::Weechat, _args: weechat::Args) -> Result<Self, ()> {
                let result = ::std::panic::catch_unwind(|| {
                    #test_body
                });

                ::std::process::exit(match result {
                    Ok(_) => 0,

                    Err(err) => {
                        eprintln!("HELLO ERROR {:?}", err);
                        1
                    },
                })
            }
        }

        weechat::plugin!(
             #module_name,
             name: "my test",
             author: "Weechat Test Macro",
             description: "",
             version: "0.1.0",
             license: "MIT"
        );
    }
    .into()
}
