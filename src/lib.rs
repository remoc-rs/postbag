#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/remoc-rs/postbag/main/.misc/postbag.png",
    html_favicon_url = "https://raw.githubusercontent.com/remoc-rs/postbag/main/.misc/postbag.png"
)]
#![doc = include_str!("../README.md")]

pub mod cfg;
pub mod compact;
mod de;
mod error;
pub mod fixint;
mod id;
mod ser;
mod varint;

const FALSE: u8 = 0;
const TRUE: u8 = 1;

const NONE: u8 = 0;
const SOME: u8 = 1;

const SPECIAL_LEN: usize = 125;
const UNKNOWN_LEN: usize = 0;

pub use de::{deserialize, deserialize_full, deserialize_slim, from_full_slice, from_slice, from_slim_slice};
pub use error::{Error, Result};
pub use ser::{serialize, serialize_full, serialize_slim, to_full_vec, to_slim_vec, to_vec};
