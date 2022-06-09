use std::path::Path;

pub fn rebuild_html() -> String {
	let mut index = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/www/index.html")).unwrap();

	const JS_SURGERY_START: &str = "<script type=\"text/javascript\" src=\"";
	const JS_SURGERY_END: &str = "\"></script>";

	const CSS_SURGERY_START: &str = "<link rel=\"stylesheet\" href=\"";
	const CSS_SURGERY_END: &str = "\">";

	let www = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/www"));

	while let Some(start) = index.find(JS_SURGERY_START) {
		let end = index[start..].find(JS_SURGERY_END).expect("expected close to <script> src attribute");
		let src = &index[start + JS_SURGERY_START.len()..start + end];
		let contents = std::fs::read_to_string(www.join(src)).unwrap();
		index = format!("{}<script type=\"text/javascript\">{contents}</script>{}", &index[..start], &index[start + end + JS_SURGERY_END.len()..]);
	}

	while let Some(start) = index.find(CSS_SURGERY_START) {
		let end = index[start..].find(CSS_SURGERY_END).expect("expected close to <link> stylesheet");
		let src = &index[start + CSS_SURGERY_START.len()..start + end];
		let contents = std::fs::read_to_string(www.join(src)).unwrap();
		index = format!("{}<style>{contents}</style>{}", &index[..start], &index[start + end + CSS_SURGERY_END.len()..]);
	}

	if cfg!(not(build_assertions)) {
		index.replace_range(index.find("<!--RELEASE-->").unwrap().."<!--RELEASE-->".len(), "<script type=\"text/javascript\">window.RELEASE = true;</script>");
	}

	index
}

#[allow(dead_code)]
fn main() {
	println!("cargo:rerun-if-changed=src/html.rs");
	println!("cargo:rerun-if-changed=www/index.html");
	println!("cargo:rerun-if-changed=www/map.js");
	println!("cargo:rerun-if-changed=www/ctl.js");
	println!("cargo:rerun-if-changed=www/ws.js");
	println!("cargo:rerun-if-changed=www/squadex.js");
	println!("cargo:rerun-if-changed=www/style.css");

	let out = std::env::var("OUT_DIR").unwrap();
	let index = rebuild_html();
	std::fs::write(Path::new(&out).join("web.html"), index).unwrap();
}