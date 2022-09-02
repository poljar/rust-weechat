use crate::RipgrepCommand;
use std::{path::Path, time::Duration};
use weechat::{
    buffer::{BufferBuilder, BufferHandle},
    Weechat,
};

pub struct GrepBuffer {
    buffer: BufferHandle,
}

impl GrepBuffer {
    pub fn new(command: &RipgrepCommand) -> GrepBuffer {
        let buffer_handle = BufferBuilder::new("ripgrep")
            .close_callback(command.clone())
            .input_callback(command.clone())
            .build()
            .expect("Can't create ripgrep buffer");

        let buffer = buffer_handle.upgrade().unwrap();

        buffer.disable_nicklist();
        buffer.disable_time_for_each_line();
        buffer.disable_log();
        buffer.set_title("ripgrep output buffer");

        GrepBuffer {
            buffer: buffer_handle,
        }
    }

    fn split_line(line: &str) -> (&str, &str, String) {
        let tab_count = line.matches('\t').count();

        let (date, nick, msg) = if tab_count >= 2 {
            let vec: Vec<&str> = line.splitn(3, '\t').collect();
            (vec[0], vec[1], vec[2])
        } else {
            ("", "", line)
        };

        let msg = msg.trim().replace("\t", "    ");
        (date.trim(), nick.trim(), msg)
    }

    fn format_line(&self, line: &str) -> String {
        let (date, nick, msg) = GrepBuffer::split_line(line);
        let nick = self.colorize_nick(nick);

        format!(
            "{date_color}{date}{reset} {nick} {msg}",
            date_color = Weechat::color("brown"),
            date = date,
            reset = Weechat::color("reset"),
            nick = nick,
            msg = msg
        )
    }

    fn print(&self, line: &str) {
        self.buffer
            .upgrade()
            .unwrap()
            .print(&self.format_line(line));
    }

    fn colorize_nick(&self, nick: &str) -> String {
        if nick.is_empty() {
            return "".to_owned();
        }

        // TODO colorize the nick prefix and suffix
        // TODO handle the extra nick prefix and suffix settings

        let (prefix, nick) = {
            let first_char = nick.chars().next();
            match first_char {
                Some('&') | Some('@') | Some('!') | Some('+') | Some('%') => {
                    (first_char, &nick[1..])
                }
                Some(_) => (None, nick),
                None => (None, nick),
            }
        };

        let prefix = match prefix {
            Some(p) => p.to_string(),
            None => "".to_owned(),
        };

        let nick_color = Weechat::info_get("nick_color_name", nick).unwrap();

        format!(
            "{}{}{}{}",
            prefix,
            Weechat::color(&nick_color),
            nick,
            Weechat::color("reset")
        )
    }

    fn print_status(&self, line: &str) {
        self.buffer.upgrade().unwrap().print(&format!(
            "{}[{}grep{}]{}\t{}",
            Weechat::color("chat_delimiters"),
            Weechat::color("chat_nick"),
            Weechat::color("chat_delimiters"),
            Weechat::color("reset"),
            line
        ))
    }

    fn set_title(&self, title: &str) {
        self.buffer.upgrade().unwrap().set_title(title);
    }

    pub fn switch_to(&self) {
        self.buffer.upgrade().unwrap().switch_to();
    }

    pub fn print_result(
        &self,
        search_term: &str,
        file: &Path,
        duration: Duration,
        result: &[String],
    ) {
        self.print_status(&format!(
            "{summary_color}Search for {emph_color}{pattern}{summary_color} \
             in {emph_color}{file:?}{color_reset}.",
            summary_color = Weechat::color("cyan"),
            emph_color = Weechat::color("lightcyan"),
            color_reset = Weechat::color("reset"),
            pattern = search_term,
            file = file
        ));

        let max_lines = std::cmp::min(result.len(), 4000);

        for line in &result[..max_lines] {
            self.print(&line);
        }

        self.print_status(&format!(
            "{summary_color}{matches} matches \"{emph_color}{search_term}\
            {summary_color}\" in {emph_color}{file:?}{color_reset}.",
            summary_color = Weechat::color("cyan"),
            emph_color = Weechat::color("lightcyan"),
            matches = result.len(),
            search_term = search_term,
            file = file,
            color_reset = Weechat::color("reset")
        ));

        let title = format!(
            "'q': close buffer | Search in {color_title}{file:?}{color_reset} \
            {matches} matches | pattern \"{color_title}{search_term}{color_reset}\" \
            | {duration:?}",
            color_title = Weechat::color("yellow"),
            file = file,
            color_reset = Weechat::color("reset"),
            matches = result.len(),
            search_term = search_term,
            duration = duration,
        );

        self.set_title(&title);
    }
}
