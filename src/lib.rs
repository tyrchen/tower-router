#[macro_use]
mod macros;
mod boxed;
mod error_handling;
mod extract;
mod handler;
mod routing;
mod util;

pub use routing::*;

use http_body::Body as HttpBody;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
