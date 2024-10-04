use std::marker::PhantomData;

use crate::{
    ep::{Connectionless, EndpointBase, EndpointImplBase, MrLocal, MrNone, UninitConnectionless},
    fid::AsRawTypedFid,
    utils::check_error,
};

use super::{cq::AsyncReadCq, eq::AsyncReadEq};

pub type UninitConnectionlessEndpointBase<EP> = EndpointBase<EP, UninitConnectionless, MrNone>;
pub type ConnectionlessEndpointBase<EP> = EndpointBase<EP, Connectionless, MrNone>;

pub type ConnectionlessEndpoint<E> =
    ConnectionlessEndpointBase<EndpointImplBase<E, dyn AsyncReadEq, dyn AsyncReadCq>>;
pub type ConnectionlessMrLocalEndpointBase<EP> = EndpointBase<EP, Connectionless, MrLocal>;

pub type UninitConnectionlessEndpoint<E> =
    UninitConnectionlessEndpointBase<EndpointImplBase<E, dyn AsyncReadEq, dyn AsyncReadCq>>;

pub type ConnectionlessMrLocalEndpoint<E> =
    ConnectionlessMrLocalEndpointBase<EndpointImplBase<E, dyn AsyncReadEq, dyn AsyncReadCq>>;

pub enum ConnectionlessEndpointB<E> {
    PlainData(ConnectionlessEndpoint<E>),
    MrLocalData(ConnectionlessMrLocalEndpoint<E>),
}

pub trait ConnlessEp {}
impl<EP> ConnlessEp for ConnectionlessEndpointBase<EP> {}

impl<EP> UninitConnectionlessEndpoint<EP> {
    pub fn enable(self) -> Result<ConnectionlessEndpointB<EP>, crate::error::Error> {
        // TODO: Move this into an UninitEp struct
        let err = unsafe { libfabric_sys::inlined_fi_enable(self.as_raw_typed_fid()) };
        check_error(err.try_into().unwrap())?;
        Ok(if self.inner.mr_local {
            ConnectionlessEndpointB::MrLocalData(ConnectionlessMrLocalEndpoint::<EP> {
                inner: self.inner.clone(),
                phantom_ep_state: PhantomData,
                phantom_mr_req: PhantomData,
            })
        } else {
            ConnectionlessEndpointB::PlainData(ConnectionlessEndpoint::<EP> {
                inner: self.inner.clone(),
                phantom_ep_state: PhantomData,
                phantom_mr_req: PhantomData,
            })
        })
    }
}
