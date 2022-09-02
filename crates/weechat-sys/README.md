# weechat-sys

Auto-generated bindings for the [Weechat] plugin [API]

This library needs the `weechat-plugin.h` include file. It will try to use the
system-wide installed one. If it can't find that one it will use a bundled one.

## Choosing the plugin include file.

By default the system-wide include file will be used if found, this behaviour
can be overridden with two environment flags.

To prefer the bundled include file `WEECHAT_BUNDLED` should be set to `true`.

A custom include file can be set with the `WEECHAT_PLUGIN_FILE` environment
variable, this environment variable takes a full path to the custom include
file.

[Weechat]: weechat.org/
[API]: https://weechat.org/files/doc/stable/weechat_plugin_api.en.html
