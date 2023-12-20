//! Handler future types.

use axum_core::{extract::Request, response::Response};
use futures_util::future::Map;
use pin_project_lite::pin_project;
use std::{convert::Infallible, future::Future, pin::Pin, task::Context};
use tower::util::Oneshot;
use tower_service::Service;

type MapFuture<Res, F> = Map<F, fn(Res) -> Result<Res, Infallible>>;
opaque_future! {
    /// The response future for [`IntoService`](super::IntoService).
    pub type IntoServiceFuture<Res, F> = MapFuture<Res, F>;
}

type MapInner<Req, Res, S> = Map<
    Oneshot<S, Req>,
    fn(Result<<S as Service<Req>>::Response, <S as Service<Req>>::Error>) -> Res,
>;
pin_project! {
    /// The response future for [`Layered`](super::Layered).
    pub struct LayeredFuture<Req, Res, S>
    where
        S: Service<Req>,
    {
        #[pin]
        inner: MapInner<Req, Res, S>,
    }
}

impl<Req, Res, S> LayeredFuture<Req, Res, S>
where
    S: Service<Req>,
{
    #[allow(clippy::type_complexity)]
    pub(super) fn new(
        inner: Map<Oneshot<S, Req>, fn(Result<S::Response, S::Error>) -> Res>,
    ) -> Self {
        Self { inner }
    }
}

impl<Req, Res, S> Future for LayeredFuture<Req, Res, S>
where
    S: Service<Req>,
{
    type Output = Res;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> std::task::Poll<Self::Output> {
        self.project().inner.poll(cx)
    }
}
