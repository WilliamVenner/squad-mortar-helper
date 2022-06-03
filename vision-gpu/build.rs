use std::{path::Path, process::Command};

fn cuda() {
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed=cuda/cuda.cu");
	println!("cargo:rerun-if-changed=cuda/dilate.h");
	println!("cargo:rerun-if-changed=cuda/dilate.cpp");
	println!("cargo:rerun-if-changed=../vision-common/src/consts/consts.cu");
	println!("cargo:rerun-if-env-changed=CUDA_LIBRARY_PATH");

	cc::Build::new()
		.cuda(true)
		.cargo_metadata(true)
		.flag("-lnppc")
		.flag("-lnppim")
		.include("cuda")
		.file("cuda/dilate.cpp")
		.compile("gpu_dilate");

	println!("cargo:rustc-link-lib=static=nppim");
	println!("cargo:rustc-link-search=native={}", std::env::var("CUDA_LIBRARY_PATH").expect("Expected CUDA_LIBRARY_PATH to be set"));

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

		if which::which("nvcc").is_err() {
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

fn main() {
	println!("cargo:rerun-if-env-changed=PATH");
	println!("cargo:rerun-if-env-changed=PATHEXT");

	cuda();
}
