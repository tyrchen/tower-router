//! Handler future types.

use axum_core::{extract::Request, response::Response};
use futures_util::future::Map;
use pin_project_lite::pin_project;
use std::{convert::Infallible, future::Future, pin::Pin, task::Context};
use tower::util::Oneshot;
use tower_service::Service;

type MapFuture<F> = Map<F, fn(Response) -> Result<Response, Infallible>>;
opaque_future! {
    /// The response future for [`IntoService`](super::IntoService).
    pub type IntoServiceFuture<F> = MapFuture<F>;
}

type MapInner<S> = Map<
    Oneshot<S, Request>,
    fn(Result<<S as Service<Request>>::Response, <S as Service<Request>>::Error>) -> Response,
>;
pin_project! {
    /// The response future for [`Layered`](super::Layered).
    pub struct LayeredFuture<S>
    where
        S: Service<Request>,
    {
        #[pin]
        inner: MapInner<S>,
    }
}

impl<S> LayeredFuture<S>
where
    S: Service<Request>,
{
    #[allow(clippy::type_complexity)]
    pub(super) fn new(
        inner: Map<Oneshot<S, Request>, fn(Result<S::Response, S::Error>) -> Response>,
    ) -> Self {
        Self { inner }
    }
}

impl<S> Future for LayeredFuture<S>
where
    S: Service<Request>,
{
    type Output = Response;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}
