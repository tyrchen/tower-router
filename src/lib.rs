#[macro_use]
mod macros;
mod boxed;
mod error_handling;
mod extract;
mod handler;
mod routing;
#[cfg(test)]
mod test_helpers;
mod util;

pub use routing::*;

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

use http_body::Body as HttpBody;

#[cfg(test)]
use axum_macros::__private_axum_test as test;
