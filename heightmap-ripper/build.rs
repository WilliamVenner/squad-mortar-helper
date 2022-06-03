#[macro_use]
extern crate build_cfg;

use std::{ffi::OsStr, path::Path, process::Command};

#[build_cfg_main]
fn main() {
	let platform = if build_cfg!(all(windows, target_arch = "x86_64")) {
		"win-x64"
	} else if build_cfg!(all(target_os = "linux", target_arch = "x86_64")) {
		"linux-x64"
	} else {
		panic!("unsupported platform")
	};

	let configuration = if cfg!(debug_assertions) { "Debug" } else { "Release" };

	let output = Command::new("dotnet")
		.current_dir("SquadHeightmapRipper")
		.args(&[
			"publish",
			"-c",
			configuration,
			"-r",
			platform,
			"-p:PublishSingleFile=true",
			"-p:PublishTrimmed=true",
			"--self-contained",
			"true",
		])
		.output()
		.unwrap();

	if !output.status.success() {
		panic!(
			"Status: {:?}\n\n============ STDOUT ============\n{}\n\n============ STDERR ============\n{}",
			output.status,
			String::from_utf8_lossy(&output.stdout),
			String::from_utf8_lossy(&output.stderr)
		);
	}

	let out_dir = std::env::var("OUT_DIR").expect("Expected OUT_DIR env var to bet set");
	let out_dir = Path::new(&out_dir);
	let out_dir = if out_dir.join("deps").is_dir() {
		out_dir
	} else {
		out_dir.parent().unwrap().parent().unwrap().parent().unwrap()
	};

	let bin_dir = format!("SquadHeightmapRipper/SquadHeightmapRipper/bin/{configuration}/net6.0/{platform}/publish");
	let bin_dir = Path::new(&bin_dir);

	let copy_extensions = [OsStr::new("so"), OsStr::new("dll"), OsStr::new("exe")];
	for entry in std::fs::read_dir(bin_dir).unwrap() {
		let entry = entry.unwrap();
		if entry.file_type().unwrap().is_file()
			&& (entry.path().extension().is_none() || copy_extensions.contains(&entry.path().extension().unwrap()))
		{
			std::fs::write(
				out_dir.join(entry.path().file_name().unwrap()),
				std::fs::read(bin_dir.join(entry.path().file_name().unwrap())).unwrap(),
			)
			.unwrap();
		}
	}
}
