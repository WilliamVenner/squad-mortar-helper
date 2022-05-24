#[macro_use] extern crate build_cfg;

use std::path::PathBuf;

#[build_cfg_main]
fn main() {
	println!("cargo:rerun-if-changed=src/lib.rs");
	println!("cargo:rerun-if-changed=src/ocr.cpp");
	println!("cargo:rerun-if-changed=src/ocr.hpp");
	println!("cargo:rerun-if-changed=build.rs");

	if build_cfg!(all(any(target_os = "linux", target_os = "windows"), any(target_arch = "x86_64", target_arch = "x86"), any(target_pointer_width = "64", target_pointer_width = "32"))) {
		let platform = if build_cfg!(all(target_os = "windows", target_pointer_width = "64")) {
			"windows-x64"
		} else if build_cfg!(all(target_os = "windows", target_pointer_width = "32")) {
			"windows-x86"
		} else if build_cfg!(all(target_os = "linux", target_pointer_width = "64")) {
			"linux-x64"
		} else if build_cfg!(all(target_os = "linux", target_pointer_width = "32")) {
			"linux-x86"
		} else {
			unreachable!()
		};

		let lib_name = if build_cfg!(target_os = "windows") {
			"tesseract51.lib"
		} else if build_cfg!(target_os = "linux") {
			"libtesseract.a"
		} else {
			unreachable!()
		};

		let link_name = if build_cfg!(target_os = "windows") {
			"tesseract51"
		} else if build_cfg!(target_os = "linux") {
			"tesseract"
		} else {
			unreachable!()
		};

		let link_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../lib/tesseract/");

		let path = PathBuf::from(format!("{link_dir}/{platform}/{lib_name}"));
		if path.is_file() {
			println!("cargo:rustc-link-search=native={link_dir}/{platform}");
			println!("cargo:rustc-link-lib=static={link_name}");
		} else {
			panic!("Tesseract library not found at {}", path.display());
		}
	} else {
		panic!("unsupported platform");
	}

	cc::Build::new()
		.flag_if_supported("-std=c++11")
		.cpp(true)
		.include("../lib/include")
		.file("src/ocr.cpp")
		.compile("smh_ocr");
}