#[macro_use] extern crate build_cfg;

#[build_cfg_main]
fn main() {
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed=mortar.ico");

	#[cfg(target_os = "windows")] {
		winres::WindowsResource::new()
			.set_icon("mortar.ico")
			.compile().expect("Failed to add icon to executable");
	}
}