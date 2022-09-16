#![recursion_limit = "256"]

extern crate proc_macro;
use std::collections::HashMap;

use proc_macro2::{Ident, Literal, Span};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream, Result},
    parse_macro_input,
    punctuated::Punctuated,
    Error, LitStr,
};

struct WeechatPluginInfo {
    plugin: syn::Ident,
    name: (usize, Literal, String),
    author: (usize, Literal),
    description: (usize, Literal),
    version: (usize, Literal),
    license: (usize, Literal),
}

enum WeechatVariable {
    Name(syn::LitStr),
    Author(syn::LitStr),
    Description(syn::LitStr),
    Version(syn::LitStr),
    License(syn::LitStr),
}

impl WeechatVariable {
    #[allow(clippy::wrong_self_convention)]
    fn to_pair(string: String) -> (usize, Literal) {
        let mut bytes = string.into_bytes();
        // Push a null byte since this goes to the C side.
        bytes.push(0);

        (bytes.len(), Literal::byte_string(&bytes))
    }

    fn as_pair(&self) -> (usize, Literal) {
        Self::to_pair(self.value())
    }

    fn default_literal() -> (usize, Literal) {
        let bytes = vec![0];
        (bytes.len(), Literal::byte_string(&bytes))
    }

    fn value(&self) -> String {
        match self {
            WeechatVariable::Name(v) => v.value(),
            WeechatVariable::Author(v) => v.value(),
            WeechatVariable::Description(v) => v.value(),
            WeechatVariable::Version(v) => v.value(),
            WeechatVariable::License(v) => v.value(),
        }
    }
}

impl Parse for WeechatVariable {
    fn parse(input: ParseStream) -> Result<Self> {
        let key: Ident = input.parse()?;
        input.parse::<syn::Token![:]>()?;
        let value = input.parse()?;

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
            Error::new(input.span(), "a struct that implements the Plugin trait needs to be given")
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
            name: variables
                .remove("name")
                .map_or_else(
                    || Err(Error::new(input.span(), "the name of the plugin needs to be defined")),
                    |v| {
                        let (length, byte_name) = v.as_pair();
                        Ok((length, byte_name, v.value()))
                    },
                )
                .unwrap(),
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
/// This configures the Weechat init and end method as well as additional plugin
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
    let WeechatPluginInfo { plugin, name, author, description, version, license } =
        parse_macro_input!(input as WeechatPluginInfo);

    let weechat = crate_name("weechat").expect("This plugin works only with the weechat crate");

    let weechat = match weechat {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!( #ident )
        }
    };

    let (name_len, name, name_string) = name;
    let (author_len, author) = author;
    let (description_len, description) = description;
    let (license_len, license) = license;
    let (version_len, version) = version;

    let name_method = Ident::new(&format!("weechat_plugin_name_{name_string}"), Span::call_site());

    let result = quote! {
        #[doc(hidden)]
        #[no_mangle]
        pub static weechat_plugin_api_version: [u8; #weechat::weechat_sys::WEECHAT_PLUGIN_API_VERSION_LENGTH] =
            *#weechat::weechat_sys::WEECHAT_PLUGIN_API_VERSION;

        #[doc(hidden)]
        #[no_mangle]
        pub static #name_method: [u8; #name_len] = *#name;

        #[doc(hidden)]
        #[no_mangle]
        pub static weechat_plugin_author: [u8; #author_len] = *#author;

        #[doc(hidden)]
        #[no_mangle]
        pub static weechat_plugin_description: [u8; #description_len] = *#description;

        #[doc(hidden)]
        #[no_mangle]
        pub static weechat_plugin_version: [u8; #version_len] = *#version;

        #[doc(hidden)]
        #[no_mangle]
        pub static weechat_plugin_license: [u8; #license_len] = *#license;

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
            plugin: *mut #weechat::weechat_sys::t_weechat_plugin,
            argc: #weechat::libc::c_int,
            argv: *mut *mut #weechat::libc::c_char,
        ) -> #weechat::libc::c_int {
            let weechat = unsafe {
                #weechat::Weechat::init_from_ptr(plugin)
            };
            let args = #weechat::Args::new(argc, argv);
            match <#plugin as #weechat::Plugin>::init(&weechat, args) {
                Ok(p) => {
                    unsafe {
                        __PLUGIN = Some(p);
                    }
                    return #weechat::weechat_sys::WEECHAT_RC_OK;
                }
                Err(_e) => {
                    return #weechat::weechat_sys::WEECHAT_RC_ERROR;
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
            _plugin: *mut #weechat::weechat_sys::t_weechat_plugin
        ) -> #weechat::libc::c_int {
            unsafe {
                __PLUGIN = None;
                #weechat::Weechat::free();
            }
            #weechat::weechat_sys::WEECHAT_RC_OK
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
