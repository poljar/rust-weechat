# go

Weechat go reimplementation in rust.

This is a port of the popular Python [go script] for Weechat. It uses a fuzzy
matching library to match buffers by their short name.

## Build

To build the plugin
```
make
```

Installation can be done like so

```
make install
```

By default this will install the plugin in your `$HOME/.weechat/plugins` directory.

### Picking the correct Weechat version.

By default the system-wide `weechat-plugin.h` file will be used if found,
this behaviour can be overridden with two environment flags.

To prefer a bundled include file `WEECHAT_BUNDLED` should be set to `true`. The
bundled include file tracks the latest Weechat release.

A custom include file can be set with the `WEECHAT_PLUGIN_FILE` environment
variable, this environment variable takes a full path to the include file.

After an adequate `weechat-plugin.h` file is found rebuild the plugin like so

```
WEECHAT_PLUGIN_FILE=/home/example/weechat-plugin.h make install
```

[go script]: https://weechat.org/scripts/source/go.py.html/
