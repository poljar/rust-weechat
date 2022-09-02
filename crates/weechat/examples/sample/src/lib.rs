use std::{borrow::Cow, time::Instant};
use weechat::{
    buffer::{Buffer, BufferBuilder, NickSettings},
    config::{
        BooleanOption, BooleanOptionSettings, Conf, Config,
        ConfigSectionSettings,
    },
    hooks::{BarItem, Command, CommandSettings, SignalData, SignalHook},
    plugin, Args, Plugin, ReturnCode, Weechat,
};

struct SamplePlugin {
    _rust_hook: Command,
    _rust_config: Config,
    _item: BarItem,
    _signal: SignalHook,
}

impl SamplePlugin {
    fn input_cb(
        _weechat: &Weechat,
        buffer: &Buffer,
        input: Cow<str>,
    ) -> Result<(), ()> {
        buffer.print(&input);
        Ok(())
    }

    fn close_cb(_weechat: &Weechat, _buffer: &Buffer) -> Result<(), ()> {
        Weechat::print("Closing buffer");
        Ok(())
    }

    fn rust_command_cb(_weechat: &Weechat, buffer: &Buffer, args: Args) {
        buffer.print("Hello world");

        for arg in args {
            buffer.print(&arg)
        }
    }

    fn option_change_cb(_weechat: &Weechat, _option: &BooleanOption) {
        Weechat::print("Changing rust option");
    }
}

impl Plugin for SamplePlugin {
    fn init(_: &Weechat, _args: Args) -> Result<Self, ()> {
        Weechat::print("Hello Rust!");

        let buffer_handle = BufferBuilder::new("Test buffer")
            .input_callback(SamplePlugin::input_cb)
            .close_callback(SamplePlugin::close_cb)
            .build()
            .expect("Can't create buffer");

        let buffer = buffer_handle.upgrade().expect("Buffer already closed?");

        buffer.print("Hello test buffer");

        let n = 100;

        let now = Instant::now();

        let op_group = buffer
            .add_nicklist_group("operators", "blue", true, None)
            .expect("Can't create nick group");
        let emma = op_group
            .add_nick(
                NickSettings::new("Emma")
                    .set_color("magenta")
                    .set_prefix("&")
                    .set_prefix_color("green"),
            )
            .expect("Can't add nick to group");

        Weechat::print(&format!("Nick name getting test: {}", emma.name()));

        for nick_number in 0..n {
            let name = &format!("nick_{}", nick_number);
            let nick = NickSettings::new(name);
            let _ = buffer.add_nick(nick);
        }

        buffer.print(&format!(
            "Elapsed time for {} nick additions: {}.{}s.",
            n,
            now.elapsed().as_secs(),
            now.elapsed().subsec_millis()
        ));

        let sample_command = CommandSettings::new("rustcommand");

        let command =
            Command::new(sample_command, SamplePlugin::rust_command_cb);

        let mut config = Config::new_with_callback(
            "rust_sample",
            |_weechat: &Weechat, _config: &Conf| {
                Weechat::print("Reloaded config");
            },
        )
        .expect("Can't create new config");

        {
            let section_info = ConfigSectionSettings::new("sample_section");

            let mut section = config
                .new_section(section_info)
                .expect("Can't create section");

            let option_settings = BooleanOptionSettings::new("test_option")
                .default_value(false)
                .set_change_callback(SamplePlugin::option_change_cb);

            section
                .new_boolean_option(option_settings)
                .expect("Can't create option");
        }
        let item = BarItem::new(
            "buffer_plugin",
            |_weechat: &Weechat, _buffer: &Buffer| "rust/sample".to_owned(),
        );

        let signal_hook = SignalHook::new(
            "buffer_switch",
            |_weechat: &Weechat,
             _signal_name: &str,
             data: Option<SignalData>| {
                if let Some(data) = data {
                    if let SignalData::Buffer(buffer) = data {
                        buffer.print("Switched buffer")
                    }
                }

                ReturnCode::Ok
            },
        );

        Ok(SamplePlugin {
            _rust_hook: command.unwrap(),
            _rust_config: config,
            _item: item.unwrap(),
            _signal: signal_hook.unwrap(),
        })
    }
}

impl Drop for SamplePlugin {
    fn drop(&mut self) {
        Weechat::print("Bye rust");
    }
}

plugin!(
    SamplePlugin,
    name: "rust_sample",
    author: "Damir Jelić <poljar@termina.org.uk>",
    description: "",
    version: "0.1.0",
    license: "MIT"
);
