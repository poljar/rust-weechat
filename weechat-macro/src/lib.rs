#![recursion_limit = "256"]

extern crate proc_macro;
use proc_macro2::{Ident, TokenStream};
use std::collections::HashMap;

use syn::{
    parse::{Parse, ParseStream, Result},
    parse_macro_input,
    punctuated::Punctuated,
    Error, Expr,
};

use quote::quote;

struct WeechatPluginInfo {
    plugin: syn::Ident,

    // The first TokenStream is an expression that yields a usize, the second yields a string
    // literal (possibly via a macro)
    name: (TokenStream, TokenStream),
    author: (TokenStream, TokenStream),
    description: (TokenStream, TokenStream),
    version: (TokenStream, TokenStream),
    license: (TokenStream, TokenStream),
}

enum WeechatVariable {
    Name(syn::Expr),
    Author(syn::Expr),
    Description(syn::Expr),
    Version(syn::Expr),
    License(syn::Expr),
}

impl WeechatVariable {
    #[allow(clippy::wrong_self_convention)]
    fn to_pair(string: &Expr) -> (TokenStream, TokenStream) {
        // This will initialize the value of the statics weechat needs to read
        let init = quote! {
            // concat!() works on string literals (which may be created via another macro)
            ::std::concat!(#string, "\0").as_bytes()
        };

        // Luckily, this works in a const context so we can use it to get how long our array needs
        // to be
        let len = quote! {
            #init.len()
        };

        (len, init)
    }

    fn as_pair(&self) -> (TokenStream, TokenStream) {
        match self {
            WeechatVariable::Name(string) => WeechatVariable::to_pair(string),
            WeechatVariable::Author(string) => WeechatVariable::to_pair(string),
            WeechatVariable::Description(string) => WeechatVariable::to_pair(string),
            WeechatVariable::Version(string) => WeechatVariable::to_pair(string),
            WeechatVariable::License(string) => WeechatVariable::to_pair(string),
        }
    }

    fn default_literal() -> (TokenStream, TokenStream) {
        let init = quote! {
            ::std::concat!("", "\0").as_bytes()
        };

        let len = quote! {
            #init.len()
        };

        (len, init)
    }
}

impl Parse for WeechatVariable {
    fn parse(input: ParseStream) -> Result<Self> {
        let key: Ident = input.parse()?;
        input.parse::<syn::Token![:]>()?;
        let value: syn::Expr = input.parse()?;

        match key.to_string().to_lowercase().as_ref() {
            "name" => Ok(WeechatVariable::Name(value)),
            "author" => Ok(WeechatVariable::Author(value)),
            "description" => Ok(WeechatVariable::Description(value)),
            "version" => Ok(WeechatVariable::Version(value)),
            "license" => Ok(WeechatVariable::License(value)),
            _ => Err(Error::new(
                key.span(),
                "expected one of name, author, description, version or license",
            )),
        }
    }
}

impl Parse for WeechatPluginInfo {
    fn parse(input: ParseStream) -> Result<Self> {
        let plugin: syn::Ident = input.parse().map_err(|_e| {
            Error::new(
                input.span(),
                "a struct that implements the Plugin trait needs to be given",
            )
        })?;
        input.parse::<syn::Token![,]>()?;

        let args: Punctuated<WeechatVariable, syn::Token![,]> =
            input.parse_terminated(WeechatVariable::parse)?;
        let mut variables = HashMap::new();

        for arg in args.pairs() {
            let variable = arg.value();
            match variable {
                WeechatVariable::Name(_) => variables.insert("name", *variable),
                WeechatVariable::Author(_) => variables.insert("author", *variable),
                WeechatVariable::Description(_) => variables.insert("description", *variable),
                WeechatVariable::Version(_) => variables.insert("version", *variable),
                WeechatVariable::License(_) => variables.insert("license", *variable),
            };
        }

        Ok(WeechatPluginInfo {
            plugin,
            name: variables.remove("name").map_or_else(
                || {
                    Err(Error::new(
                        input.span(),
                        "the name of the plugin needs to be defined",
                    ))
                },
                |v| Ok(v.as_pair()),
            )?,
            author: variables
                .remove("author")
                .map_or_else(WeechatVariable::default_literal, |v| v.as_pair()),
            description: variables
                .remove("description")
                .map_or_else(WeechatVariable::default_literal, |v| v.as_pair()),
            version: variables
                .remove("version")
                .map_or_else(WeechatVariable::default_literal, |v| v.as_pair()),
            license: variables
                .remove("license")
                .map_or_else(WeechatVariable::default_literal, |v| v.as_pair()),
        })
    }
}

/// Register a struct that implements the `Plugin` trait as a Weechat plugin.
///
/// This configures the Weechat init and end method as well as additonal plugin
/// metadata.
///
/// # Example
/// ```
/// # use weechat::{plugin, Args, Weechat, Plugin};
/// # struct SamplePlugin;
/// # impl Plugin for SamplePlugin {
/// #    fn init(weechat: &Weechat, _args: Args) -> Result<Self, ()> {
/// #        Ok(SamplePlugin)
/// #    }
/// # }
/// plugin!(
///     SamplePlugin,
///     name: "rust_sample",
///     author: "poljar",
///     description: "",
///     version: "0.1.0",
///     license: "MIT"
/// );
/// ```
#[proc_macro]
pub fn plugin(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let WeechatPluginInfo {
        plugin,
        name,
        author,
        description,
        version,
        license,
    } = parse_macro_input!(input as WeechatPluginInfo);

    let (name_len, name) = name;
    let (author_len, author) = author;
    let (description_len, description) = description;
    let (license_len, license) = license;
    let (version_len, version) = version;

    let result = quote! {
        #[doc(hidden)]
        #[no_mangle]
        pub static weechat_plugin_api_version: [u8; weechat::weechat_sys::WEECHAT_PLUGIN_API_VERSION_LENGTH] =
            *weechat::weechat_sys::WEECHAT_PLUGIN_API_VERSION;

        // Each of these unsafe blocks is the reason this generates code only usable on a nightly compiler:
        // raw pointer dereferences specifically in const/static contexts is unstable. See this issue:
        // https://github.com/rust-lang/rust/issues/51911

        #[doc(hidden)]
        #[no_mangle]
        pub static weechat_plugin_name: [u8; #name_len] = unsafe { *(#name.as_ptr() as *const [u8; #name_len]) };

        #[doc(hidden)]
        #[no_mangle]
        pub static weechat_plugin_author: [u8; #author_len] = unsafe { *(#author.as_ptr() as *const [u8; #author_len]) };

        #[doc(hidden)]
        #[no_mangle]
        pub static weechat_plugin_description: [u8; #description_len] = unsafe { *(#description.as_ptr() as *const [u8; #description_len]) };

        #[doc(hidden)]
        #[no_mangle]
        pub static weechat_plugin_version: [u8; #version_len] = unsafe { *(#version.as_ptr() as *const [u8; #version_len]) };

        #[doc(hidden)]
        #[no_mangle]
        pub static weechat_plugin_license: [u8; #license_len] = unsafe { *(#license.as_ptr() as *const [u8; #license_len]) };

        #[doc(hidden)]
        static mut __PLUGIN: Option<#plugin> = None;

        /// This function is called when plugin is loaded by WeeChat.
        ///
        /// # Safety
        /// This function needs to be an extern C function and it can't be
        /// mangled, otherwise Weechat will not find the symbol.
        #[doc(hidden)]
        #[no_mangle]
        pub unsafe extern "C" fn weechat_plugin_init(
            plugin: *mut weechat::weechat_sys::t_weechat_plugin,
            argc: weechat::libc::c_int,
            argv: *mut *mut weechat::libc::c_char,
        ) -> weechat::libc::c_int {
            let weechat = unsafe {
                Weechat::init_from_ptr(plugin)
            };
            let args = Args::new(argc, argv);
            match <#plugin as ::weechat::Plugin>::init(&weechat, args) {
                Ok(p) => {
                    unsafe {
                        __PLUGIN = Some(p);
                    }
                    return weechat::weechat_sys::WEECHAT_RC_OK;
                }
                Err(_e) => {
                    return weechat::weechat_sys::WEECHAT_RC_ERROR;
                }
            }
        }

        /// This function is called when plugin is unloaded by WeeChat.
        ///
        /// # Safety
        /// This function needs to be an extern C function and it can't be
        /// mangled, otherwise Weechat will not find the symbol.
        #[doc(hidden)]
        #[no_mangle]
        pub unsafe extern "C" fn weechat_plugin_end(
            _plugin: *mut weechat::weechat_sys::t_weechat_plugin
        ) -> weechat::libc::c_int {
            unsafe {
                __PLUGIN = None;
                Weechat::free();
            }
            weechat::weechat_sys::WEECHAT_RC_OK
        }

        impl #plugin {
            /// Get a reference to our created plugin.
            ///
            /// # Panic
            ///
            /// Panics if this is called before the plugin `init()` method is
            /// done.
            pub fn get() -> &'static mut #plugin {
                unsafe {
                    match &mut __PLUGIN {
                        Some(p) => p,
                        None => panic!("Weechat plugin isn't initialized"),
                    }
                }
            }
        }
    };

    result.into()
}
