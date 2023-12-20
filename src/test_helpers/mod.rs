#![allow(clippy::disallowed_names)]

use axum::serve;
use axum_core::{extract::Request, response::Response};
mod test_client;
pub(crate) use self::test_client::*;

pub(crate) mod tracing_helpers;

pub(crate) fn assert_send<T: Send>() {}
pub(crate) fn assert_sync<T: Sync>() {}

pub(crate) struct NotSendSync(*const ());
