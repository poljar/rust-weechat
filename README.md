[![CI](https://github.com/poljar/rust-weechat/actions/workflows/ci.yml/badge.svg)](https://github.com/poljar/rust-weechat/actions/workflows/ci.yml)
[![Docs](https://docs.rs/weechat/badge.svg)](https://docs.rs/weechat)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

# rust-weechat

**rust-weechat** is a high level wrapper of the [Weechat][] plugin API in [Rust][].

[Weechat]: https://weechat.org/
[Rust]: https://www.rust-lang.org/

## Project structure

The project consists of three different crates.

- **weechat** - High level and idiomatic Rust library that allows to easily
  write [Weechat][] plugins.
- **weechat-macro** - Procedural macro implementations that allow you to define
  C entry functions that a Weechat plugin needs to define.
- **weechat-sys** - Auto-generated low level bindings to the Weechat plugin API.

## Status

The library is in an beta state, things that are implemented generally work some
breaking changes might still be introduced. A lot of the less used plugin APIs
aren't yet implemented.
