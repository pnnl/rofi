use std::marker::PhantomData;

use crate::{
    cq::ReadCq,
    ep::{Address, Connected, EndpointBase, EndpointImplBase, Unconnected},
    eq::{ConnectedEvent, ReadEq},
    fid::{AsRawFid, AsRawTypedFid, EpRawFid},
    utils::check_error,
};

pub type UnconnectedEndpointBase<EP> = EndpointBase<EP, Unconnected>;

pub type UnconnectedEndpoint<T> =
    UnconnectedEndpointBase<EndpointImplBase<T, dyn ReadEq, dyn ReadCq>>;

impl<EP: AsRawTypedFid<Output = EpRawFid>> UnconnectedEndpointBase<EP> {
    pub fn connect_with<T>(&self, addr: &Address, param: &[T]) -> Result<(), crate::error::Error> {
        let err = unsafe {
            libfabric_sys::inlined_fi_connect(
                self.as_raw_typed_fid(),
                addr.as_bytes().as_ptr().cast(),
                param.as_ptr().cast(),
                param.len(),
            )
        };

        check_error(err.try_into().unwrap())
    }

    pub fn connect(&self, addr: &Address) -> Result<(), crate::error::Error> {
        let err = unsafe {
            libfabric_sys::inlined_fi_connect(
                self.as_raw_typed_fid(),
                addr.as_bytes().as_ptr().cast(),
                std::ptr::null_mut(),
                0,
            )
        };

        check_error(err.try_into().unwrap())
    }

    pub fn accept_with<T0>(&self, param: &[T0]) -> Result<(), crate::error::Error> {
        let err = unsafe {
            libfabric_sys::inlined_fi_accept(
                self.as_raw_typed_fid(),
                param.as_ptr().cast(),
                param.len(),
            )
        };

        check_error(err.try_into().unwrap())
    }

    pub fn accept(&self) -> Result<(), crate::error::Error> {
        let err = unsafe {
            libfabric_sys::inlined_fi_accept(self.as_raw_typed_fid(), std::ptr::null_mut(), 0)
        };

        check_error(err.try_into().unwrap())
    }
}

impl<E> UnconnectedEndpoint<E> {
    pub fn connect_complete(self, event: ConnectedEvent) -> ConnectedEndpoint<E> {
        // TODO: Create a type specifically for each event type

        assert_eq!(event.get_fid(), self.as_raw_fid());

        ConnectedEndpoint {
            inner: self.inner.clone(),
            phantom: PhantomData,
        }
    }
}

pub trait ConnectedEp {}

pub type ConnectedEndpointBase<EP> = EndpointBase<EP, Connected>;

pub type ConnectedEndpoint<T> = ConnectedEndpointBase<EndpointImplBase<T, dyn ReadEq, dyn ReadCq>>;

impl<EP> ConnectedEp for ConnectedEndpointBase<EP> {}

impl<EP: AsRawTypedFid<Output = EpRawFid>> ConnectedEndpointBase<EP> {
    pub fn shutdown(&self) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_shutdown(self.as_raw_typed_fid(), 0) };

        check_error(err.try_into().unwrap())
    }

    pub fn peer(&self) -> Result<Address, crate::error::Error> {
        let mut len = 0;
        let err = unsafe {
            libfabric_sys::inlined_fi_getpeer(
                self.as_raw_typed_fid(),
                std::ptr::null_mut(),
                &mut len,
            )
        };

        if -err as u32 == libfabric_sys::FI_ETOOSMALL {
            let mut address = vec![0; len];
            let err = unsafe {
                libfabric_sys::inlined_fi_getpeer(
                    self.as_raw_typed_fid(),
                    address.as_mut_ptr().cast(),
                    &mut len,
                )
            };
            if err != 0 {
                Err(crate::error::Error::from_err_code(
                    (-err).try_into().unwrap(),
                ))
            } else {
                Ok(Address { address })
            }
        } else {
            Err(crate::error::Error::from_err_code(
                (-err).try_into().unwrap(),
            ))
        }
    }
}
