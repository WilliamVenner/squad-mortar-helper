fn main() {
	println!("cargo:rerun-if-changed=build.rs");
	println!("cargo:rerun-if-changed=src/consts/consts.toml");

	let toml_consts = toml_consts::from_str(include_str!("src/consts/consts.toml")).unwrap();

	let mut c = "#define SMH_CONSTS\n\n".to_string();
	let mut rs = String::new();

	toml_consts.serialize_cuda(&mut c).unwrap();
	toml_consts.serialize_rust(&mut rs).unwrap();

	std::fs::write("src/consts/consts.cu", c).unwrap();
	std::fs::write("src/consts/consts.rs", rs).unwrap();
}