// We want to be able to format strings to display in the user interface.
// The problem with this is that if we're formatting strings every frame, we're going to be abusing the heap and doing a lot of work every frame that should really be avoided.
// Therefore, we want to be able to keep around allocated memory for re-use.
// For this purpose, I have chosen an arena allocator that can be used to allocate memory for formatted strings.

macro_rules! ui_format {
	($ui_state:ident, $($arg:tt)*) => {{
		bumpalo::format!(in &$ui_state.ui_fmt_alloc, $($arg)*)
	}};
}
pub(super) use ui_format;
