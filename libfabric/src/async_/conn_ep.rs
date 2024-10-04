use std::marker::PhantomData;

use crate::{
    ep::{
        Address, Connected, EndpointBase, EndpointImplBase, MrLocal, MrNone, Unconnected,
        UninitUnconnected,
    },
    eq::Event,
    fid::{AsRawFid, AsRawTypedFid, Fid},
    utils::check_error,
};

use super::{cq::AsyncReadCq, eq::AsyncReadEq};

pub type UninitUnconnectedEndpointBase<EP> = EndpointBase<EP, UninitUnconnected, MrNone>;

pub type UninitUnconnectedEndpoint<T> =
    UninitUnconnectedEndpointBase<EndpointImplBase<T, dyn AsyncReadEq, dyn AsyncReadCq>>;

pub type UnconnectedEndpointBase<EP> = EndpointBase<EP, Unconnected, MrNone>;

pub type UnconnectedEndpoint<T> =
    UnconnectedEndpointBase<EndpointImplBase<T, dyn AsyncReadEq, dyn AsyncReadCq>>;

pub type UnconnectedMrLocalEndpointBase<EP> = EndpointBase<EP, Unconnected, MrLocal>;

pub type UnconnectedMrLocalEndpoint<T> =
    UnconnectedMrLocalEndpointBase<EndpointImplBase<T, dyn AsyncReadEq, dyn AsyncReadCq>>;

pub enum UnconnectedEndpointB<E> {
    PlainData(UnconnectedEndpoint<E>),
    MrLocalData(UnconnectedMrLocalEndpoint<E>),
}

pub type ConnectedMrLocalEndpointBase<EP> = EndpointBase<EP, Connected, MrLocal>;

pub type ConnectedMrLocalEndpoint<T> =
    ConnectedEndpointBase<EndpointImplBase<T, dyn AsyncReadEq, dyn AsyncReadCq>>;
pub trait ConnectedEp {}

pub type ConnectedEndpointBase<EP> = EndpointBase<EP, Connected, MrNone>;

pub type ConnectedEndpoint<T> =
    ConnectedEndpointBase<EndpointImplBase<T, dyn AsyncReadEq, dyn AsyncReadCq>>;

impl<EP> ConnectedEp for ConnectedEndpointBase<EP> {}

impl<EP> UninitUnconnectedEndpoint<EP> {
    pub fn enable(self) -> Result<UnconnectedEndpointB<EP>, crate::error::Error> {
        // TODO: Move this into an UninitEp struct
        let err = unsafe { libfabric_sys::inlined_fi_enable(self.as_raw_typed_fid()) };
        check_error(err.try_into().unwrap())?;
        Ok(if self.inner.mr_local {
            UnconnectedEndpointB::MrLocalData(UnconnectedMrLocalEndpoint::<EP> {
                inner: self.inner.clone(),
                phantom_ep_state: PhantomData,
                phantom_mr_req: PhantomData,
            })
        } else {
            UnconnectedEndpointB::PlainData(UnconnectedEndpoint::<EP> {
                inner: self.inner.clone(),
                phantom_ep_state: PhantomData,
                phantom_mr_req: PhantomData,
            })
        })
    }
}

impl<EP> UnconnectedEndpoint<EP> {
    pub async fn connect_async(
        &self,
        addr: &Address,
    ) -> Result<ConnectedEndpoint<EP>, crate::error::Error> {
        self.connect(addr)?;

        let eq = self
            .inner
            .eq
            .get()
            .expect("Endpoint not bound to an EventQueue");
        let res = eq
            .async_event_wait(libfabric_sys::FI_CONNECTED, Fid(self.as_raw_fid()), 0)
            .await?;

        match res {
            Event::Connected(event) => {
                assert_eq!(event.get_fid(), self.as_raw_fid())
            }
            _ => panic!("Unexpected Event Type"),
        }

        Ok(ConnectedEndpoint {
            inner: self.inner.clone(),
            phantom_ep_state: PhantomData,
            phantom_mr_req: PhantomData,
        })
    }

    pub async fn accept_async(&self) -> Result<ConnectedEndpoint<EP>, crate::error::Error> {
        self.accept()?;

        let eq = self
            .inner
            .eq
            .get()
            .expect("Endpoint not bound to an EventQueue");
        let res = eq
            .async_event_wait(libfabric_sys::FI_CONNECTED, Fid(self.as_raw_fid()), 0)
            .await?;

        match res {
            Event::Connected(event) => {
                assert_eq!(event.get_fid(), self.as_raw_fid())
            }
            _ => panic!("Unexpected Event Type"),
        }

        Ok(ConnectedEndpoint {
            inner: self.inner.clone(),
            phantom_ep_state: PhantomData,
            phantom_mr_req: PhantomData,
        })
    }
}

impl<EP> UnconnectedMrLocalEndpoint<EP> {
    pub async fn connect_async(
        &self,
        addr: &Address,
    ) -> Result<ConnectedMrLocalEndpoint<EP>, crate::error::Error> {
        self.connect(addr)?;

        let eq = self
            .inner
            .eq
            .get()
            .expect("Endpoint not bound to an EventQueue");
        let res = eq
            .async_event_wait(libfabric_sys::FI_CONNECTED, Fid(self.as_raw_fid()), 0)
            .await?;

        match res {
            Event::Connected(event) => {
                assert_eq!(event.get_fid(), self.as_raw_fid())
            }
            _ => panic!("Unexpected Event Type"),
        }

        Ok(ConnectedEndpoint {
            inner: self.inner.clone(),
            phantom_ep_state: PhantomData,
            phantom_mr_req: PhantomData,
        })
    }

    pub async fn accept_async(&self) -> Result<ConnectedMrLocalEndpoint<EP>, crate::error::Error> {
        self.accept()?;

        let eq = self
            .inner
            .eq
            .get()
            .expect("Endpoint not bound to an EventQueue");
        let res = eq
            .async_event_wait(libfabric_sys::FI_CONNECTED, Fid(self.as_raw_fid()), 0)
            .await?;

        match res {
            Event::Connected(event) => {
                assert_eq!(event.get_fid(), self.as_raw_fid())
            }
            _ => panic!("Unexpected Event Type"),
        }

        Ok(ConnectedMrLocalEndpoint {
            inner: self.inner.clone(),
            phantom_ep_state: PhantomData,
            phantom_mr_req: PhantomData,
        })
    }
}
