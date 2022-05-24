pub trait ContainsIgnoreCase {
	fn contains_ignore_ascii_case(&self, needle: &str) -> bool;
}
impl<S: AsRef<str>> ContainsIgnoreCase for S {
    fn contains_ignore_ascii_case(&self, needle: &str) -> bool {
        let str = self.as_ref();

		if needle.is_empty() {
			return true;
		} else if needle.len() > str.len() {
			return false;
		}

		let mut needle_chars = needle.chars();
		for char in str.chars() {
			let needle_char = match needle_chars.next() {
				Some(needle_char) => needle_char,
				None => return true
			};
			if !char.eq_ignore_ascii_case(&needle_char) {
				needle_chars = needle.chars();
			}
		}

		false
    }
}