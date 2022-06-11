use super::*;
use std::{
	collections::LinkedList,
	fs::{File, OpenOptions},
	io::{Seek, SeekFrom, Write as IoWrite},
};

#[derive(Clone)]
pub struct Log {
	pub level: log::Level,
	pub text: Box<str>,
	pub count: u16,
}
impl Eq for Log {}
impl PartialEq for Log {
	#[inline]
	fn eq(&self, other: &Self) -> bool {
		self.level == other.level && self.text == other.text
	}
}

static LOG_STATE: DeferCell<crossbeam::Sender<Log>> = DeferCell::defer();

pub struct LogState {
	pub window_open: bool,
	logs: LinkedList<Log>,
	rx: crossbeam::Receiver<Log>,
}
impl LogState {
	fn new() -> Self {
		let (tx, rx) = crossbeam::unbounded();
		unsafe { LOG_STATE.set(tx) };
		LogState {
			window_open: false,
			logs: Default::default(),
			rx,
		}
	}

	pub fn digest(&mut self) {
		while let Ok(log) = self.rx.try_recv() {
			if let Some(prev) = self.logs.back_mut() {
				if *prev == log {
					prev.count = prev.count.saturating_add(1);
					continue;
				}
			}

			if log.level == log::Level::Error {
				self.window_open = true;
			}

			self.logs.push_back(log);
		}
	}
}

struct SmhLoggerFileInner {
	file: File,
	last_log: Option<Log>,
}
struct SmhLoggerFile(Mutex<SmhLoggerFileInner>);
impl SmhLoggerFile {
	fn new() -> Result<Self, SmhLogger> {
		match OpenOptions::new()
			.create(true)
			.write(true)
			.truncate(false)
			.open(std::env::temp_dir().join("smh.log"))
			.and_then(|mut file| {
				file.seek(SeekFrom::End(0))?;
				Ok(file)
			}) {
			Ok(mut file) => {
				write!(
					file,
					"\n============ SMH LOG {} ============",
					std::time::SystemTime::now()
						.duration_since(std::time::SystemTime::UNIX_EPOCH)
						.map(|d| d.as_secs())
						.unwrap_or(0)
				)
				.ok();

				file.flush().ok();

				Ok(SmhLoggerFile(Mutex::new(SmhLoggerFileInner { file, last_log: None })))
			}
			Err(_) => Err(SmhLogger),
		}
	}
}
impl log::Log for SmhLoggerFile {
	#[inline]
	fn enabled(&self, metadata: &log::Metadata) -> bool {
		metadata.level() <= log::Level::Info
	}

	fn log(&self, record: &log::Record) {
		let text = format!("[{}] {}", record.module_path().unwrap_or("?"), record.args()).into_boxed_str();

		println!("[{}] {text}", record.level());

		let log = Log {
			level: record.level(),
			text,
			count: 0,
		};

		let mut state = self.0.lock();

		#[allow(clippy::never_loop)]
		'new: loop {
			if let Some(ref mut last_log) = state.last_log {
				if *last_log == log {
					if let Some(count) = last_log.count.checked_add(1) {
						// Write the number of repeats
						last_log.count = count;
						if count != 1 {
							let n_len = (count as f32).log10() as i64 + 1; // calculate the number of digits in the count
							state.file.seek(SeekFrom::Current(-(n_len + " (x)".len() as i64))).ok(); // seek back to overwrite the previous count
						}
						write!(state.file, " (x{})", count + 1).ok();
						state.file.flush().ok();
					} else {
						// If we saturate at u16::MAX, don't keep writing to the file, just discard this log
					}
					break 'new;
				}
			}

			state.last_log = Some(log.clone());

			write!(state.file, "\n[{}] {}", record.level(), log.text).ok();
			state.file.flush().ok();

			break;
		}

		LOG_STATE.get().sus_unwrap().send(log).ok();
	}

	fn flush(&self) {}
}

struct SmhLogger;
impl log::Log for SmhLogger {
	#[inline]
	fn enabled(&self, metadata: &log::Metadata) -> bool {
		metadata.level() <= log::Level::Info
	}

	fn log(&self, record: &log::Record) {
		let text = format!("[{}] {}", record.module_path().unwrap_or("?"), record.args()).into_boxed_str();

		println!("[{}] {text}", record.level());

		LOG_STATE
			.get()
			.sus_unwrap()
			.send(Log {
				level: record.level(),
				text,
				count: 0,
			})
			.ok();
	}

	fn flush(&self) {}
}

pub fn init() -> LogState {
	log::set_max_level(log::LevelFilter::Info);
	log::set_logger(logger()).ok();
	LogState::new()
}

pub fn logger() -> &'static dyn log::Log {
	lazy_static! {
		static ref LOGGER: &'static dyn log::Log = Box::leak({
			let logger: Box<dyn log::Log> = if let Some("--dumplogs") = std::env::args().nth(1).as_deref() {
				match SmhLoggerFile::new().map(Box::new).map_err(Box::new) {
					Ok(logger) => logger,
					Err(logger) => logger,
				}
			} else {
				Box::new(SmhLogger)
			};
			logger
		});
	}
	*LOGGER
}

pub(super) fn render_window(state: &mut UiState, ui: &Ui) {
	state.logs.digest();

	if !state.logs.window_open {
		return;
	};

	let window = match imgui::Window::new("Logs")
		.size([400.0, 300.0], imgui::Condition::FirstUseEver)
		.opened(&mut state.logs.window_open)
		.begin(ui)
	{
		Some(window) => window,
		None => return,
	};

	if ui.button("Copy") && !state.logs.logs.is_empty() {
		let mut dump = String::new();
		for log in state.logs.logs.iter() {
			dump.push_str(match log.level {
				log::Level::Info => "[INFO] ",
				log::Level::Error => "[ERROR] ",
				log::Level::Warn => "[WARN] ",
				log::Level::Debug => "[DEBUG] ",
				log::Level::Trace => "[TRACE] ",
			});
			dump.push_str(&log.text);
			if log.count > 0 {
				dump.push_str(&ui_format!(state, " (x{})", log.count + 1));
			}
			dump.push('\n');
		}
		dump.pop();
		ui.set_clipboard_text(dump);
	}

	ui.same_line();

	if ui.button("Close") {
		state.logs.window_open = false;
	}

	for log in state.logs.logs.iter().rev() {
		let (prefix, color) = match log.level {
			log::Level::Info => ("[INFO]", [0.0, 0.58, 1.0, 1.0]),
			log::Level::Error => ("[ERROR]", [1.0, 0.0, 0.0, 1.0]),
			log::Level::Warn => ("[WARN]", [1.0, 0.58, 0.0, 1.0]),
			log::Level::Debug => ("[DEBUG]", [0.58, 0.0, 1.0, 1.0]),
			log::Level::Trace => ("[TRACE]", [1.0, 1.0, 1.0, 1.0]),
		};

		let color = ui.push_style_color(imgui::StyleColor::Text, color);
		ui.text_wrapped(prefix);
		color.end();

		ui.same_line();
		if log.count > 0 {
			ui.text_wrapped(&ui_format!(state, "{} (x{})", log.text, log.count + 1));
		} else {
			ui.text_wrapped(&log.text);
		}
	}

	window.end();
}
