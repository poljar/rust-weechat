// Copyright (C) 2008-2018 Sébastien Helleu <flashcode@flashtux.org>
// Copyright (C) 2020 Damir Jelić <poljar@termina.org.uk>
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::{borrow::Cow, cell::RefCell, rc::Rc};

use chrono::{offset::Utc, DateTime};
use itertools::sorted;

use weechat::{
    buffer::{Buffer, BufferBuilder, BufferCloseCallback, BufferHandle, BufferInputCallback},
    hooks::{Command, CommandCallback, CommandSettings},
    infolist::InfolistVariable,
    plugin, Args, Plugin, Prefix, Weechat,
};

#[allow(unused)]
struct Infolist {
    command: Command,
}

#[derive(Default, Clone)]
struct InnerInfolist {
    buffer: Rc<RefCell<Option<BufferHandle>>>,
}

impl InnerInfolist {
    fn set_title(&self, weechat: &Weechat, buffer: &Buffer) {
        let infolist = weechat
            .get_infolist("hook", Some("infolist"))
            .expect("Can't get the infolist list");

        let infolist_names: Vec<String> = infolist
            .filter_map(|item| {
                let name = item.get("infolist_name")?;
                if let InfolistVariable::String(n) = name {
                    Some(n.to_string())
                } else {
                    None
                }
            })
            .collect();

        buffer.set_title(&format!(
            "Infolist 0.1 | Infolists: {}",
            infolist_names.join(" ")
        ));
    }

    fn display_infolist(&self, weechat: &Weechat, buffer: &Buffer, args: &str) {
        let mut args = args.splitn(2, ' ');

        let infolist_name = args.next().unwrap_or_default();
        let infolist_args = args.next();

        let infolist = if let Ok(i) = weechat.get_infolist(infolist_name, infolist_args) {
            i
        } else {
            buffer.print(&format!(
                "{}Infolist {} not found",
                Weechat::prefix(Prefix::Error),
                infolist_name
            ));
            return;
        };

        buffer.clear();
        buffer.print_date_tags(
            0,
            &["no_filter"],
            &format!(
                "Infolist {} with arguments '{}':",
                infolist_name,
                infolist_args.unwrap_or_default()
            ),
        );

        let mut infolist = infolist.peekable();

        if infolist.peek().is_none() {
            buffer.print("");
            buffer.print_date_tags(0, &["no_filter"], "Empty infolist.");
        } else {
            for (count, item) in infolist.enumerate() {
                buffer.print("");

                let mut prefix = format!(
                    "{}[{}{}{}]{}\t",
                    Weechat::color("chat_delimiters"),
                    Weechat::color("chat_buffer"),
                    count,
                    Weechat::color("chat_delimiters"),
                    Weechat::color("reset"),
                );

                for (name, value) in sorted(&item) {
                    let (value_type, value) = match value {
                        InfolistVariable::Buffer(b) => (
                            "ptr",
                            format!(
                                "{}{:?}{}",
                                Weechat::color("green"),
                                b,
                                Weechat::color("chat")
                            ),
                        ),
                        InfolistVariable::Integer(i) => (
                            "int",
                            format!(
                                "{}{}{}",
                                Weechat::color("yellow"),
                                i.to_string(),
                                Weechat::color("chat")
                            ),
                        ),
                        InfolistVariable::String(s) => (
                            "str",
                            format!(
                                "'{}{}{}'",
                                Weechat::color("cyan"),
                                s,
                                Weechat::color("chat")
                            ),
                        ),
                        InfolistVariable::Time(t) => (
                            "tim",
                            format!(
                                "{}{}{}",
                                Weechat::color("lightblue"),
                                DateTime::<Utc>::from(t).format("%F %T"),
                                Weechat::color("chat")
                            ),
                        ),
                    };

                    buffer.print_date_tags(
                        0,
                        &["no_filter"],
                        &format!(
                            "{}{:.<30} {}{}{} {}",
                            prefix,
                            name,
                            Weechat::color("brown"),
                            value_type,
                            Weechat::color("chat"),
                            value
                        ),
                    );

                    prefix = "".to_string();
                }
            }
        }
    }
}

impl BufferInputCallback for InnerInfolist {
    fn callback(&mut self, weechat: &Weechat, buffer: &Buffer, input: Cow<str>) -> Result<(), ()> {
        match input.as_ref() {
            "q" | "Q" => buffer.close(),
            _ => self.display_infolist(weechat, buffer, &input),
        }

        Ok(())
    }
}

impl BufferCloseCallback for InnerInfolist {
    fn callback(&mut self, _: &Weechat, _: &Buffer) -> Result<(), ()> {
        self.buffer.borrow_mut().take();
        Ok(())
    }
}

impl CommandCallback for InnerInfolist {
    fn callback(&mut self, weechat: &Weechat, _: &Buffer, mut arguments: Args) {
        if self.buffer.borrow().is_none() {
            let buffer = BufferBuilder::new("infolist")
                .input_callback(self.clone())
                .close_callback(self.clone())
                .build()
                .expect("Can't create infolist buffer");
            let b = buffer.upgrade().unwrap();

            b.set_localvar("no_log", "1");
            b.disable_time_for_each_line();
            self.set_title(weechat, &b);

            *self.buffer.borrow_mut() = Some(buffer);
        }

        let buffer_cell = self.buffer.borrow();
        let buffer_handle = buffer_cell.as_ref().expect("Buffer wasn't created");
        let buffer = buffer_handle
            .upgrade()
            .expect("Buffer was closed but the handle was still around");

        arguments.next();

        let args: String = arguments.collect::<Vec<String>>().join(" ");

        if !args.is_empty() {
            self.display_infolist(weechat, &buffer, &args);
        }

        buffer.switch_to();
    }
}

impl Plugin for Infolist {
    fn init(_: &Weechat, _args: Args) -> Result<Self, ()> {
        let command_settings = CommandSettings::new("infolist")
            .description("Display an infolist and it's items in a buffer")
            .add_argument("[infolist_name]")
            .add_argument("[arguments]")
            .arguments_description(
                " infolist: name of infolist\n\
                arguments: optional arguments for infolist\n\n\
                The command without any arguments will just open the infolist \
                buffer.\n\n\

                Inside the infolist buffer a name of an infolist can be entered \
                with optional arguments.\n\

                Enter 'q' to close the infolist buffer.\n\n\
                Examples:\n  \
                  Show information about the nick \"FlashCode\" in the channel \
                  \"#weechat\" on the server \"freenode\":\n    \
                    /infolist irc_nick freenode,#weechat,FlashCode",
            )
            .add_completion("%(infolists)");
        let command = Command::new(command_settings, InnerInfolist::default())?;

        Ok(Infolist { command })
    }
}

plugin!(
    Infolist,
    name: "infolist",
    author: "Damir Jelić <poljar@termina.org.uk>",
    description: "Display a infolist and it's items in a buffer",
    version: "0.1.0",
    license: "GPL3"
);
