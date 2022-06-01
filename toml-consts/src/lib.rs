use std::{str::FromStr, collections::BTreeMap};

#[macro_use] extern crate serde;

macro_rules! define_types {
	{$(
		$enum:ident: {
			toml: $toml:literal,
			c: $c:literal,
			rust: $rust:ty,
			serde: $serde:ident
		}
	),*} => {
		#[derive(Clone, Copy, Debug)]
		pub enum CrossType {
			$($enum),*
		}
		impl<'de> serde::Deserialize<'de> for CrossType {
			fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
			where
				D: serde::Deserializer<'de>
			{
				let str: &str = serde::Deserialize::deserialize(deserializer)?;
				match str {
					$($toml => Ok(CrossType::$enum),)*
					_ => Err(serde::de::Error::custom(format!("unknown type: {}", str)))
				}
			}
		}
		impl CrossType {
			fn serialize_cuda<W: std::fmt::Write>(self, name: &str, fmt: &mut W, value: &toml::Value) -> std::fmt::Result {
				match self {
					$(CrossType::$enum => {
						if let Some(values) = value.as_array().cloned() {
							let len = values.len();

							write!(fmt, concat!("constexpr __device__ ", $c, " {}[{}] = {{"), name, len)?;

							let mut values = values.into_iter();

							values.by_ref().take(len - 1).try_for_each(|value| write!(fmt, "{}, ", match value.$serde() {
								Some(value) => value as $rust,
								None => panic!(concat!("expected ", $toml, " for {}"), name)
							}))?;

							writeln!(fmt, "{}}};", match values.next().unwrap().$serde() {
								Some(value) => value as $rust,
								None => panic!(concat!("expected ", $toml, " for {}"), name)
							})
						} else {
							let value = match value.$serde() {
								Some(value) => value as $rust,
								None => panic!(concat!("expected ", $toml, " for {}"), name)
							};
							writeln!(fmt, concat!("constexpr __device__ ", $c, " {} = {};"), name, value)
						}
					}),*
				}
			}

			fn serialize_rust<W: std::fmt::Write>(self, name: &str, fmt: &mut W, value: &toml::Value) -> std::fmt::Result {
				match self {
					$(CrossType::$enum => {
						if let Some(values) = value.as_array().cloned() {
							let len = values.len();

							write!(fmt, concat!("pub const {}: [", stringify!($rust), "; {}] = ["), name, len)?;

							let mut values = values.into_iter();

							values.by_ref().take(len - 1).try_for_each(|value| write!(fmt, "{}, ", match value.$serde() {
								Some(value) => value as $rust,
								None => panic!(concat!("expected ", $toml, " for {}"), name)
							}))?;

							writeln!(fmt, "{}];", match values.next().unwrap().$serde() {
								Some(value) => value as $rust,
								None => panic!(concat!("expected ", $toml, " for {}"), name)
							})
						} else {
							let value = match value.$serde() {
								Some(value) => value as $rust,
								None => panic!(concat!("expected ", $toml, " for {}"), name)
							};
							writeln!(fmt, concat!("pub const {}: ", stringify!($rust), " = {};"), name, value)
						}
					}),*
				}
			}
		}
	};
}

define_types! {
	Bool: {
		toml: "bool",
		c: "bool",
		rust: bool,
		serde: as_bool
	},
	Float: {
		toml: "f32",
		c: "float",
		rust: f32,
		serde: as_float
	},
	Double: {
		toml: "f64",
		c: "double",
		rust: f64,
		serde: as_float
	},
	String: {
		toml: "string",
		c: "char*",
		rust: &str,
		serde: as_str
	},
	I8: {
		toml: "i8",
		c: "int8_t",
		rust: i8,
		serde: as_integer
	},
	I16: {
		toml: "i16",
		c: "int16_t",
		rust: i16,
		serde: as_integer
	},
	I32: {
		toml: "i32",
		c: "int32_t",
		rust: i32,
		serde: as_integer
	},
	I64: {
		toml: "i64",
		c: "int64_t",
		rust: i64,
		serde: as_integer
	},
	U8: {
		toml: "u8",
		c: "uint8_t",
		rust: u8,
		serde: as_integer
	},
	U16: {
		toml: "u16",
		c: "uint16_t",
		rust: u16,
		serde: as_integer
	},
	U32: {
		toml: "u32",
		c: "uint32_t",
		rust: u32,
		serde: as_integer
	},
	U64: {
		toml: "u64",
		c: "uint64_t",
		rust: u64,
		serde: as_integer
	}
}

#[derive(Deserialize, Clone, Debug)]
struct TomlConst {
	#[serde(rename = "type")]
	cross_type: CrossType,
	value: toml::Value
}

#[derive(Clone, Debug)]
pub struct TomlConsts(BTreeMap<String, TomlConst>);
impl FromStr for TomlConsts {
	type Err = toml::de::Error;

	fn from_str(str: &str) -> Result<Self, Self::Err> {
		Ok(TomlConsts(toml::de::from_str(str)?))
	}
}
impl TomlConsts {
	pub fn serialize_cuda<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
		self.0.iter().try_for_each(|(name, toml_const)| {
			toml_const.cross_type.serialize_cuda(name, w, &toml_const.value)
		})
	}

	pub fn serialize_rust<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
		self.0.iter().try_for_each(|(name, toml_const)| {
			toml_const.cross_type.serialize_rust(name, w, &toml_const.value)
		})
	}
}

#[inline]
pub fn from_str(str: &str) -> Result<TomlConsts, <TomlConsts as FromStr>::Err> {
	TomlConsts::from_str(str)
}