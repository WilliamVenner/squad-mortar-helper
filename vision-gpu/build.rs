#[macro_use]
extern crate build_cfg;

use std::{path::Path, process::Command};

fn cuda() {
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed=cuda/cuda.cu");
	println!("cargo:rerun-if-changed=cuda/dilate.h");
	println!("cargo:rerun-if-changed=cuda/dilate.cpp");
	println!("cargo:rerun-if-changed=../vision-common/src/consts/consts.cu");
	println!("cargo:rerun-if-env-changed=CUDA_LIBRARY_PATH");

	let nvcc_path = which::which("nvcc").ok();

	if build_cfg!(target_os = "windows") {
		// nvidia why???

		let mut build = cc::Build::new();
		build
			.cuda(true)
			.cargo_metadata(true)
			.flag("-lnppc")
			.flag("-lnppim")
			.include("cuda")
			.file("cuda/dilate.cpp");

		let nvcc_path = nvcc_path.clone().unwrap_or_else(|| build.get_compiler().path().to_path_buf());
		if !nvcc_path.is_file() {
			panic!("nvcc not found - needed to copy nppim dll");
		}

		build.compile("gpu_dilate");

		println!("cargo:rustc-link-lib=nppim");

		let dll = nvcc_path
			.parent()
			.unwrap()
			.read_dir()
			.ok()
			.and_then(|dir| {
				dir.filter_map(|entry| {
					entry
						.ok()
						.and_then(|entry| if entry.file_type().ok()?.is_file() { Some(entry.path()) } else { None })
				})
				.find(|path| {
					path.extension() == Some(std::ffi::OsStr::new("dll"))
						&& path
							.file_name()
							.and_then(|name| name.to_str())
							.map(|name| name.starts_with("nppim64"))
							.unwrap_or(false)
				})
			})
			.expect("Failed to find nppim dll");

		let mut out = Path::new(&std::env::var("OUT_DIR").unwrap()).to_path_buf();
		if !out.join("deps").is_dir() {
			out.pop();
			out.pop();
			out.pop();
		}
		out.push(dll.file_name().unwrap());

		std::fs::copy(dll, out).expect("Failed to copy nppim dll"); // ughhhh
	} else {
		cc::Build::new()
			.cuda(true)
			.cargo_metadata(true)
			.flag("-lnppc_static")
			.flag("-lnppim_static")
			.include("cuda")
			.file("cuda/dilate.cpp")
			.compile("gpu_dilate");

		println!("cargo:rustc-link-lib=static=nppim_static");
	}

	if cfg!(feature = "gpu-ptx-vendored") {
		return;
	}

	let ccbin = cc::Build::new().get_compiler();
	let ccbin = ccbin.path().parent().unwrap().join("cl.exe"); // TODO support linux here

	let path: &Path = "cuda/cuda.cu".as_ref();

	let compute_capabilities: &'static [Option<&'static str>] = if cfg!(any(debug_assertions, feature = "force-gpu-debug")) {
		&[None]
	} else {
		&[None, Some("86"), Some("75"), Some("61"), Some("52"), Some("35")]
	};

	for compute_capability in compute_capabilities {
		let mut ptx_path = path.with_extension("ptx");

		if cfg!(any(debug_assertions, feature = "force-gpu-debug")) {
			ptx_path.set_file_name("cuda_dbg.ptx");
		} else if let Some(compute_capability) = compute_capability {
			ptx_path.set_file_name(&format!("cuda_release_{compute_capability}.ptx"));
		} else {
			ptx_path.set_file_name("cuda_release.ptx");
		}

		if nvcc_path.is_none() {
			if ptx_path.is_file() {
				// We might be doing continuous development, so don't fail if the ptx file is missing
				return;
			} else {
				panic!("{} is missing, and nvcc is not installed on this system", ptx_path.display());
			}
		}

		let mut nvcc = Command::new("nvcc");
		nvcc.args(&["--pre-include", "../../vision-common/src/consts/consts.cu"])
			.arg("-ccbin")
			.arg(&ccbin)
			.arg("--ptx")
			.arg(&path)
			.arg("-o")
			.arg(&ptx_path);

		if cfg!(any(debug_assertions, feature = "force-gpu-debug")) {
			nvcc.args(&["--device-debug"]);
		}
		if cfg!(any(not(debug_assertions), feature = "force-gpu-ptx-optimised")) {
			nvcc.args(&["-O", "3"]).args(&["-D", "NDEBUG"]);
		}
		if let Some(compute_capability) = compute_capability {
			nvcc.arg("-gencode")
				.arg(&format!("arch=compute_{cc},code=sm_{cc}", cc = compute_capability));
		}

		let output = nvcc.output().expect("Failed to run nvcc");
		if !output.status.success() {
			panic!(
				"nvcc error {:?} for {}\n\n===== stdout =====\n{}\n\n===== stderr =====\n{}",
				output.status.code(),
				path.display(),
				String::from_utf8_lossy(&output.stdout),
				String::from_utf8_lossy(&output.stderr)
			);
		}

		if !ptx_path.is_file() {
			panic!("compilation to {} succeeded, but the file is missing?", ptx_path.display());
		}
	}
}

#[build_cfg_main]
fn main() {
	println!("cargo:rerun-if-env-changed=PATH");
	println!("cargo:rerun-if-env-changed=PATHEXT");

	cuda();
}
