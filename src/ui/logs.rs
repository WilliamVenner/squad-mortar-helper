use super::*;
use std::{fs::{File, OpenOptions}, collections::LinkedList, io::Write as IoWrite};

pub struct Log {
	pub level: log::Level,
	pub text: Box<str>
}

static LOG_STATE: DeferCell<crossbeam::Sender<Log>> = DeferCell::defer();

pub struct LogState {
	pub window_open: bool,
	logs: LinkedList<Log>,
	rx: crossbeam::Receiver<Log>
}
impl LogState {
	fn new() -> Self {
		let (tx, rx) = crossbeam::unbounded();
		unsafe { LOG_STATE.set(tx) };
		LogState {
			window_open: false,
			logs: Default::default(),
			rx
		}
	}

	pub fn digest(&mut self) {
		while let Ok(log) = self.rx.try_recv() {
			if log.level == log::Level::Error {
				self.window_open = true;
			}

			self.logs.push_back(log);
		}
	}
}

struct SMHLoggerFile(Mutex<File>);
impl SMHLoggerFile {
	fn new() -> Result<Self, SMHLogger> {
		match OpenOptions::new().append(true).create(true).open(std::env::temp_dir().join("smh.log")) {
			Ok(mut f) => {
				writeln!(f, "============ SMH LOG {} ============", std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)).ok();
				Ok(SMHLoggerFile(Mutex::new(f)))
			},
			Err(_) => Err(SMHLogger)
		}
	}
}
impl log::Log for SMHLoggerFile {
	#[inline]
	fn enabled(&self, metadata: &log::Metadata) -> bool {
		metadata.level() <= log::Level::Info
	}

	fn log(&self, record: &log::Record) {
		let text = format!("[{}] {}", record.module_path().unwrap_or("?"), record.args()).into_boxed_str();

		println!("[{}] {text}", record.level());

		writeln!(&mut *self.0.lock(), "[{}] {text}", record.level()).ok();

		LOG_STATE.get().sus_unwrap().send(Log { level: record.level(), text }).ok();
	}

	fn flush(&self) {}
}

struct SMHLogger;
impl log::Log for SMHLogger {
	#[inline]
	fn enabled(&self, metadata: &log::Metadata) -> bool {
		metadata.level() <= log::Level::Info
	}

	fn log(&self, record: &log::Record) {
		let text = format!("[{}] {}", record.module_path().unwrap_or("?"), record.args()).into_boxed_str();

		println!("[{}] {text}", record.level());

		LOG_STATE.get().sus_unwrap().send(Log { level: record.level(), text }).ok();
	}

	fn flush(&self) {}
}

pub fn init() -> LogState {
	log::set_max_level(log::LevelFilter::Info);

	let logger: Box<dyn log::Log> = if let Some("--dumplogs") = std::env::args().nth(1).as_deref() {
		match SMHLoggerFile::new().map(Box::new).map_err(Box::new) {
			Ok(logger) => logger,
			Err(logger) => logger
		}
	} else {
		Box::new(SMHLogger)
	};
	log::set_logger(Box::leak(logger)).expect("Failed to initialize logger");

	LogState::new()
}

pub(super) fn render_window(state: &mut UIState, ui: &Ui) {
	state.logs.digest();

	if !state.logs.window_open { return };

	let window = match imgui::Window::new("Logs").size([400.0, 300.0], imgui::Condition::FirstUseEver).opened(&mut state.logs.window_open).begin(ui) {
		Some(window) => window,
		None => return
	};

	if ui.button("Copy") && !state.logs.logs.is_empty() {
		let mut dump = String::new();
		for log in state.logs.logs.iter() {
			dump.push_str(match log.level {
				log::Level::Info => "[INFO] ",
				log::Level::Error => "[ERROR] ",
				log::Level::Warn => "[WARN] ",
				log::Level::Debug => "[DEBUG] ",
				log::Level::Trace => "[TRACE] "
			});
			dump.push_str(&log.text);
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
			log::Level::Trace => ("[TRACE]", [1.0, 1.0, 1.0, 1.0])
		};

		let color = ui.push_style_color(imgui::StyleColor::Text, color);
		ui.text_wrapped(prefix);
		color.end();

		ui.same_line();
		ui.text_wrapped(&log.text);
	}

	window.end();
}