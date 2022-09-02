mod buffer;

use clap::{App, Arg};
use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    str::FromStr,
};
use tokio::{
    runtime::{Builder, Runtime},
    sync::mpsc::{channel, Receiver, Sender},
};

use grep_regex::RegexMatcher;
use grep_searcher::{sinks::Lossy, Searcher};
use std::io;

use std::{cell::RefCell, rc::Rc, time::Instant};

use weechat::{infolist::InfolistVariable, Args, Plugin, Weechat};

use weechat::{
    buffer::{Buffer, BufferCloseCallback, BufferInputCallback},
    config::{BooleanOptionSettings, Config, ConfigOption, ConfigSectionSettings},
    hooks::{Command, CommandCallback, CommandSettings},
    plugin, Prefix,
};

use buffer::GrepBuffer;

type SearchResult = Result<Vec<String>, io::Error>;

struct Ripgrep {
    _config: Rc<RefCell<Config>>,
    _command: Command,
    _runtime: Rc<RefCell<Option<Runtime>>>,
}

#[derive(Clone)]
pub struct RipgrepCommand {
    config: Rc<RefCell<Config>>,
    buffer: Rc<RefCell<Option<GrepBuffer>>>,
    runtime: Rc<RefCell<Option<Runtime>>>,
    last_search_file: Rc<RefCell<Option<PathBuf>>>,
}

impl RipgrepCommand {
    /// Wait for the result from the search task and print it out.
    ///
    /// This runs on the main Weechat thread.
    // TODO we could spawn this task from the search task running on the Tokio
    // runtime using Weechat::spawn_from_thread(). This would get rid of the
    // receiver.
    async fn receive_result(
        &self,
        file: PathBuf,
        search_term: String,
        mut receiver: Receiver<SearchResult>,
    ) {
        let start = Instant::now();
        let result = receiver.recv().await;

        let result = if let Some(result) = result {
            match result {
                Ok(r) => r,
                Err(e) => {
                    Weechat::print(&format!("Error searching: {:?}", e));
                    return;
                }
            }
        } else {
            Weechat::print("Error searching: empty result");
            return;
        };

        self.last_search_file.borrow_mut().replace(file.clone());

        let buffer = &self.buffer;
        let buffer_exists = buffer.borrow().is_some();

        if !buffer_exists {
            let buffer_handle = GrepBuffer::new(&self);
            *buffer.borrow_mut() = Some(buffer_handle);
        }

        let buffer_borrow = buffer.borrow();
        let buffer = buffer_borrow.as_ref().expect("Buffer wasn't created");

        let end = Instant::now();

        buffer.print_result(&search_term, &file, end - start, &result);

        let config = self.config.borrow();
        let section = config.search_section("main").unwrap();
        let go_to_buffer = section.search_option("go_to_buffer").unwrap();

        let go_to_buffer = match go_to_buffer {
            ConfigOption::Boolean(opt) => opt,
            _ => panic!("Invalid option type"),
        };

        if go_to_buffer.value() {
            buffer.switch_to();
        }
    }

    /// Helper to spawn a result receiving coroutine, so we avoid using async
    /// coroutines which are not stable.
    async fn receive_result_helper(
        command: RipgrepCommand,
        file: PathBuf,
        search_term: String,
        rx: Receiver<SearchResult>,
    ) {
        command.receive_result(file, search_term, rx).await
    }

    /// Get the logger file for the given buffer from the infolist.
    fn file_from_infolist(&self, weechat: &Weechat, buffer: &Buffer) -> Option<String> {
        let infolist = weechat.get_infolist("logger_buffer", None).ok()?;

        for item in infolist {
            let info_buffer = if let Some(b) = item.get("buffer") {
                b
            } else {
                continue;
            };

            if let InfolistVariable::Buffer(info_buffer) = info_buffer {
                if buffer == &info_buffer {
                    let path = item.get("log_filename")?;

                    if let InfolistVariable::String(path) = path {
                        return Some(path.to_string());
                    }
                }
            }
        }

        None
    }

    /// Guess the log file from the buffer name.
    fn file_from_name(&self, full_name: &str) -> PathBuf {
        let weechat_home = Weechat::info_get("weechat_dir", "").expect("Can't find Weechat home");
        let mut file = Path::new(&weechat_home).join("logs");
        let mut full_name = full_name.to_owned();
        full_name.push_str(".weechatlog");
        file.push(full_name);
        file
    }

    /// Get the log file for a buffer.
    fn get_file_by_buffer(&self, weechat: &Weechat, buffer: &Buffer) -> Option<PathBuf> {
        let path = self.file_from_infolist(weechat, buffer);

        if let Some(path) = path {
            PathBuf::from_str(&path)
        } else {
            let full_name = buffer.full_name().to_lowercase();
            Ok(self.file_from_name(&full_name))
        }
        .ok()
    }

    /// Search the given file using the given regex matcher.
    ///
    /// This runs on the Tokio executor in a separate thread, returns the
    /// searchresult through a mpsc channel to the Weechat thread.
    async fn search(file: PathBuf, matcher: RegexMatcher, sender: Sender<SearchResult>) {
        let mut matches: Vec<String> = vec![];

        let sink = Lossy(|_, line| {
            matches.push(line.to_string());
            Ok(true)
        });

        match Searcher::new().search_path(&matcher, file, sink) {
            Ok(_) => sender.send(Ok(matches)),
            Err(e) => sender.send(Err(e)),
        }
        .await
        .unwrap_or(());
    }

    /// Start a search.
    ///
    /// This spawns a Tokio task to search the given file and a Weechat task to
    /// wait for the result.
    fn start_search(&self, term: &str, file: &Path) {
        let matcher = match RegexMatcher::new(term) {
            Ok(m) => m,
            Err(e) => {
                Weechat::print(&format!(
                    "{} Invalid regular expression {:?}",
                    Weechat::prefix(Prefix::Error),
                    e
                ));
                return;
            }
        };

        let (tx, rx) = channel(1);

        self.runtime
            .borrow_mut()
            .as_ref()
            .unwrap()
            .spawn(RipgrepCommand::search(file.to_owned(), matcher, tx));
        Weechat::spawn(RipgrepCommand::receive_result_helper(
            self.clone(),
            file.to_owned(),
            term.to_string(),
            rx,
        ))
        .detach();
    }
}

impl BufferInputCallback for RipgrepCommand {
    fn callback(&mut self, _weechat: &Weechat, buffer: &Buffer, input: Cow<str>) -> Result<(), ()> {
        if input == "q" || input == "Q" {
            buffer.close();
            return Ok(());
        }

        let file = self.last_search_file.borrow();

        let file = match &*file {
            Some(f) => f,
            None => return Err(()),
        };

        self.start_search(&input, file);

        Ok(())
    }
}

impl BufferCloseCallback for RipgrepCommand {
    fn callback(&mut self, _weechat: &Weechat, _buffer: &Buffer) -> Result<(), ()> {
        self.buffer.borrow_mut().take();
        Ok(())
    }
}

impl CommandCallback for RipgrepCommand {
    fn callback(&mut self, weechat: &Weechat, buffer: &Buffer, arguments: Args) {
        let parsed_args = App::new("rg")
            .arg(
                Arg::with_name("pattern")
                    .index(1)
                    .value_name("PATTERN")
                    .help("A regular expression used for searching.")
                    .multiple(true),
            )
            .get_matches_from_safe(arguments);

        let parsed_args = match parsed_args {
            Ok(a) => a,
            Err(e) => {
                Weechat::print(&format!("Error parsing grep args {}", e));
                return;
            }
        };

        let file = self.get_file_by_buffer(weechat, buffer);

        let file = match file {
            Some(f) => f,
            None => return,
        };

        let pattern = match parsed_args.value_of("pattern") {
            Some(p) => p,
            None => {
                Weechat::print("Invalid pattern");
                return;
            }
        };

        self.start_search(pattern, &file);
    }
}

impl Plugin for Ripgrep {
    fn init(_: &Weechat, _args: Args) -> Result<Self, ()> {
        let mut config = Config::new("ripgrep")?;

        {
            let section_settings = ConfigSectionSettings::new("main");
            let mut section = config
                .new_section(section_settings)
                .expect("Can't create main config section");

            let option_settings = BooleanOptionSettings::new("go_to_buffer")
                .description("Automatically go to grep buffer when search is over.")
                .default_value(true);

            section
                .new_boolean_option(option_settings)
                .expect("Can't create boolean option");
        }

        let config = Rc::new(RefCell::new(config));

        let command_info = CommandSettings::new("rg");

        let runtime = Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("ripgrep-searcher")
            .build()
            .expect("Can't create the Tokio runtime");

        let runtime = Rc::new(RefCell::new(Some(runtime)));

        let command = Command::new(
            command_info,
            RipgrepCommand {
                runtime: runtime.clone(),
                buffer: Rc::new(RefCell::new(None)),
                config: config.clone(),
                last_search_file: Rc::new(RefCell::new(None)),
            },
        );

        Ok(Ripgrep {
            _config: config,
            _command: command?,
            _runtime: runtime,
        })
    }
}

plugin!(
    Ripgrep,
    name: "ripgrep",
    author: "Damir Jelic <poljar@termina.org.uk>",
    description: "Search in buffers and logs using ripgrep",
    version: "0.1.0",
    license: "ISC"
);
