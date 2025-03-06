/// Keep only versions that break signatures in crate
#[allow(unused)]
#[repr(u64)]
enum WeechatApiVersions {
    V4_1_0 = 20230908,
    V4_2_0 = 20240105,
}

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=WEECHAT_BUNDLED");
    println!("cargo::rerun-if-env-changed=WEECHAT_PLUGIN_FILE");
    println!("cargo::rustc-check-cfg=cfg(weechat410)");
    println!("cargo::rustc-check-cfg=cfg(weechat420)");

    let (version, _) = std::str::from_utf8(weechat_sys::WEECHAT_PLUGIN_API_VERSION)
        .expect("Failed to parse weechat version string")
        .split_once('-')
        .expect("Failed to split weechat version string");

    println!("cargo::warning=WEECHAT_PLUGIN_API_VERSION: {version}");

    let version: u64 = version.parse().expect("Failed to parse weechat version string as u64");

    use crate::WeechatApiVersions::*;
    match version {
        v if v >= V4_2_0 as _ => {
            println!("cargo::rustc-cfg=weechat420");
        }
        v if v < V4_2_0 as _ => {
            println!("cargo::rustc-cfg=weechat410");
        }
        _ => {
            println!("cargo::error=Failed to match weechat API version: {version}");
        }
    }
}
