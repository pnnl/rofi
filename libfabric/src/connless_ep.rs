use std::marker::PhantomData;

use crate::{
    cq::ReadCq,
    ep::{
        Connectionless, EndpointBase, EndpointImplBase, EpMrReq, MrLocal, MrNone,
        UninitConnectionless, UninitEndpoint,
    },
    eq::ReadEq,
    fid::{AsRawFid, AsRawTypedFid, EpRawFid},
    utils::check_error,
};

pub type UninitConnectionlessEndpointBase<EP> = EndpointBase<EP, UninitConnectionless, MrNone>;
pub type ConnectionlessEndpointBase<EP> = EndpointBase<EP, Connectionless, MrNone>;
pub type ConnectionlessMrLocalEndpointBase<EP> = EndpointBase<EP, Connectionless, MrLocal>;

pub type ConnectionlessEndpoint<E> =
    ConnectionlessEndpointBase<EndpointImplBase<E, dyn ReadEq, dyn ReadCq>>;

pub type ConnectionlessMrLocalEndpoint<E> =
    ConnectionlessMrLocalEndpointBase<EndpointImplBase<E, dyn ReadEq, dyn ReadCq>>;

pub enum ConnectionlessEndpointB<E> {
    PlainData(ConnectionlessEndpoint<E>),
    MrLocalData(ConnectionlessMrLocalEndpoint<E>),
}

pub type UninitConnectionlessEndpoint<E> =
    UninitConnectionlessEndpointBase<EndpointImplBase<E, dyn ReadEq, dyn ReadCq>>;

pub trait ConnlessEp {}
pub trait ConnlessMrEp {}

impl<EP> ConnlessEp for ConnectionlessEndpointBase<EP> {}
impl<EP: AsRawTypedFid<Output = EpRawFid> + AsRawFid> UninitEndpoint
    for UninitConnectionlessEndpointBase<EP>
{
}

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
