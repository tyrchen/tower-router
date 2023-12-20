use axum_core::{
    body::Body,
    extract::Request,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use http::{
    header::{self, CONTENT_LENGTH},
    HeaderMap, HeaderValue,
};
use http_body::Body as HttpBody;
use pin_project_lite::pin_project;
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower::{
    util::{BoxCloneService, MapErrLayer, MapRequestLayer, MapResponseLayer, Oneshot},
    ServiceExt,
};
use tower_layer::Layer;
use tower_service::Service;

/// How routes are stored inside a [`Router`](super::Router).
///
/// You normally shouldn't need to care about this type. It's used in
/// [`Router::layer`](super::Router::layer).
pub struct Route<Req, Res, E = Infallible>(BoxCloneService<Req, Res, E>);

impl<Req, Res, E> Route<Req, Res, E> {
    pub(crate) fn new<T>(svc: T) -> Self
    where
        T: Service<Request, Error = E> + Clone + Send + 'static,
        T::Response: IntoResponse + 'static,
        T::Future: Send + 'static,
    {
        Self(BoxCloneService::new(
            svc.map_response(IntoResponse::into_response),
        ))
    }

    pub(crate) fn oneshot_inner(&mut self, req: Req) -> Oneshot<BoxCloneService<Req, Res, E>, Req> {
        self.0.clone().oneshot(req)
    }

    pub(crate) fn layer<L, NewError>(self, layer: L) -> Route<Req, Res, NewError>
    where
        L: Layer<Route<Req, Res, E>> + Clone + Send + 'static,
        L::Service: Service<Req> + Clone + Send + 'static,
        <L::Service as Service<Req>>::Response: IntoResponse + 'static,
        <L::Service as Service<Req>>::Error: Into<NewError> + 'static,
        <L::Service as Service<Req>>::Future: Send + 'static,
        NewError: 'static,
    {
        let layer = (
            MapRequestLayer::new(|req: Req| req.map(Body::new)),
            MapErrLayer::new(Into::into),
            MapResponseLayer::new(IntoResponse::into_response),
            layer,
        );

        Route::new(layer.layer(self))
    }
}

impl<Req, Res, E> Clone for Route<Req, Res, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Req, Res, E> fmt::Debug for Route<Req, Res, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Route").finish()
    }
}

impl<Req, Res, E> Service<Req> for Route<Req, Res, E> {
    type Response = Req;
    type Error = E;
    type Future = RouteFuture<Req, Res, E>;

    #[inline]
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: Req) -> Self::Future {
        RouteFuture::from_future(self.oneshot_inner(req))
    }
}

pin_project! {
    /// Response future for [`Route`].
    pub struct RouteFuture<Req, Res, E> {
        #[pin]
        kind: RouteFutureKind<Req, Res, E>,
        strip_body: bool,
        allow_header: Option<Bytes>,
    }
}

pin_project! {
    #[project = RouteFutureKindProj]
    enum RouteFutureKind<Req, Res, E> {
        Future {
            #[pin]
            future: Oneshot<
                BoxCloneService<Req, Res, E>,
                Req,
            >,
        },
        Response {
            response: Option<Res>,
        }
    }
}

impl<Req, Res, E> RouteFuture<Req, Res, E> {
    pub(crate) fn from_future(future: Oneshot<BoxCloneService<Req, Res, E>, Req>) -> Self {
        Self {
            kind: RouteFutureKind::Future { future },
            strip_body: false,
            allow_header: None,
        }
    }

    pub(crate) fn strip_body(mut self, strip_body: bool) -> Self {
        self.strip_body = strip_body;
        self
    }

    pub(crate) fn allow_header(mut self, allow_header: Bytes) -> Self {
        self.allow_header = Some(allow_header);
        self
    }
}

impl<Req, Res, E> Future for RouteFuture<Req, Res, E> {
    type Output = Result<Res, E>;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let mut res = match this.kind.project() {
            RouteFutureKindProj::Future { future } => match future.poll(cx) {
                Poll::Ready(Ok(res)) => res,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                Poll::Pending => return Poll::Pending,
            },
            RouteFutureKindProj::Response { response } => {
                response.take().expect("future polled after completion")
            }
        };

        set_allow_header(res.headers_mut(), this.allow_header);

        // make sure to set content-length before removing the body
        set_content_length(res.size_hint(), res.headers_mut());

        let res = if *this.strip_body {
            res.map(|_| Body::empty())
        } else {
            res
        };

        Poll::Ready(Ok(res))
    }
}

fn set_allow_header(headers: &mut HeaderMap, allow_header: &mut Option<Bytes>) {
    match allow_header.take() {
        Some(allow_header) if !headers.contains_key(header::ALLOW) => {
            headers.insert(
                header::ALLOW,
                HeaderValue::from_maybe_shared(allow_header).expect("invalid `Allow` header"),
            );
        }
        _ => {}
    }
}

fn set_content_length(size_hint: http_body::SizeHint, headers: &mut HeaderMap) {
    if headers.contains_key(CONTENT_LENGTH) {
        return;
    }

    if let Some(size) = size_hint.exact() {
        let header_value = if size == 0 {
            #[allow(clippy::declare_interior_mutable_const)]
            const ZERO: HeaderValue = HeaderValue::from_static("0");

            ZERO
        } else {
            let mut buffer = itoa::Buffer::new();
            HeaderValue::from_str(buffer.format(size)).unwrap()
        };

        headers.insert(CONTENT_LENGTH, header_value);
    }
}

pin_project! {
    /// A [`RouteFuture`] that always yields a [`Response`].
    pub struct InfallibleRouteFuture<Req, Res> {
        #[pin]
        future: RouteFuture<Req, Res, Infallible>,
    }
}

impl<Req, Res> InfallibleRouteFuture<Req, Res> {
    pub(crate) fn new(future: RouteFuture<Req, Res, Infallible>) -> Self {
        Self { future }
    }
}

impl<Req, Res> Future for InfallibleRouteFuture<Req, Res> {
    type Output = Response;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match futures_util::ready!(self.project().future.poll(cx)) {
            Ok(response) => Poll::Ready(response),
            Err(err) => match err {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traits() {
        use crate::test_helpers::*;
        assert_send::<Route<()>>();
    }
}
