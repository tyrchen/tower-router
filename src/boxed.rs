use crate::{
    handler::Handler,
    routing::{future::RouteFuture, Route},
    Router,
};
use axum_core::extract::Request;
use std::{convert::Infallible, fmt};
use tower::Service;

pub(crate) struct BoxedIntoRoute<Req, Res, S, E>(Box<dyn ErasedIntoRoute<Req, Res, S, E>>);

impl<Req, Res, S> BoxedIntoRoute<Req, Res, S, Infallible>
where
    S: Clone + Send + Sync + 'static,
{
    pub(crate) fn from_handler<H, T>(handler: H) -> Self
    where
        H: Handler<Req, T, S>,
        T: 'static,
    {
        Self(Box::new(MakeErasedHandler {
            handler,
            into_route: |handler, state| Route::new(Handler::with_state(handler, state)),
        }))
    }
}

impl<Req, Res, S, E> BoxedIntoRoute<Req, Res, S, E> {
    pub(crate) fn map<F, E2>(self, f: F) -> BoxedIntoRoute<Req, Res, S, E2>
    where
        S: 'static,
        E: 'static,
        F: FnOnce(Route<Req, Res, E>) -> Route<Req, Res, E2> + Clone + Send + 'static,
        E2: 'static,
    {
        BoxedIntoRoute(Box::new(Map {
            inner: self.0,
            layer: Box::new(f),
        }))
    }

    pub(crate) fn into_route(self, state: S) -> Route<Req, Res, E> {
        self.0.into_route(state)
    }
}

impl<Req, Res, S, E> Clone for BoxedIntoRoute<Req, Res, S, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl<Req, Res, S, E> fmt::Debug for BoxedIntoRoute<Req, Res, S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("BoxedIntoRoute").finish()
    }
}

pub(crate) trait ErasedIntoRoute<Req, Res, S, E>: Send {
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<Req, Res, S, E>>;

    fn into_route(self: Box<Self>, state: S) -> Route<Req, Res, E>;

    fn call_with_state(self: Box<Self>, request: Request, state: S) -> RouteFuture<Req, Res, E>;
}

pub(crate) struct MakeErasedHandler<Req, Res, H, S> {
    pub(crate) handler: H,
    pub(crate) into_route: fn(H, S) -> Route<Req, Res>,
}

impl<Req, Res, H, S> ErasedIntoRoute<Req, Res, S, Infallible> for MakeErasedHandler<Req, Res, H, S>
where
    H: Clone + Send + 'static,
    S: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<Req, Res, S, Infallible>> {
        Box::new(self.clone())
    }

    fn into_route(self: Box<Self>, state: S) -> Route<Req, Res> {
        (self.into_route)(self.handler, state)
    }

    fn call_with_state(
        self: Box<Self>,
        request: Request,
        state: S,
    ) -> RouteFuture<Req, Res, Infallible> {
        self.into_route(state).call(request)
    }
}

impl<Req, Res, H, S> Clone for MakeErasedHandler<Req, Res, H, S>
where
    H: Clone,
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),
            into_route: self.into_route,
        }
    }
}

pub(crate) struct MakeErasedRouter<Req, Res, S> {
    pub(crate) router: Router<Req, Res, S>,
    pub(crate) into_route: fn(Router<Req, Res, S>, S) -> Route<Req, Res>,
}

impl<Req, Res, S> ErasedIntoRoute<Req, Res, S, Infallible> for MakeErasedRouter<Req, Res, S>
where
    S: Clone + Send + Sync + 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<Req, Res, S, Infallible>> {
        Box::new(self.clone())
    }

    fn into_route(self: Box<Self>, state: S) -> Route<Req, Res> {
        (self.into_route)(self.router, state)
    }

    fn call_with_state(
        mut self: Box<Self>,
        request: Request,
        state: S,
    ) -> RouteFuture<Req, Res, Infallible> {
        self.router.call_with_state(request, state)
    }
}

impl<Req, Res, S> Clone for MakeErasedRouter<Req, Res, S>
where
    S: Clone,
{
    fn clone(&self) -> Self {
        Self {
            router: self.router.clone(),
            into_route: self.into_route,
        }
    }
}

pub(crate) struct Map<Req, Res, S, E, E2> {
    pub(crate) inner: Box<dyn ErasedIntoRoute<Req, Res, S, E>>,
    pub(crate) layer: Box<dyn LayerFn<Req, Res, E, E2>>,
}

impl<Req, Res, S, E, E2> ErasedIntoRoute<Req, Res, S, E2> for Map<Req, Res, S, E, E2>
where
    S: 'static,
    E: 'static,
    E2: 'static,
{
    fn clone_box(&self) -> Box<dyn ErasedIntoRoute<Req, Res, S, E2>> {
        Box::new(Self {
            inner: self.inner.clone_box(),
            layer: self.layer.clone_box(),
        })
    }

    fn into_route(self: Box<Self>, state: S) -> Route<Req, Res, E2> {
        (self.layer)(self.inner.into_route(state))
    }

    fn call_with_state(self: Box<Self>, request: Request, state: S) -> RouteFuture<Req, Res, E2> {
        (self.layer)(self.inner.into_route(state)).call(request)
    }
}

pub(crate) trait LayerFn<Req, Res, E, E2>:
    FnOnce(Route<Req, Res, E>) -> Route<Req, Res, E2> + Send
{
    fn clone_box(&self) -> Box<dyn LayerFn<Req, Res, E, E2>>;
}

impl<Req, Res, F, E, E2> LayerFn<Req, Res, E, E2> for F
where
    F: FnOnce(Route<Req, Res, E>) -> Route<Req, Res, E2> + Clone + Send + 'static,
{
    fn clone_box(&self) -> Box<dyn LayerFn<Req, Res, E, E2>> {
        Box::new(self.clone())
    }
}
