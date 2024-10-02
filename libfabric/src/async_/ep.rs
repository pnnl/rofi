use std::marker::PhantomData;

use super::{
    conn_ep::UnconnectedEndpoint,
    connless_ep::ConnectionlessEndpoint,
    cq::{AsyncReadCq, CompletionQueue},
    eq::AsyncReadEq,
};
use crate::{
    av::AddressVectorBase,
    cq::ReadCq,
    domain::DomainBase,
    enums::EndpointType,
    ep::{
        EndpointAttr, EndpointBase, EndpointImplBase, EpCq, EpState, IncompleteBindCntr,
        PassiveEndpointBase, PassiveEndpointImplBase,
    },
    eq::{Event, EventQueueBase, ReadEq},
    fid::{AsRawFid, AsRawTypedFid, Fid, RawFid},
    info::InfoEntry,
    utils::check_error,
    Context, MyRc,
};

pub struct ConnectionListener {
    eq: MyRc<dyn AsyncReadEq>,
    ep_fid: RawFid,
}

impl ConnectionListener {
    fn new(ep_fid: RawFid, eq: &MyRc<dyn AsyncReadEq>) -> Self {
        Self {
            ep_fid,
            eq: eq.clone(),
        }
    }

    pub async fn next(&self) -> Result<Event, crate::error::Error> {
        let res = self
            .eq
            .async_event_wait(libfabric_sys::FI_CONNREQ, Fid(self.ep_fid), 0)
            .await?;
        Ok(res)
    }
}
pub enum Endpoint<EP> {
    Connectionless(ConnectionlessEndpoint<EP>),
    ConnectionOriented(UnconnectedEndpoint<EP>),
}

// pub type Endpoint<T> = EndpointBase<EndpointImplBase<T, dyn AsyncReadEq, dyn AsyncReadCq>>;

impl<STATE: EpState> EndpointBase<EndpointImplBase<(), dyn AsyncReadEq, dyn AsyncReadCq>, STATE> {
    pub(crate) fn new<E, DEQ: ?Sized + 'static>(
        domain: &crate::domain::DomainBase<DEQ>,
        info: &InfoEntry<E>,
        flags: u64,
        context: Option<&mut Context>,
    ) -> Result<
        EndpointBase<EndpointImplBase<E, dyn AsyncReadEq, dyn AsyncReadCq>, STATE>,
        crate::error::Error,
    > {
        let c_void = match context {
            Some(ctx) => ctx.inner_mut(),
            None => std::ptr::null_mut(),
        };

        Ok(
            EndpointBase::<EndpointImplBase<E, dyn AsyncReadEq, dyn AsyncReadCq>, STATE> {
                inner: MyRc::new(EndpointImplBase::new(&domain.inner, info, flags, c_void)?),
                phantom: PhantomData,
            },
        )
    }
}

// impl<T> Endpoint<T> {
//     pub async fn connect_async(&self, addr: &Address) -> Result<Event, crate::error::Error> {
//         self.inner.connect(addr)?;

//         let eq = self
//             .inner
//             .eq
//             .get()
//             .expect("Endpoint not bound to an EventQueue");
//         let res = eq
//             .async_event_wait(libfabric_sys::FI_CONNECTED, Fid(self.as_raw_fid()), 0)
//             .await?;
//         Ok(res)
//     }

//     pub async fn accept_async(&self) -> Result<Event, crate::error::Error> {
//         self.accept()?;

//         let eq = self
//             .inner
//             .eq
//             .get()
//             .expect("Endpoint not bound to an EventQueue");
//         let res = eq
//             .async_event_wait(libfabric_sys::FI_CONNECTED, Fid(self.as_raw_fid()), 0)
//             .await?;
//         Ok(res)
//     }
// }

#[allow(dead_code)]
pub struct IncompleteBindCq<'a, EP> {
    pub(crate) ep: &'a EndpointImplBase<EP, dyn AsyncReadEq, dyn AsyncReadCq>,
    pub(crate) flags: u64,
}

impl<EP> EndpointImplBase<EP, dyn AsyncReadEq, dyn AsyncReadCq> {
    // pub(crate) fn bind_cq_<T: AsyncReadCq + 'static>(&self, cq: &MyRc<T>, flags: u64) -> Result<(), crate::error::Error> {
    //     let err = unsafe { libfabric_sys::inlined_fi_ep_bind(self.as_raw_typed_fid(), cq.as_raw_fid(), flags) };

    //     if err != 0 {
    //         Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
    //     }
    //     else {

    //         if (flags & libfabric_sys::FI_TRANSMIT as u64) != 0 && (flags & libfabric_sys::FI_RECV as u64) != 0{
    //             if self.tx_cq.set(cq.clone()).is_err() {
    //                 panic!("Endpoint is already bound to a CompletionQueue for this direction");
    //             }
    //             if self.rx_cq.set(cq.clone()).is_err() {
    //                 panic!("Endpoint is already bound to a CompletionQueue for this direction");
    //             }
    //         }
    //         else if flags & libfabric_sys::FI_TRANSMIT as u64 != 0 {
    //             if self.tx_cq.set(cq.clone()).is_err() {
    //                 panic!("Endpoint is already bound to a CompletionQueue for this direction");
    //             }
    //         }
    //         else if flags & libfabric_sys::FI_RECV as u64 != 0{
    //             if self.rx_cq.set(cq.clone()).is_err() {
    //                 panic!("Endpoint is already bound to a CompletionQueue for this direction");
    //             }
    //         }
    //         else {
    //             panic!("Binding to Endpoint without specifying direction");
    //         }
    //         Ok(())
    //     }
    // }

    pub(crate) fn bind_shared_cq<T: AsRawFid + AsyncReadCq + 'static>(
        &self,
        cq: &MyRc<T>,
        selective: bool,
    ) -> Result<(), crate::error::Error> {
        let mut flags = libfabric_sys::FI_TRANSMIT as u64 | libfabric_sys::FI_RECV as u64;
        if selective {
            flags |= libfabric_sys::FI_SELECTIVE_COMPLETION;
        }

        let err = unsafe {
            libfabric_sys::inlined_fi_ep_bind(self.as_raw_typed_fid(), cq.as_raw_fid(), flags)
        };

        check_error(err as isize)?;
        if self.cq.set(EpCq::Shared(cq.clone())).is_err() {
            panic!("Endpoint already bound with another shared Completion Queueu");
        }

        Ok(())
    }

    pub(crate) fn bind_separate_cqs<T: AsRawFid + AsyncReadCq + 'static>(
        &self,
        tx_cq: &MyRc<T>,
        tx_selective: bool,
        rx_cq: &MyRc<T>,
        rx_selective: bool,
    ) -> Result<(), crate::error::Error> {
        let mut tx_flags = libfabric_sys::FI_TRANSMIT as u64;
        if tx_selective {
            tx_flags |= libfabric_sys::FI_SELECTIVE_COMPLETION;
        }

        let mut rx_flags = libfabric_sys::FI_RECV as u64;
        if rx_selective {
            rx_flags |= libfabric_sys::FI_SELECTIVE_COMPLETION;
        }

        let err = unsafe {
            libfabric_sys::inlined_fi_ep_bind(self.as_raw_typed_fid(), tx_cq.as_raw_fid(), tx_flags)
        };
        check_error(err as isize)?;

        let err = unsafe {
            libfabric_sys::inlined_fi_ep_bind(self.as_raw_typed_fid(), rx_cq.as_raw_fid(), rx_flags)
        };
        check_error(err as isize)?;

        if self
            .cq
            .set(EpCq::Separate(tx_cq.clone(), rx_cq.clone()))
            .is_err()
        {
            panic!("Endpoint already bound with other  Completion Queueus");
        }

        Ok(())
    }
}

impl<EP, CQ: ?Sized + ReadCq> EndpointImplBase<EP, dyn AsyncReadEq, CQ> {
    pub(crate) fn bind_eq<T: AsyncReadEq + 'static>(
        &self,
        eq: &MyRc<T>,
    ) -> Result<(), crate::error::Error> {
        let err = unsafe {
            libfabric_sys::inlined_fi_ep_bind(self.as_raw_typed_fid(), eq.as_raw_fid(), 0)
        };

        if err != 0 {
            Err(crate::error::Error::from_err_code(
                (-err).try_into().unwrap(),
            ))
        } else {
            if self.eq.set(eq.clone()).is_err() {
                panic!("Endpoint is already bound to another EventQueue"); // Should never reach this since inlined_fi_ep_bind will throw an error ealier
                                                                           // but keep it here to satisfy the compiler.
            }
            Ok(())
        }
    }

    // pub fn alias(&self, flags: u64) -> Result<Self, crate::error::Error> {
    //     Ok(
    //         Self {
    //             inner: MyRc::new (self.inner.alias(flags)?),
    //         }
    //     )
}

impl<EP, STATE: EpState>
    EndpointBase<EndpointImplBase<EP, dyn AsyncReadEq, dyn AsyncReadCq>, STATE>
{
    pub fn bind_cntr(&self) -> IncompleteBindCntr<EP, dyn AsyncReadEq, dyn AsyncReadCq> {
        self.inner.bind_cntr()
    }

    pub fn bind_av<EQ: ?Sized + ReadEq + 'static>(
        &self,
        av: &AddressVectorBase<EQ>,
    ) -> Result<(), crate::error::Error> {
        self.inner.bind_av(av)
    }

    // pub fn alias(&self, flags: u64) -> Result<Self, crate::error::Error> {
    //     Ok(
    //         Self {
    //             inner: MyRc::new (self.inner.alias(flags)?),
    //         }
    //     )
    // }
}

impl<EP, STATE: EpState>
    EndpointBase<EndpointImplBase<EP, dyn AsyncReadEq, dyn AsyncReadCq>, STATE>
{
    pub fn bind_shared_cq<T: AsRawFid + AsyncReadCq + 'static>(
        &self,
        cq: &CompletionQueue<T>,
    ) -> Result<(), crate::error::Error> {
        self.inner.bind_shared_cq(&cq.inner, false)
    }

    pub fn bind_separate_cqs<T: AsRawFid + AsyncReadCq + 'static>(
        &self,
        tx_cq: &CompletionQueue<T>,
        rx_cq: &CompletionQueue<T>,
    ) -> Result<(), crate::error::Error> {
        self.inner
            .bind_separate_cqs(&tx_cq.inner, false, &rx_cq.inner, false)
    }
}

impl<EP, STATE: EpState>
    EndpointBase<EndpointImplBase<EP, dyn AsyncReadEq, dyn AsyncReadCq>, STATE>
{
    pub fn bind_eq<T: AsyncReadEq + 'static>(
        &self,
        eq: &EventQueueBase<T>,
    ) -> Result<(), crate::error::Error> {
        self.inner.bind_eq(&eq.inner)
    }
}

// impl<'a, EP> IncompleteBindCq<'a, EP> {
//     pub fn recv(&mut self, selective: bool) -> &mut Self {
//         if selective {
//             self.flags |= libfabric_sys::FI_SELECTIVE_COMPLETION | libfabric_sys::FI_RECV  as u64 ;

//             self
//         }
//         else {
//             self.flags |= libfabric_sys::FI_RECV as u64;

//             self
//         }
//     }

//     pub fn transmit(&mut self, selective: bool) -> &mut Self {
//         if selective {
//             self.flags |= libfabric_sys::FI_SELECTIVE_COMPLETION | libfabric_sys::FI_TRANSMIT as u64;

//             self
//         }
//         else {
//             self.flags |= libfabric_sys::FI_TRANSMIT as u64;

//             self
//         }
//     }

//     pub fn cq<T: AsyncReadCq + 'static>(&mut self, cq: &crate::cq::CompletionQueueBase<T>) -> Result<(), crate::error::Error> {
//         self.ep.bind_cq_(&cq.inner, self.flags)
//     }
// }

// ============== Async stuff ======================= //
pub type PassiveEndpoint<T> = PassiveEndpointBase<T, dyn AsyncReadEq>;

impl<T> PassiveEndpoint<T> {
    pub fn listen_async(&self) -> Result<ConnectionListener, crate::error::Error> {
        self.listen()?;

        Ok(ConnectionListener::new(
            self.as_raw_fid(),
            self.inner.eq.get().unwrap(),
        ))
    }
}

impl<E> PassiveEndpointImplBase<E, dyn AsyncReadEq> {
    pub(crate) fn bind<T: AsyncReadEq + 'static>(
        &self,
        res: &MyRc<T>,
        flags: u64,
    ) -> Result<(), crate::error::Error> {
        let err = unsafe {
            libfabric_sys::inlined_fi_pep_bind(self.as_raw_typed_fid(), res.as_raw_fid(), flags)
        };
        if err != 0 {
            Err(crate::error::Error::from_err_code(
                (-err).try_into().unwrap(),
            ))
        } else {
            if self.eq.set(res.clone()).is_err() {
                panic!("Could not set oncecell")
            }
            Ok(())
        }
    }
}

impl<E> PassiveEndpointBase<E, dyn AsyncReadEq> {
    pub fn bind<T: AsyncReadEq + 'static>(
        &self,
        res: &EventQueueBase<T>,
        flags: u64,
    ) -> Result<(), crate::error::Error> {
        self.inner.bind(&res.inner, flags)
    }
}

pub struct EndpointBuilder<'a, E> {
    ep_attr: EndpointAttr,
    flags: u64,
    info: &'a InfoEntry<E>,
    ctx: Option<&'a mut Context>,
}

impl<'a> EndpointBuilder<'a, ()> {
    pub fn new<E>(info: &'a InfoEntry<E>) -> EndpointBuilder<'a, E> {
        EndpointBuilder {
            ep_attr: EndpointAttr::new(),
            flags: 0,
            info,
            ctx: None,
        }
    }
}

impl<'a, E> EndpointBuilder<'a, E> {
    pub fn build<DEQ: ?Sized + 'static>(
        self,
        domain: &DomainBase<DEQ>,
    ) -> Result<Endpoint<E>, crate::error::Error> {
        match self.info.ep_attr().type_() {
            EndpointType::Unspec => panic!("Should not be reachable."),
            EndpointType::Msg | EndpointType::SockStream => Ok(Endpoint::ConnectionOriented(
                UnconnectedEndpoint::new(domain, self.info, self.flags, self.ctx)?,
            )),
            EndpointType::Dgram | EndpointType::Rdm | EndpointType::SockDgram => {
                Ok(Endpoint::Connectionless(ConnectionlessEndpoint::new(
                    domain, self.info, self.flags, self.ctx,
                )?))
            }
        }
    }

    // pub fn build_scalable(self, domain: &crate::domain::Domain) -> Result<ScalableEndpoint<E>, crate::error::Error> {
    //     ScalableEndpoint::new(domain, self.info, self.ctx)
    // }

    pub fn build_passive(
        self,
        fabric: &crate::fabric::Fabric,
    ) -> Result<PassiveEndpoint<E>, crate::error::Error> {
        PassiveEndpoint::new(fabric, self.info, self.ctx)
    }

    pub fn flags(mut self, flags: u64) -> Self {
        self.flags = flags;
        self
    }

    pub fn ep_type(mut self, type_: crate::enums::EndpointType) -> Self {
        self.ep_attr.set_type(type_);
        self
    }

    pub fn protocol(mut self, proto: crate::enums::Protocol) -> Self {
        self.ep_attr.set_protocol(proto);
        self
    }

    pub fn max_msg_size(mut self, size: usize) -> Self {
        self.ep_attr.set_max_msg_size(size);
        self
    }

    pub fn msg_prefix_size(mut self, size: usize) -> Self {
        self.ep_attr.set_msg_prefix_size(size);
        self
    }

    pub fn max_order_raw_size(mut self, size: usize) -> Self {
        self.ep_attr.set_max_order_raw_size(size);
        self
    }

    pub fn max_order_war_size(mut self, size: usize) -> Self {
        self.ep_attr.set_max_order_war_size(size);
        self
    }

    pub fn max_order_waw_size(mut self, size: usize) -> Self {
        self.ep_attr.set_max_order_waw_size(size);
        self
    }

    pub fn mem_tag_format(mut self, tag: u64) -> Self {
        self.ep_attr.set_mem_tag_format(tag);
        self
    }

    pub fn tx_ctx_cnt(mut self, size: usize) -> Self {
        self.ep_attr.set_tx_ctx_cnt(size);
        self
    }

    pub fn rx_ctx_cnt(mut self, size: usize) -> Self {
        self.ep_attr.set_rx_ctx_cnt(size);
        self
    }

    pub fn auth_key(mut self, key: &mut [u8]) -> Self {
        self.ep_attr.set_auth_key(key);
        self
    }
}

pub trait AsyncCmEp {
    fn retrieve_eq(&self) -> &MyRc<impl AsyncReadEq + ?Sized>;
}
pub trait AsyncTxEp {
    fn retrieve_tx_cq(&self) -> &MyRc<impl AsyncReadCq + ?Sized>;
}

pub trait AsyncRxEp {
    fn retrieve_rx_cq(&self) -> &MyRc<impl AsyncReadCq + ?Sized>;
}

impl<EP, EQ: ?Sized + AsyncReadEq, CQ: ?Sized + AsyncReadCq> AsyncCmEp
    for EndpointImplBase<EP, EQ, CQ>
{
    fn retrieve_eq(&self) -> &MyRc<impl AsyncReadEq + ?Sized> {
        self.eq.get().unwrap()
    }
}

impl<EP, EQ: ?Sized + AsyncReadEq, CQ: ?Sized + AsyncReadCq> AsyncTxEp
    for EndpointImplBase<EP, EQ, CQ>
{
    fn retrieve_tx_cq(&self) -> &MyRc<impl AsyncReadCq + ?Sized> {
        match self
            .cq
            .get()
            .expect("Endpoint is not bound to Completion Queue(s)")
        {
            EpCq::Shared(tx_cq) | EpCq::Separate(tx_cq, _) => tx_cq,
        }
    }
}

impl<EP, EQ: ?Sized + AsyncReadEq, CQ: ?Sized + AsyncReadCq> AsyncRxEp
    for EndpointImplBase<EP, EQ, CQ>
{
    fn retrieve_rx_cq(&self) -> &MyRc<impl AsyncReadCq + ?Sized> {
        match self
            .cq
            .get()
            .expect("Endpoint is not bound to Completion Queue(s)")
        {
            EpCq::Shared(rx_cq) | EpCq::Separate(_, rx_cq) => rx_cq,
        }
    }
}

impl<EP: AsyncCmEp, STATE: EpState> AsyncCmEp for EndpointBase<EP, STATE> {
    fn retrieve_eq(&self) -> &MyRc<impl AsyncReadEq + ?Sized> {
        self.inner.retrieve_eq()
    }
}

impl<EP: AsyncTxEp, STATE: EpState> AsyncTxEp for EndpointBase<EP, STATE> {
    fn retrieve_tx_cq(&self) -> &MyRc<impl AsyncReadCq + ?Sized> {
        self.inner.retrieve_tx_cq()
    }
}

impl<EP: AsyncRxEp, STATE: EpState> AsyncRxEp for EndpointBase<EP, STATE> {
    fn retrieve_rx_cq(&self) -> &MyRc<impl AsyncReadCq + ?Sized> {
        self.inner.retrieve_rx_cq()
    }
}
