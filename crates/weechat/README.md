[![Build Status](https://travis-ci.org/poljar/rust-weechat.svg?branch=master)](https://travis-ci.org/poljar/rust-weechat)
[![Docs](https://docs.rs/weechat/badge.svg)](https://docs.rs/weechat)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

# Rust-Weechat

[Weechat](https://weechat.org/) is an extensible chat client.

[Rust-Weechat](https://github.com/poljar/rust-weechat/) is a high level Rust
library providing an API for building Weechat plugins.

It wraps the Weechat C plugin [API] as safe Rust bindings.

## Project Status

This project is in a decently stable state, many things that the Weechat plugin
API allows are exposed in a higher level safe API. Many things still need to be
figured out and exposed safely. Breaking changes might still get introduced.

Experimental or unsound features are gated behind feature flags.

## Example

Example plugins can be found in the [examples] part of the repository.

The following example shows a minimal working Rust plugin.

```rust
use weechat::{
    buffer::Buffer,
    weechat_plugin, Args, Weechat, Plugin,
};

struct HelloWorld;

impl Plugin for HelloWorld {
    fn init(_: &Weechat, _: Args) -> Result<Self, ()> {
        Weechat::print("Hello from Rust");
        Ok(Self)
    }
}

impl Drop for HelloWorld {
    fn drop(&mut self) {
        Weechat::print("Bye from Rust");
    }
}

weechat_plugin!(
    HelloWorld,
    name: "hello",
    author: "Damir Jelić <poljar@termina.org.uk>",
    description: "Simple hello world Rust plugin",
    version: "1.0.0",
    license: "MIT"
);
```

## Projects build with Rust-Weechat

* [Weechat-Matrix-rs](https://github.com/poljar/weechat-matrix-rs)
* [Weechat-Discord](https://github.com/terminal-discord/weechat-discord)

Are we missing a project? Submit a pull request and we'll get you added!
Just edit this `README.md` file.

## Picking the correct Weechat version.

By default the system-wide `weechat-plugin.h` file will be used if found,
this behaviour can be overridden with two environment flags.

To prefer a bundled include file `WEECHAT_BUNDLED` should be set to `true`. The
bundled include file tracks the latest Weechat release.

A custom include file can be set with the `WEECHAT_PLUGIN_FILE` environment
variable, this environment variable takes a full path to the include file.

[Weechat]: weechat.org/
[API]: https://weechat.org/files/doc/stable/weechat_plugin_api.en.html
[examples]: https://github.com/poljar/rust-weechat/tree/master/weechat/examples
