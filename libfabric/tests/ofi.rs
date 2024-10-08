use std::ops::Deref;

use libfabric::{
    av::AddressVectorBuilder,
    comm::{
        atomic::{
            AtomicCASEp, AtomicCASMrEp, AtomicFetchEp, AtomicFetchMrEp, AtomicWriteEp,
            AtomicWriteMrEp, ConnectedAtomicCASEp, ConnectedAtomicCASMrEp, ConnectedAtomicFetchEp,
            ConnectedAtomicFetchMrEp, ConnectedAtomicWriteEp, ConnectedAtomicWriteMrEp,
        },
        message::{
            ConnectedRecvEp, ConnectedRecvMrEp, ConnectedSendEp, ConnectedSendMrEp, RecvEp,
            RecvMrEp, SendEp, SendMrEp,
        },
        rma::{
            ConnectedReadEp, ConnectedReadMrEp, ConnectedWriteEp, ConnectedWriteMrEp, ReadEp,
            ReadMrEp, WriteEp, WriteMrLocalEp,
        },
        tagged::{
            ConnectedTagRecvEp, ConnectedTagRecvMrEp, ConnectedTagSendEp, ConnectedTagSendMrEp,
            TagRecvEp, TagRecvMrEp, TagSendEp, TagSendMrEp,
        },
    },
    conn_ep::{ConnectedEndpoint, ConnectedMrLocalEndpoint},
    connless_ep::{ConnectionlessEndpoint, ConnectionlessMrLocalEndpoint},
    cq::{Completion, CompletionQueue, CompletionQueueBuilder, ReadCq, WaitCq},
    domain::{Domain, DomainBuilder},
    enums::{
        AVOptions, AtomicMsgOptions, AtomicOp, CompareAtomicOp, CqFormat, EndpointType,
        FetchAtomicOp, ReadMsgOptions, TferOptions, WriteMsgOptions,
    },
    ep::{Address, BaseEndpoint, Endpoint, EndpointBuilder, UninitEndpoint},
    eq::{EventQueueBuilder, WaitEq},
    error::{Error, ErrorKind},
    fabric::FabricBuilder,
    info::{Info, InfoEntry, Version},
    infocapsoptions::{
        AtomicDefaultCap, Caps, CollCap, InfoCaps, MsgDefaultCap, RmaDefaultCap, TagDefaultCap,
    },
    iovec::{IoVec, IoVecMr, IoVecMut, IoVecMutMr, Ioc, IocMr, IocMut, IocMutMr, RmaIoVec, RmaIoc},
    mr::{
        default_desc, DisabledMemoryRegion, MappedMemoryRegionKey, MemoryRegion,
        MemoryRegionBuilder, MemoryRegionDesc, MemoryRegionKey, MemoryRegionSlice,
    },
    msg::{
        Msg, MsgAtomic, MsgAtomicConnected, MsgAtomicConnectedMr, MsgAtomicMr, MsgCompareAtomic,
        MsgCompareAtomicConnected, MsgCompareAtomicConnectedMr, MsgCompareAtomicMr, MsgConnected,
        MsgConnectedMr, MsgConnectedMut, MsgConnectedMutMr, MsgFetchAtomic,
        MsgFetchAtomicConnected, MsgFetchAtomicConnectedMr, MsgFetchAtomicMr, MsgMr, MsgMut,
        MsgMutMr, MsgRma, MsgRmaConnected, MsgRmaConnectedMr, MsgRmaConnectedMut,
        MsgRmaConnectedMutMr, MsgRmaMr, MsgRmaMut, MsgRmaMutMr, MsgTagged, MsgTaggedConnected,
        MsgTaggedConnectedMr, MsgTaggedConnectedMut, MsgTaggedConnectedMutMr, MsgTaggedMr,
        MsgTaggedMut, MsgTaggedMutMr,
    },
    Context, CqCaps, EqCaps, MappedAddress,
};
pub type SpinCq = libfabric::cq_caps_type!(CqCaps::WAIT);
pub type WaitableEq = libfabric::eq_caps_type!(EqCaps::WAIT);
pub mod common;

pub enum CqType {
    Separate((CompletionQueue<SpinCq>, CompletionQueue<SpinCq>)),
    Shared(CompletionQueue<SpinCq>),
}

pub enum MsgType<L, R, LL, RR> {
    ConnectionlessMsg(L),
    ConnectedMsg(R),
    ConnectionlessMrMsg(LL),
    ConnectedMrMsg(RR),
}

impl CqType {
    pub fn tx_cq(&self) -> &CompletionQueue<SpinCq> {
        match self {
            CqType::Separate((tx, _)) => tx,
            CqType::Shared(tx) => tx,
        }
    }

    pub fn rx_cq(&self) -> &CompletionQueue<SpinCq> {
        match self {
            CqType::Separate((_, rx)) => rx,
            CqType::Shared(rx) => rx,
        }
    }
}

// pub enum EpType<I> {
//     Connected(Endpoint<I>, EventQueue<WaitableEq>),
//     Connectionless(Endpoint<I>, MappedAddress),
// }

pub enum MyEndpoint<I> {
    Connected(ConnectedEndpoint<I>),
    ConnectedMrLocal(ConnectedMrLocalEndpoint<I>),
    Connectionless(ConnectionlessEndpoint<I>),
    ConnectionlessMrLocal(ConnectionlessMrLocalEndpoint<I>),
}

pub struct Ofi<I> {
    pub info_entry: InfoEntry<I>,
    pub mr: Option<MemoryRegion>,
    pub key: Option<MemoryRegionKey>,
    pub remote_key: Option<MappedMemoryRegionKey>,
    pub remote_mem_addr: Option<(u64, u64)>,
    pub domain: Domain,
    pub cq_type: CqType,
    pub ep: MyEndpoint<I>,
    pub mapped_addr: Option<MappedAddress>,
    pub reg_mem: Vec<u8>,
    // pub tx_pending_cnt: AtomicUsize,
    // pub tx_complete_cnt: AtomicUsize,
    // pub rx_pending_cnt: AtomicUsize,
    // pub rx_complete_cnt: AtomicUsize,
}

impl<I> Drop for Ofi<I> {
    fn drop(&mut self) {
        match self.info_entry.ep_attr().type_() {
            EndpointType::Msg | EndpointType::SockStream => match &self.ep {
                MyEndpoint::Connected(ep) => ep.shutdown().unwrap(),
                MyEndpoint::ConnectedMrLocal(ep) => ep.shutdown().unwrap(),
                MyEndpoint::Connectionless(_) | MyEndpoint::ConnectionlessMrLocal(_) => todo!(),
            },
            EndpointType::Unspec
            | EndpointType::Dgram
            | EndpointType::Rdm
            | EndpointType::SockDgram => {}
        }
    }
}

macro_rules!  post{
    ($post_fn:ident, $prog_fn:ident, $cq:expr, $ep:ident, $( $x:expr),* ) => {
        loop {
            let ret = $ep.$post_fn($($x,)*);
            if ret.is_ok() {
                break;
            }
            else if let Err(ref err) = ret {
                if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                    panic!("Unexpected error!")
                }

            }
            $prog_fn($cq);
        }
    };
}

pub fn ft_progress(cq: &impl ReadCq) {
    let ret = cq.read(0);
    match ret {
        Ok(_) => {
            panic!("Should not read anything")
        }
        Err(ref err) => {
            if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                ret.unwrap();
            }
        }
    }
}

impl<I: MsgDefaultCap + Caps + 'static> Ofi<I> {
    pub fn new(
        info_entry: InfoEntry<I>,
        shared_cqs: bool,
        server: bool,
        name: &str,
    ) -> Result<Self, Error> {
        if server {
            unsafe { std::env::set_var(name, "1") };
        } else {
            while std::env::var(name).is_err() {
                std::thread::yield_now();
            }
        }

        let format = if info_entry.caps().is_tagged() {
            CqFormat::Tagged
        } else {
            CqFormat::Data
        };

        let fabric = FabricBuilder::new().build(&info_entry).unwrap();
        let tx_cq_builder = CompletionQueueBuilder::new()
            .size(info_entry.tx_attr().size())
            .format(format);

        let rx_cq_builder = CompletionQueueBuilder::new()
            .size(info_entry.rx_attr().size())
            .format(format);

        let shared_cq_builder = CompletionQueueBuilder::new()
            .size(info_entry.rx_attr().size() + info_entry.tx_attr().size())
            .format(format);

        let ep_type = info_entry.ep_attr().type_();
        let domain;
        let cq_type;
        let mr;
        let key;

        // let mut tx_pending_cnt: usize = 0;
        // let mut tx_complete_cnt: usize = 0;
        // let mut rx_pending_cnt: usize = 0;
        // let mut rx_complete_cnt: usize = 0;
        let mut reg_mem = vec![0u8; 1024 * 1024];

        let (info_entry, ep, mapped_addr) = match ep_type {
            EndpointType::Msg | EndpointType::SockStream => {
                let eq = EventQueueBuilder::new(&fabric).build().unwrap();

                let info_entry = if server {
                    let pep = EndpointBuilder::new(&info_entry)
                        .build_passive(&fabric)
                        .unwrap();
                    pep.bind(&eq, 0).unwrap();
                    pep.listen().unwrap();
                    let event = eq.sread(-1).unwrap();
                    match event {
                        libfabric::eq::Event::ConnReq(entry) => entry.get_info().unwrap(),
                        _ => panic!("Unexpected event"),
                    }
                } else {
                    info_entry
                };

                domain = DomainBuilder::new(&fabric, &info_entry).build().unwrap();

                cq_type = if shared_cqs {
                    CqType::Shared(shared_cq_builder.build(&domain).unwrap())
                } else {
                    CqType::Separate((
                        tx_cq_builder.build(&domain).unwrap(),
                        rx_cq_builder.build(&domain).unwrap(),
                    ))
                };
                let ep = match EndpointBuilder::new(&info_entry).build(&domain).unwrap() {
                    Endpoint::Connectionless(_) => panic!("Expected connected EP"),
                    Endpoint::ConnectionOriented(unconn_ep) => unconn_ep,
                };
                ep.bind_eq(&eq).unwrap();
                match cq_type {
                    CqType::Separate((ref tx_cq, ref rx_cq)) => {
                        ep.bind_separate_cqs(tx_cq, false, rx_cq, false).unwrap()
                    }
                    CqType::Shared(ref scq) => ep.bind_shared_cq(&scq, false).unwrap(),
                }

                let ep = match ep.enable().unwrap() {
                    libfabric::conn_ep::UnconnectedEndpointB::PlainData(ep) => {
                        if !server {
                            ep.connect(info_entry.dest_addr().unwrap()).unwrap();
                        } else {
                            ep.accept().unwrap();
                        }

                        let ep = match eq.sread(-1) {
                            Ok(event) => match event {
                                libfabric::eq::Event::Connected(event) => {
                                    ep.connect_complete(event)
                                }
                                _ => panic!("Unexpected Event type"),
                            },
                            Err(err) => {
                                if matches!(err.kind, ErrorKind::ErrorAvailable) {
                                    let err = eq.readerr().unwrap();
                                    panic!("Error in EQ: {}", eq.strerror(&err))
                                } else {
                                    panic!("Error in EQ: {:?}", err)
                                }
                            }
                        };
                        MyEndpoint::Connected(ep)
                    }
                    libfabric::conn_ep::UnconnectedEndpointB::MrLocalData(ep) => {
                        if !server {
                            ep.connect(info_entry.dest_addr().unwrap()).unwrap();
                        } else {
                            ep.accept().unwrap();
                        }

                        let ep = match eq.sread(-1) {
                            Ok(event) => match event {
                                libfabric::eq::Event::Connected(event) => {
                                    ep.connect_complete(event)
                                }
                                _ => panic!("Unexpected Event type"),
                            },
                            Err(err) => {
                                if matches!(err.kind, ErrorKind::ErrorAvailable) {
                                    let err = eq.readerr().unwrap();
                                    panic!("Error in EQ: {}", eq.strerror(&err))
                                } else {
                                    panic!("Error in EQ: {:?}", err)
                                }
                            }
                        };
                        MyEndpoint::ConnectedMrLocal(ep)
                    }
                };

                (mr, key) = if info_entry.domain_attr().mr_mode().is_local()
                    || info_entry.caps().is_rma()
                {
                    let mr =
                        MemoryRegionBuilder::new(&mut reg_mem, libfabric::enums::HmemIface::System)
                            .access_read()
                            .access_write()
                            .access_send()
                            .access_recv()
                            .build(&domain)?;
                    let mr = match mr {
                        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
                        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
                            match ep {
                                MyEndpoint::Connected(ref ep) => mr.bind_ep(&ep).unwrap(),
                                MyEndpoint::ConnectedMrLocal(ref ep) => mr.bind_ep(&ep).unwrap(),
                                MyEndpoint::Connectionless(ref ep) => mr.bind_ep(&ep).unwrap(),
                                MyEndpoint::ConnectionlessMrLocal(ref ep) => {
                                    mr.bind_ep(&ep).unwrap()
                                }
                            }

                            mr.enable().unwrap()
                        }
                    };
                    let key = mr.key().unwrap();
                    (Some(mr), Some(key))
                } else {
                    (None, None)
                };

                (info_entry, ep, None)
            }
            _ => {
                domain = DomainBuilder::new(&fabric, &info_entry).build().unwrap();

                cq_type = if shared_cqs {
                    CqType::Shared(shared_cq_builder.build(&domain).unwrap())
                } else {
                    CqType::Separate((
                        tx_cq_builder.build(&domain).unwrap(),
                        rx_cq_builder.build(&domain).unwrap(),
                    ))
                };

                let ep = match EndpointBuilder::new(&info_entry).build(&domain).unwrap() {
                    Endpoint::Connectionless(ep) => ep,
                    Endpoint::ConnectionOriented(_) => panic!("Expected connectionless ep"),
                };
                match cq_type {
                    CqType::Separate((ref tx_cq, ref rx_cq)) => {
                        ep.bind_separate_cqs(tx_cq, false, rx_cq, false).unwrap()
                    }
                    CqType::Shared(ref scq) => ep.bind_shared_cq(&scq, false).unwrap(),
                }

                let av = match info_entry.domain_attr().av_type() {
                    libfabric::enums::AddressVectorType::Unspec => AddressVectorBuilder::new(),
                    _ => AddressVectorBuilder::new().type_(*info_entry.domain_attr().av_type()),
                }
                .build(&domain)
                .unwrap();
                ep.bind_av(&av).unwrap();
                let ep = match ep.enable().unwrap() {
                    libfabric::connless_ep::ConnectionlessEndpointB::PlainData(ep) => {
                        MyEndpoint::Connectionless(ep)
                    }
                    libfabric::connless_ep::ConnectionlessEndpointB::MrLocalData(ep) => {
                        MyEndpoint::ConnectionlessMrLocal(ep)
                    }
                };
                (mr, key) = if info_entry.domain_attr().mr_mode().is_local()
                    || info_entry.caps().is_rma()
                {
                    let mr =
                        MemoryRegionBuilder::new(&mut reg_mem, libfabric::enums::HmemIface::System)
                            .access_read()
                            .access_write()
                            .access_send()
                            .access_recv()
                            .build(&domain)?;
                    let mr = match mr {
                        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
                        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
                            match ep {
                                MyEndpoint::Connected(ref ep) => mr.bind_ep(&ep).unwrap(),
                                MyEndpoint::ConnectedMrLocal(ref ep) => mr.bind_ep(&ep).unwrap(),
                                MyEndpoint::Connectionless(ref ep) => mr.bind_ep(&ep).unwrap(),
                                MyEndpoint::ConnectionlessMrLocal(ref ep) => {
                                    mr.bind_ep(&ep).unwrap()
                                }
                            }
                            mr.enable().unwrap()
                        }
                    };
                    let key = mr.key().unwrap();
                    (Some(mr), Some(key))
                } else {
                    (None, None)
                };

                let epname = match ep {
                    MyEndpoint::Connected(ref ep) => ep.getname().unwrap(),
                    MyEndpoint::ConnectedMrLocal(ref ep) => ep.getname().unwrap(),
                    MyEndpoint::Connectionless(ref ep) => ep.getname().unwrap(),
                    MyEndpoint::ConnectionlessMrLocal(ref ep) => ep.getname().unwrap(),
                };
                let mapped_address = if let Some(dest_addr) = info_entry.dest_addr() {
                    let mapped_address = av
                        .insert(std::slice::from_ref(dest_addr).into(), AVOptions::new())
                        .unwrap()
                        .pop()
                        .unwrap()
                        .unwrap();

                    let epname_bytes = epname.as_bytes();
                    let addrlen = epname_bytes.len();
                    reg_mem[..addrlen].copy_from_slice(epname_bytes);

                    match ep {
                        MyEndpoint::ConnectionlessMrLocal(ref ep) => {
                            let mr_slice = mr.as_ref().unwrap().slice(0);

                            post!(
                                send_to,
                                ft_progress,
                                cq_type.tx_cq(),
                                ep,
                                &mr_slice.slice(..addrlen),
                                &mut default_desc(),
                                &mapped_address
                            );
                        }
                        MyEndpoint::Connectionless(ref ep) => {
                            post!(
                                send_to,
                                ft_progress,
                                cq_type.tx_cq(),
                                ep,
                                &reg_mem[..addrlen],
                                &mut default_desc(),
                                &mapped_address
                            );
                        }
                        _ => panic!("Connectionless only"),
                    }

                    cq_type.tx_cq().sread(1, -1).unwrap();

                    match ep {
                        MyEndpoint::Connectionless(ref ep) => {
                            post!(
                                recv_from_any,
                                ft_progress,
                                cq_type.rx_cq(),
                                ep,
                                std::slice::from_mut(&mut reg_mem[0]),
                                &mut default_desc()
                            );
                        }
                        MyEndpoint::ConnectionlessMrLocal(ref ep) => {
                            let mr_slice = mr.as_ref().unwrap().slice(0);

                            post!(
                                recv_from_any,
                                ft_progress,
                                cq_type.rx_cq(),
                                ep,
                                &mut mr_slice.slice(0..1),
                                &mut default_desc()
                            );
                        }
                        _ => panic!("Connectionless only"),
                    }
                    // ep.recv(std::slice::from_mut(&mut ack), &mut default_desc()).unwrap();

                    cq_type.rx_cq().sread(1, -1).unwrap();

                    mapped_address
                } else {
                    let addrlen = epname.as_bytes().len();

                    let mut mr_desc = if let Some(ref mr) = mr {
                        mr.description()
                    } else {
                        default_desc()
                    };
                    match ep {
                        MyEndpoint::Connectionless(ref ep) => {
                            post!(
                                recv_from_any,
                                ft_progress,
                                cq_type.rx_cq(),
                                ep,
                                &mut reg_mem[..addrlen],
                                &mut mr_desc
                            );
                        }
                        MyEndpoint::ConnectionlessMrLocal(ref ep) => {
                            let mr_slice = mr.as_ref().unwrap().slice(0);
                            post!(
                                recv_from_any,
                                ft_progress,
                                cq_type.rx_cq(),
                                ep,
                                &mut mr_slice.slice(..addrlen),
                                &mut mr_desc
                            );
                        }
                        _ => panic!("Connectionless only"),
                    }

                    cq_type.rx_cq().sread(1, -1).unwrap();
                    // ep.recv(&mut reg_mem, &mut mr_desc).unwrap();
                    let remote_address = unsafe { Address::from_bytes(&reg_mem) };
                    let mapped_address = av
                        .insert(
                            std::slice::from_ref(&remote_address).into(),
                            AVOptions::new(),
                        )
                        .unwrap()
                        .pop()
                        .unwrap()
                        .unwrap();
                    match ep {
                        MyEndpoint::Connectionless(ref ep) => {
                            post!(
                                send_to,
                                ft_progress,
                                cq_type.tx_cq(),
                                ep,
                                &std::slice::from_ref(&reg_mem[0]),
                                &mut mr_desc,
                                &mapped_address
                            );
                        }
                        MyEndpoint::ConnectionlessMrLocal(ref ep) => {
                            let mr_slice = mr.as_ref().unwrap().slice(0);

                            post!(
                                send_to,
                                ft_progress,
                                cq_type.tx_cq(),
                                ep,
                                &mr_slice.slice(0..1),
                                &mut mr_desc,
                                &mapped_address
                            );
                        }
                        _ => panic!("Connectionless only"),
                    }

                    cq_type.tx_cq().sread(1, -1).unwrap();

                    mapped_address
                };
                (info_entry, ep, Some(mapped_address))
            }
        };
        if server {
            unsafe { std::env::remove_var(name) };
        }

        Ok(Self {
            info_entry,
            mapped_addr,
            mr,
            key,
            remote_key: None,
            remote_mem_addr: None,
            cq_type,
            domain,
            ep,
            reg_mem,
            // tx_pending_cnt,
            // tx_complete_cnt,
            // rx_pending_cnt,
            // rx_complete_cnt,
        })
    }
}

impl<I: TagDefaultCap> Ofi<I> {
    pub fn tsend<T>(&self, buf: &[T], desc: &mut MemoryRegionDesc, tag: u64, data: Option<u64>) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            ep.tinjectdata_to(
                                &buf,
                                data.unwrap(),
                                self.mapped_addr.as_ref().unwrap(),
                                tag,
                            )
                        } else {
                            ep.tinject_to(&buf, self.mapped_addr.as_ref().unwrap(), tag)
                        }
                    } else {
                        if data.is_some() {
                            ep.tsenddata_to(
                                &buf,
                                desc,
                                data.unwrap(),
                                self.mapped_addr.as_ref().unwrap(),
                                tag,
                            )
                        } else {
                            ep.tsend_to(&buf, desc, self.mapped_addr.as_ref().unwrap(), tag)
                        }
                    }
                }
                MyEndpoint::Connected(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            ep.tinjectdata(&buf, data.unwrap(), tag)
                        } else {
                            ep.tinject(&buf, tag)
                        }
                    } else {
                        if data.is_some() {
                            ep.tsenddata(&buf, desc, data.unwrap(), tag)
                        } else {
                            ep.tsend(&buf, desc, tag)
                        }
                    }
                }
                _ => panic!("Only handles plain data"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn tsend_mr<T: Copy>(
        &self,
        buf: &MemoryRegionSlice<T>,
        desc: &mut MemoryRegionDesc,
        tag: u64,
        data: Option<u64>,
    ) {
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            ep.tinjectdata_to(
                                buf,
                                data.unwrap(),
                                self.mapped_addr.as_ref().unwrap(),
                                tag,
                            )
                        } else {
                            ep.tinject_to(&buf, self.mapped_addr.as_ref().unwrap(), tag)
                        }
                    } else {
                        if data.is_some() {
                            ep.tsenddata_to(
                                buf,
                                desc,
                                data.unwrap(),
                                self.mapped_addr.as_ref().unwrap(),
                                tag,
                            )
                        } else {
                            ep.tsend_to(&buf, desc, self.mapped_addr.as_ref().unwrap(), tag)
                        }
                    }
                }
                MyEndpoint::ConnectedMrLocal(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            ep.tinjectdata(buf, data.unwrap(), tag)
                        } else {
                            ep.tinject(buf, tag)
                        }
                    } else {
                        if data.is_some() {
                            ep.tsenddata(buf, desc, data.unwrap(), tag)
                        } else {
                            ep.tsend(&buf, desc, tag)
                        }
                    }
                }
                _ => panic!("Only handles mr data"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn tsendv(&mut self, iov: &[IoVec], desc: &mut [MemoryRegionDesc], tag: u64) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => {
                    ep.tsendv_to(iov, desc, self.mapped_addr.as_ref().unwrap(), tag)
                }
                MyEndpoint::Connected(ep) => ep.tsendv(iov, desc, tag),
                _ => panic!("Only handles plain data"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn tsendv_mr(&mut self, iov: &[IoVecMr], desc: &mut [MemoryRegionDesc], tag: u64) {
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => {
                    ep.tsendv_to(iov, desc, self.mapped_addr.as_ref().unwrap(), tag)
                }
                MyEndpoint::ConnectedMrLocal(ep) => ep.tsendv(iov, desc, tag),
                _ => panic!("Only handles plain data"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn trecvv(&mut self, iov: &[IoVecMut], desc: &mut [MemoryRegionDesc], tag: u64) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => {
                    ep.trecvv_from(iov, desc, self.mapped_addr.as_ref().unwrap(), tag, 0)
                }
                MyEndpoint::Connected(ep) => ep.trecvv(iov, desc, 0, tag),
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn trecvv_mr(&mut self, iov: &[IoVecMutMr], desc: &mut [MemoryRegionDesc], tag: u64) {
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => {
                    ep.trecvv_from(iov, desc, self.mapped_addr.as_ref().unwrap(), tag, 0)
                }
                MyEndpoint::ConnectedMrLocal(ep) => ep.trecvv(iov, desc, 0, tag),
                _ => panic!("Mr data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn trecv<T>(&mut self, buf: &mut [T], desc: &mut MemoryRegionDesc, tag: u64) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => {
                    ep.trecv_from(buf, desc, self.mapped_addr.as_ref().unwrap(), tag, 0)
                }
                MyEndpoint::Connected(ep) => ep.trecv(buf, desc, tag, 0),
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn trecv_mr<T: Copy>(
        &mut self,
        buf: &mut MemoryRegionSlice<T>,
        desc: &mut MemoryRegionDesc,
        tag: u64,
    ) {
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => {
                    ep.trecv_from(buf, desc, self.mapped_addr.as_ref().unwrap(), tag, 0)
                }
                MyEndpoint::ConnectedMrLocal(ep) => ep.trecv(buf, desc, tag, 0),
                _ => panic!("Mr data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn tsendmsg(
        &mut self,
        msg: &MsgType<MsgTagged, MsgTaggedConnected, MsgTaggedMr, MsgTaggedConnectedMr>,
    ) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => match msg {
                    MsgType::ConnectionlessMsg(msg) => {
                        ep.tsendmsg_to(msg, TferOptions::new().remote_cq_data())
                    }
                    MsgType::ConnectedMsg(_) => panic!("Wrong message type used"),
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::Connected(ep) => match msg {
                    MsgType::ConnectionlessMsg(_) => panic!("Wrong message type used"),
                    MsgType::ConnectedMsg(msg) => {
                        ep.tsendmsg(msg, TferOptions::new().remote_cq_data())
                    }
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::ConnectionlessMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(msg) => {
                        ep.tsendmsg_to(msg, TferOptions::new().remote_cq_data())
                    }
                    MsgType::ConnectedMrMsg(_) => panic!("Wrong message type used"),
                    _ => panic!("Mr data only"),
                },
                MyEndpoint::ConnectedMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(_) => panic!("Wrong message type used"),
                    MsgType::ConnectedMrMsg(msg) => {
                        ep.tsendmsg(msg, TferOptions::new().remote_cq_data())
                    }
                    _ => panic!("Mr data only"),
                },
            };

            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn trecvmsg(
        &mut self,
        msg: &MsgType<MsgTaggedMut, MsgTaggedConnectedMut, MsgTaggedMutMr, MsgTaggedConnectedMutMr>,
    ) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => match msg {
                    MsgType::ConnectionlessMsg(msg) => ep.trecvmsg_from(msg, TferOptions::new()),
                    MsgType::ConnectedMsg(_) => panic!("Wrong message type"),
                    _ => panic!("Only plain data"),
                },
                MyEndpoint::Connected(ep) => match msg {
                    MsgType::ConnectionlessMsg(_) => panic!("Wrong message type"),
                    MsgType::ConnectedMsg(msg) => ep.trecvmsg(msg, TferOptions::new()),
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::ConnectionlessMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(msg) => ep.trecvmsg_from(msg, TferOptions::new()),
                    MsgType::ConnectedMrMsg(_) => panic!("Wrong message type"),
                    _ => panic!("Only plain data"),
                },
                MyEndpoint::ConnectedMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(_) => panic!("Wrong message type"),
                    MsgType::ConnectedMrMsg(msg) => ep.trecvmsg(msg, TferOptions::new()),
                    _ => panic!("Plain data only"),
                },
            };

            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }
}

impl<I: MsgDefaultCap + 'static> Ofi<I> {
    pub fn send<T>(&self, buf: &[T], desc: &mut MemoryRegionDesc, data: Option<u64>) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            ep.injectdata_to(buf, data.unwrap(), self.mapped_addr.as_ref().unwrap())
                        } else {
                            ep.inject_to(&buf, self.mapped_addr.as_ref().unwrap())
                        }
                    } else {
                        if data.is_some() {
                            ep.senddata_to(
                                &buf,
                                desc,
                                data.unwrap(),
                                self.mapped_addr.as_ref().unwrap(),
                            )
                        } else {
                            ep.send_to(&buf, desc, self.mapped_addr.as_ref().unwrap())
                        }
                    }
                }
                MyEndpoint::Connected(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            ep.injectdata(&buf, data.unwrap())
                        } else {
                            ep.inject(&buf)
                        }
                    } else {
                        if data.is_some() {
                            ep.senddata(&buf, desc, data.unwrap())
                        } else {
                            ep.send(&buf, desc)
                        }
                    }
                }
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }
    pub fn send_mr<T: Copy>(
        &self,
        buf: &MemoryRegionSlice<T>,
        desc: &mut MemoryRegionDesc,
        data: Option<u64>,
    ) {
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            ep.injectdata_to(buf, data.unwrap(), self.mapped_addr.as_ref().unwrap())
                        } else {
                            ep.inject_to(&buf, self.mapped_addr.as_ref().unwrap())
                        }
                    } else {
                        if data.is_some() {
                            ep.senddata_to(
                                &buf,
                                desc,
                                data.unwrap(),
                                self.mapped_addr.as_ref().unwrap(),
                            )
                        } else {
                            ep.send_to(&buf, desc, self.mapped_addr.as_ref().unwrap())
                        }
                    }
                }
                MyEndpoint::ConnectedMrLocal(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            ep.injectdata(&buf, data.unwrap())
                        } else {
                            ep.inject(&buf)
                        }
                    } else {
                        if data.is_some() {
                            ep.senddata(&buf, desc, data.unwrap())
                        } else {
                            ep.send(&buf, desc)
                        }
                    }
                }
                _ => panic!("Mr data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn send_with_context<T>(
        &self,
        buf: &[T],
        desc: &mut MemoryRegionDesc,
        data: Option<u64>,
        context: &mut Context,
    ) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            ep.injectdata_to(buf, data.unwrap(), self.mapped_addr.as_ref().unwrap())
                        } else {
                            ep.inject_to(&buf, self.mapped_addr.as_ref().unwrap())
                        }
                    } else {
                        if data.is_some() {
                            ep.senddata_to_with_context(
                                &buf,
                                desc,
                                data.unwrap(),
                                self.mapped_addr.as_ref().unwrap(),
                                context,
                            )
                        } else {
                            ep.send_to_with_context(
                                &buf,
                                desc,
                                self.mapped_addr.as_ref().unwrap(),
                                context,
                            )
                        }
                    }
                }
                MyEndpoint::Connected(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            ep.injectdata(&buf, data.unwrap())
                        } else {
                            ep.inject(&buf)
                        }
                    } else {
                        if data.is_some() {
                            ep.senddata_with_context(&buf, desc, data.unwrap(), context)
                        } else {
                            ep.send_with_context(&buf, desc, context)
                        }
                    }
                }
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn send_mr_with_context<T: Copy>(
        &self,
        buf: &MemoryRegionSlice<T>,
        desc: &mut MemoryRegionDesc,
        data: Option<u64>,
        context: &mut Context,
    ) {
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            ep.injectdata_to(buf, data.unwrap(), self.mapped_addr.as_ref().unwrap())
                        } else {
                            ep.inject_to(&buf, self.mapped_addr.as_ref().unwrap())
                        }
                    } else {
                        if data.is_some() {
                            ep.senddata_to_with_context(
                                &buf,
                                desc,
                                data.unwrap(),
                                self.mapped_addr.as_ref().unwrap(),
                                context,
                            )
                        } else {
                            ep.send_to_with_context(
                                &buf,
                                desc,
                                self.mapped_addr.as_ref().unwrap(),
                                context,
                            )
                        }
                    }
                }
                MyEndpoint::ConnectedMrLocal(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            ep.injectdata(&buf, data.unwrap())
                        } else {
                            ep.inject(&buf)
                        }
                    } else {
                        if data.is_some() {
                            ep.senddata_with_context(&buf, desc, data.unwrap(), context)
                        } else {
                            ep.send_with_context(&buf, desc, context)
                        }
                    }
                }
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn sendv(&mut self, iov: &[IoVec], desc: &mut [MemoryRegionDesc]) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => {
                    ep.sendv_to(iov, desc, self.mapped_addr.as_ref().unwrap())
                }
                MyEndpoint::Connected(ep) => ep.sendv(iov, desc),
                _ => panic!("Only handles plain data"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn sendv_mr(&mut self, iov: &[IoVecMr], desc: &mut [MemoryRegionDesc]) {
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectedMrLocal(ep) => ep.sendv(iov, desc),
                MyEndpoint::ConnectionlessMrLocal(ep) => {
                    ep.sendv_to(iov, desc, self.mapped_addr.as_ref().unwrap())
                }
                _ => panic!("Does not handle plain data"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn recvv(&mut self, iov: &[IoVecMut], desc: &mut [MemoryRegionDesc]) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => {
                    ep.recvv_from(iov, desc, self.mapped_addr.as_ref().unwrap())
                }
                MyEndpoint::Connected(ep) => ep.recvv(iov, desc),
                _ => panic!("Only handles plain data"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn recvv_mr(&mut self, iov: &[IoVecMutMr], desc: &mut [MemoryRegionDesc]) {
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => {
                    ep.recvv_from(iov, desc, self.mapped_addr.as_ref().unwrap())
                }
                MyEndpoint::ConnectedMrLocal(ep) => ep.recvv(iov, desc),
                _ => panic!("Only mr data"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn recv<T>(&mut self, buf: &mut [T], desc: &mut MemoryRegionDesc) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => {
                    ep.recv_from(buf, desc, self.mapped_addr.as_ref().unwrap())
                }
                MyEndpoint::Connected(ep) => ep.recv(buf, desc),
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn recv_mr<T: Copy>(
        &mut self,
        buf: &mut MemoryRegionSlice<T>,
        desc: &mut MemoryRegionDesc,
    ) {
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => {
                    ep.recv_from(buf, desc, self.mapped_addr.as_ref().unwrap())
                }
                MyEndpoint::ConnectedMrLocal(ep) => ep.recv(buf, desc),
                _ => panic!("MR data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn sendmsg(&mut self, msg: &MsgType<Msg, MsgConnected, MsgMr, MsgConnectedMr>) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => match msg {
                    MsgType::ConnectionlessMsg(msg) => {
                        ep.sendmsg_to(msg, TferOptions::new().remote_cq_data())
                    }
                    MsgType::ConnectedMsg(_) => panic!("Wrong msg type"),
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::Connected(ep) => match msg {
                    MsgType::ConnectionlessMsg(_) => panic!("Wrong msg type"),
                    MsgType::ConnectedMsg(msg) => {
                        ep.sendmsg(msg, TferOptions::new().remote_cq_data())
                    }
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::ConnectionlessMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(msg) => {
                        ep.sendmsg_to(msg, TferOptions::new().remote_cq_data())
                    }
                    MsgType::ConnectedMrMsg(_) => panic!("Wrong msg type"),
                    _ => panic!("Mr data only"),
                },
                MyEndpoint::ConnectedMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(_) => panic!("Wrong msg type"),
                    MsgType::ConnectedMrMsg(msg) => {
                        ep.sendmsg(msg, TferOptions::new().remote_cq_data())
                    }
                    _ => panic!("Mr data only"),
                },
            };

            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn recvmsg(&mut self, msg: &MsgType<MsgMut, MsgConnectedMut, MsgMutMr, MsgConnectedMutMr>) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => match msg {
                    MsgType::ConnectionlessMsg(msg) => ep.recvmsg_from(msg, TferOptions::new()),
                    MsgType::ConnectedMsg(_) => panic!("Wrong message type"),
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::Connected(ep) => match msg {
                    MsgType::ConnectionlessMsg(_) => panic!("Wrong message type"),
                    MsgType::ConnectedMsg(msg) => ep.recvmsg(msg, TferOptions::new()),
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::ConnectionlessMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(msg) => ep.recvmsg_from(msg, TferOptions::new()),
                    MsgType::ConnectedMrMsg(_) => panic!("Wrong message type"),
                    _ => panic!("Mr data only"),
                },
                MyEndpoint::ConnectedMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(_) => panic!("Wrong message type"),
                    MsgType::ConnectedMrMsg(msg) => ep.recvmsg(msg, TferOptions::new()),
                    _ => panic!("Mr data only"),
                },
            };

            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn exchange_keys(&mut self, key: MemoryRegionKey, addr: usize, len: usize) {
        let mut len = unsafe {
            std::slice::from_raw_parts(
                &len as *const usize as *const u8,
                std::mem::size_of::<usize>(),
            )
        }
        .to_vec();
        let mut addr = unsafe {
            std::slice::from_raw_parts(
                &addr as *const usize as *const u8,
                std::mem::size_of::<usize>(),
            )
        }
        .to_vec();

        let key_bytes = key.to_bytes();
        let mut reg_mem = Vec::new();
        reg_mem.append(&mut key_bytes.clone());
        reg_mem.append(&mut len);
        reg_mem.append(&mut addr);
        let total_len = reg_mem.len();
        reg_mem.append(&mut vec![0; total_len]);

        let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
            .access_recv()
            .access_send()
            .build(&self.domain)
            .unwrap();

        let mr = match mr {
            libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
            libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
                bind_mr(&self.ep, &mr);
                mr.enable().unwrap()
            }
        };

        let mut desc = mr.description();
        match self.ep {
            MyEndpoint::Connected(_) => {
                self.send(
                    &reg_mem[..key_bytes.len() + 2 * std::mem::size_of::<usize>()],
                    &mut desc,
                    None,
                );
                self.recv(
                    &mut reg_mem[key_bytes.len() + 2 * std::mem::size_of::<usize>()
                        ..2 * key_bytes.len() + 4 * std::mem::size_of::<usize>()],
                    &mut desc,
                );
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                self.send_mr(
                    &mr.slice(0)
                        .slice(..key_bytes.len() + 2 * std::mem::size_of::<usize>()),
                    &mut desc,
                    None,
                );
                self.recv_mr(
                    &mut mr.slice(0).slice(
                        key_bytes.len() + 2 * std::mem::size_of::<usize>()
                            ..2 * key_bytes.len() + 4 * std::mem::size_of::<usize>(),
                    ),
                    &mut desc,
                );
            }
            MyEndpoint::Connectionless(_) => {
                self.send(
                    &reg_mem[..key_bytes.len() + 2 * std::mem::size_of::<usize>()],
                    &mut desc,
                    None,
                );
                self.recv(
                    &mut reg_mem[key_bytes.len() + 2 * std::mem::size_of::<usize>()
                        ..2 * key_bytes.len() + 4 * std::mem::size_of::<usize>()],
                    &mut desc,
                );
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                self.send_mr(
                    &mr.slice(0)
                        .slice(..key_bytes.len() + 2 * std::mem::size_of::<usize>()),
                    &mut desc,
                    None,
                );
                self.recv_mr(
                    &mut mr.slice(0).slice(
                        key_bytes.len() + 2 * std::mem::size_of::<usize>()
                            ..2 * key_bytes.len() + 4 * std::mem::size_of::<usize>(),
                    ),
                    &mut desc,
                );
            }
        }
        // self.send(
        //     &reg_mem[..key_bytes.len() + 2 * std::mem::size_of::<usize>()],
        //     &mut desc,
        //     None,
        // );
        // self.recv(
        //     &mut reg_mem[key_bytes.len() + 2 * std::mem::size_of::<usize>()
        //         ..2 * key_bytes.len() + 4 * std::mem::size_of::<usize>()],
        //     &mut desc,
        // );

        self.cq_type.rx_cq().sread(1, -1).unwrap();
        let remote_key = unsafe {
            MemoryRegionKey::from_bytes(
                &reg_mem[key_bytes.len() + 2 * std::mem::size_of::<usize>()
                    ..2 * key_bytes.len() + 2 * std::mem::size_of::<usize>()],
                &self.domain,
            )
        }
        .into_mapped(&self.domain)
        .unwrap();
        let len = unsafe {
            std::slice::from_raw_parts(
                reg_mem[2 * key_bytes.len() + 2 * std::mem::size_of::<usize>()
                    ..2 * key_bytes.len() + 3 * std::mem::size_of::<usize>()]
                    .as_ptr() as *const u8 as *const u64,
                1,
            )
        }[0];
        let addr = unsafe {
            std::slice::from_raw_parts(
                reg_mem[2 * key_bytes.len() + 3 * std::mem::size_of::<usize>()
                    ..2 * key_bytes.len() + 4 * std::mem::size_of::<usize>()]
                    .as_ptr() as *const u8 as *const u64,
                1,
            )
        }[0];
        self.remote_key = Some(remote_key);
        self.remote_mem_addr = Some((addr, addr + len));
    }
}

impl<I: MsgDefaultCap + RmaDefaultCap> Ofi<I> {
    pub fn write<T>(
        &mut self,
        buf: &[T],
        dest_addr: u64,
        desc: &mut MemoryRegionDesc,
        data: Option<u64>,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            unsafe {
                                ep.inject_writedata_to(
                                    buf,
                                    data.unwrap(),
                                    self.mapped_addr.as_ref().unwrap(),
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        } else {
                            unsafe {
                                ep.inject_write_to(
                                    buf,
                                    self.mapped_addr.as_ref().unwrap(),
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        }
                    } else {
                        if data.is_some() {
                            unsafe {
                                ep.writedata_to(
                                    buf,
                                    desc,
                                    data.unwrap(),
                                    self.mapped_addr.as_ref().unwrap(),
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        } else {
                            unsafe {
                                ep.write_to(
                                    buf,
                                    desc,
                                    self.mapped_addr.as_ref().unwrap(),
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        }
                    }
                }
                MyEndpoint::Connected(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            unsafe {
                                ep.inject_writedata(
                                    buf,
                                    data.unwrap(),
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        } else {
                            unsafe {
                                ep.inject_write(
                                    buf,
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        }
                    } else {
                        if data.is_some() {
                            unsafe {
                                ep.writedata(
                                    buf,
                                    desc,
                                    data.unwrap(),
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        } else {
                            unsafe {
                                ep.write(
                                    buf,
                                    desc,
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        }
                    }
                }
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn write_mr<T: Copy>(
        &mut self,
        buf: &MemoryRegionSlice<T>,
        dest_addr: u64,
        desc: &mut MemoryRegionDesc,
        data: Option<u64>,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            unsafe {
                                ep.inject_writedata_to(
                                    buf,
                                    data.unwrap(),
                                    self.mapped_addr.as_ref().unwrap(),
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        } else {
                            unsafe {
                                ep.inject_write_to(
                                    buf,
                                    self.mapped_addr.as_ref().unwrap(),
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        }
                    } else {
                        if data.is_some() {
                            unsafe {
                                ep.writedata_to(
                                    buf,
                                    desc,
                                    data.unwrap(),
                                    self.mapped_addr.as_ref().unwrap(),
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        } else {
                            unsafe {
                                ep.write_to(
                                    buf,
                                    desc,
                                    self.mapped_addr.as_ref().unwrap(),
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        }
                    }
                }
                MyEndpoint::ConnectedMrLocal(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        if data.is_some() {
                            unsafe {
                                ep.inject_writedata(
                                    buf,
                                    data.unwrap(),
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        } else {
                            unsafe {
                                ep.inject_write(
                                    buf,
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        }
                    } else {
                        if data.is_some() {
                            unsafe {
                                ep.writedata(
                                    buf,
                                    desc,
                                    data.unwrap(),
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        } else {
                            unsafe {
                                ep.write(
                                    buf,
                                    desc,
                                    start + dest_addr,
                                    self.remote_key.as_ref().unwrap(),
                                )
                            }
                        }
                    }
                }
                _ => panic!("Mr data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn read<T>(&mut self, buf: &mut [T], dest_addr: u64, desc: &mut MemoryRegionDesc) {
        let (start, _end) = self.remote_mem_addr.unwrap();

        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => unsafe {
                    ep.read_from(
                        buf,
                        desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                MyEndpoint::Connected(ep) => unsafe {
                    ep.read(
                        buf,
                        desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn read_mr<T: Copy>(
        &mut self,
        buf: &mut MemoryRegionSlice<T>,
        dest_addr: u64,
        desc: &mut MemoryRegionDesc,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();

        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => unsafe {
                    ep.read_from(
                        buf,
                        desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                MyEndpoint::ConnectedMrLocal(ep) => unsafe {
                    ep.read(
                        buf,
                        desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                _ => panic!("Mr data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn writev(&mut self, iov: &[IoVec], dest_addr: u64, desc: &mut [MemoryRegionDesc]) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => unsafe {
                    ep.writev_to(
                        iov,
                        desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                MyEndpoint::Connected(ep) => unsafe {
                    ep.writev(
                        iov,
                        desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn writev_mr(&mut self, iov: &[IoVecMr], dest_addr: u64, desc: &mut [MemoryRegionDesc]) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => unsafe {
                    ep.writev_to(
                        iov,
                        desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                MyEndpoint::ConnectedMrLocal(ep) => unsafe {
                    ep.writev(
                        iov,
                        desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                _ => panic!("Mr data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn readv(&mut self, iov: &[IoVecMut], dest_addr: u64, desc: &mut [MemoryRegionDesc]) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => unsafe {
                    ep.readv_from(
                        iov,
                        desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                MyEndpoint::Connected(ep) => unsafe {
                    ep.readv(
                        iov,
                        desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn readv_mr(&mut self, iov: &[IoVecMutMr], dest_addr: u64, desc: &mut [MemoryRegionDesc]) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => unsafe {
                    ep.readv_from(
                        iov,
                        desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                MyEndpoint::ConnectedMrLocal(ep) => unsafe {
                    ep.readv(
                        iov,
                        desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                _ => panic!("Mr data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    // [TODO] Enabling .remote_cq_data causes the buffer not being written correctly
    // on the remote side.
    pub fn writemsg(
        &mut self,
        msg: &MsgType<MsgRma, MsgRmaConnected, MsgRmaMr, MsgRmaConnectedMr>,
    ) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => match msg {
                    MsgType::ConnectionlessMsg(msg) => unsafe {
                        ep.writemsg_to(msg, WriteMsgOptions::new())
                    },
                    MsgType::ConnectedMsg(_) => panic!("Wrong message type"),
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::Connected(ep) => match msg {
                    MsgType::ConnectionlessMsg(_) => panic!("Wrong message type"),
                    MsgType::ConnectedMsg(msg) => unsafe {
                        ep.writemsg(msg, WriteMsgOptions::new())
                    },
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::ConnectionlessMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(msg) => unsafe {
                        ep.writemsg_to(msg, WriteMsgOptions::new())
                    },
                    MsgType::ConnectedMrMsg(_) => panic!("Wrong message type"),
                    _ => panic!("Mr data only"),
                },
                MyEndpoint::ConnectedMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(_) => panic!("Wrong message type"),
                    MsgType::ConnectedMrMsg(msg) => unsafe {
                        ep.writemsg(msg, WriteMsgOptions::new())
                    },
                    _ => panic!("Mr data only"),
                },
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn readmsg(
        &mut self,
        msg: &MsgType<MsgRmaMut, MsgRmaConnectedMut, MsgRmaMutMr, MsgRmaConnectedMutMr>,
    ) {
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => match msg {
                    MsgType::ConnectionlessMsg(msg) => unsafe {
                        ep.readmsg_from(msg, ReadMsgOptions::new())
                    },
                    MsgType::ConnectedMsg(_) => todo!(),
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::Connected(ep) => match msg {
                    MsgType::ConnectionlessMsg(_) => panic!("Wrong message type"),
                    MsgType::ConnectedMsg(msg) => unsafe { ep.readmsg(msg, ReadMsgOptions::new()) },
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::ConnectionlessMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(msg) => unsafe {
                        ep.readmsg_from(msg, ReadMsgOptions::new())
                    },
                    MsgType::ConnectedMrMsg(_) => todo!(),
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::ConnectedMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(_) => panic!("Wrong message type"),
                    MsgType::ConnectedMrMsg(msg) => unsafe {
                        ep.readmsg(msg, ReadMsgOptions::new())
                    },
                    _ => panic!("Plain data only"),
                },
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }
}

impl<I: AtomicDefaultCap> Ofi<I> {
    pub fn atomic<T: libfabric::AsFiType>(
        &mut self,
        buf: &[T],
        dest_addr: u64,
        desc: &mut MemoryRegionDesc,
        op: AtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        unsafe {
                            ep.inject_atomic_to(
                                buf,
                                self.mapped_addr.as_ref().unwrap(),
                                start + dest_addr,
                                self.remote_key.as_ref().unwrap(),
                                op,
                            )
                        }
                    } else {
                        unsafe {
                            ep.atomic_to(
                                buf,
                                desc,
                                self.mapped_addr.as_ref().unwrap(),
                                start + dest_addr,
                                self.remote_key.as_ref().unwrap(),
                                op,
                            )
                        }
                    }
                }
                MyEndpoint::Connected(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        unsafe {
                            ep.inject_atomic(
                                buf,
                                start + dest_addr,
                                self.remote_key.as_ref().unwrap(),
                                op,
                            )
                        }
                    } else {
                        unsafe {
                            ep.atomic(
                                buf,
                                desc,
                                start + dest_addr,
                                self.remote_key.as_ref().unwrap(),
                                op,
                            )
                        }
                    }
                }
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn atomic_mr<T: libfabric::AsFiType + Copy>(
        &mut self,
        buf: &MemoryRegionSlice<T>,
        dest_addr: u64,
        desc: &mut MemoryRegionDesc,
        op: AtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        unsafe {
                            ep.inject_atomic_to(
                                buf,
                                self.mapped_addr.as_ref().unwrap(),
                                start + dest_addr,
                                self.remote_key.as_ref().unwrap(),
                                op,
                            )
                        }
                    } else {
                        unsafe {
                            ep.atomic_to(
                                buf,
                                desc,
                                self.mapped_addr.as_ref().unwrap(),
                                start + dest_addr,
                                self.remote_key.as_ref().unwrap(),
                                op,
                            )
                        }
                    }
                }
                MyEndpoint::ConnectedMrLocal(ep) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        unsafe {
                            ep.inject_atomic(
                                buf,
                                start + dest_addr,
                                self.remote_key.as_ref().unwrap(),
                                op,
                            )
                        }
                    } else {
                        unsafe {
                            ep.atomic(
                                buf,
                                desc,
                                start + dest_addr,
                                self.remote_key.as_ref().unwrap(),
                                op,
                            )
                        }
                    }
                }
                _ => panic!("Mr data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn atomicv<T: libfabric::AsFiType>(
        &mut self,
        ioc: &[libfabric::iovec::Ioc<T>],
        dest_addr: u64,
        desc: &mut [MemoryRegionDesc],
        op: AtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => unsafe {
                    ep.atomicv_to(
                        ioc,
                        desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MyEndpoint::Connected(ep) => unsafe {
                    ep.atomicv(
                        ioc,
                        desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn atomicv_mr<T: libfabric::AsFiType + Copy>(
        &mut self,
        ioc: &[libfabric::iovec::IocMr<T>],
        dest_addr: u64,
        desc: &mut [MemoryRegionDesc],
        op: AtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => unsafe {
                    ep.atomicv_to(
                        ioc,
                        desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MyEndpoint::ConnectedMrLocal(ep) => unsafe {
                    ep.atomicv(
                        ioc,
                        desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                _ => panic!("Mr data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn atomicmsg<T: libfabric::AsFiType + std::marker::Copy>(
        &mut self,
        msg: &MsgType<MsgAtomic<T>, MsgAtomicConnected<T>, MsgAtomicMr<T>, MsgAtomicConnectedMr<T>>,
    ) {
        let opts = AtomicMsgOptions::new();
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => match msg {
                    MsgType::ConnectionlessMsg(msg) => unsafe { ep.atomicmsg_to(msg, opts) },
                    MsgType::ConnectedMsg(_) => todo!(),
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::Connected(ep) => match msg {
                    MsgType::ConnectionlessMsg(_) => todo!(),
                    MsgType::ConnectedMsg(msg) => unsafe { ep.atomicmsg(msg, opts) },
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::ConnectionlessMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(msg) => unsafe { ep.atomicmsg_to(msg, opts) },
                    MsgType::ConnectedMsg(_) => todo!(),
                    _ => panic!("Mr data only"),
                },
                MyEndpoint::ConnectedMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(_) => todo!(),
                    MsgType::ConnectedMrMsg(msg) => unsafe { ep.atomicmsg(msg, opts) },
                    _ => panic!("Mr data only"),
                },
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn fetch_atomic<T: libfabric::AsFiType>(
        &mut self,
        buf: &[T],
        res: &mut [T],
        dest_addr: u64,
        desc: &mut MemoryRegionDesc,
        res_desc: &mut MemoryRegionDesc,
        op: FetchAtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => unsafe {
                    ep.fetch_atomic_from(
                        buf,
                        desc,
                        res,
                        res_desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MyEndpoint::Connected(ep) => unsafe {
                    ep.fetch_atomic(
                        buf,
                        desc,
                        res,
                        res_desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn fetch_atomic_mr<T: libfabric::AsFiType + Copy>(
        &mut self,
        buf: &MemoryRegionSlice<T>,
        res: &mut MemoryRegionSlice<T>,
        dest_addr: u64,
        desc: &mut MemoryRegionDesc,
        res_desc: &mut MemoryRegionDesc,
        op: FetchAtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => unsafe {
                    ep.fetch_atomic_from(
                        buf,
                        desc,
                        res,
                        res_desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MyEndpoint::ConnectedMrLocal(ep) => unsafe {
                    ep.fetch_atomic(
                        buf,
                        desc,
                        res,
                        res_desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn fetch_atomicv<T: libfabric::AsFiType>(
        &mut self,
        ioc: &[libfabric::iovec::Ioc<T>],
        res_ioc: &mut [libfabric::iovec::IocMut<T>],
        dest_addr: u64,
        desc: &mut [MemoryRegionDesc],
        res_desc: &mut [MemoryRegionDesc],
        op: FetchAtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => unsafe {
                    ep.fetch_atomicv_from(
                        ioc,
                        desc,
                        res_ioc,
                        res_desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MyEndpoint::Connected(ep) => unsafe {
                    ep.fetch_atomicv(
                        ioc,
                        desc,
                        res_ioc,
                        res_desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn fetch_atomicv_mr<T: libfabric::AsFiType + Copy>(
        &mut self,
        ioc: &[libfabric::iovec::IocMr<T>],
        res_ioc: &mut [libfabric::iovec::IocMutMr<T>],
        dest_addr: u64,
        desc: &mut [MemoryRegionDesc],
        res_desc: &mut [MemoryRegionDesc],
        op: FetchAtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => unsafe {
                    ep.fetch_atomicv_from(
                        ioc,
                        desc,
                        res_ioc,
                        res_desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MyEndpoint::ConnectedMrLocal(ep) => unsafe {
                    ep.fetch_atomicv(
                        ioc,
                        desc,
                        res_ioc,
                        res_desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                _ => panic!("Mr data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn fetch_atomicmsg<T: libfabric::AsFiType>(
        &mut self,
        msg: &MsgType<
            MsgFetchAtomic<T>,
            MsgFetchAtomicConnected<T>,
            MsgFetchAtomicMr<T>,
            MsgFetchAtomicConnectedMr<T>,
        >,
        res_ioc: &mut [libfabric::iovec::IocMut<T>],
        res_desc: &mut [MemoryRegionDesc],
    ) {
        let opts = AtomicMsgOptions::new();
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => match msg {
                    MsgType::ConnectionlessMsg(msg) => unsafe {
                        ep.fetch_atomicmsg_from(msg, res_ioc, res_desc, opts)
                    },
                    MsgType::ConnectedMsg(_) => todo!(),
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::Connected(ep) => match msg {
                    MsgType::ConnectionlessMsg(_) => todo!(),
                    MsgType::ConnectedMsg(msg) => unsafe {
                        ep.fetch_atomicmsg(msg, res_ioc, res_desc, opts)
                    },
                    _ => panic!("Plain data only"),
                },
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn fetch_atomicmsg_mr<T: libfabric::AsFiType + Copy>(
        &mut self,
        msg: &MsgType<
            MsgFetchAtomic<T>,
            MsgFetchAtomicConnected<T>,
            MsgFetchAtomicMr<T>,
            MsgFetchAtomicConnectedMr<T>,
        >,
        res_ioc: &mut [libfabric::iovec::IocMutMr<T>],
        res_desc: &mut [MemoryRegionDesc],
    ) {
        let opts = AtomicMsgOptions::new();
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(msg) => unsafe {
                        ep.fetch_atomicmsg_from(msg, res_ioc, res_desc, opts)
                    },
                    MsgType::ConnectedMrMsg(_) => todo!(),
                    _ => panic!("Mr data only"),
                },
                MyEndpoint::ConnectedMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(_) => todo!(),
                    MsgType::ConnectedMrMsg(msg) => unsafe {
                        ep.fetch_atomicmsg(msg, res_ioc, res_desc, opts)
                    },
                    _ => panic!("Mr data only"),
                },
                _ => panic!("Mr data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }
    pub fn compare_atomic<T: libfabric::AsFiType>(
        &mut self,
        buf: &[T],
        comp: &[T],
        res: &mut [T],
        dest_addr: u64,
        desc: &mut MemoryRegionDesc,
        comp_desc: &mut MemoryRegionDesc,
        res_desc: &mut MemoryRegionDesc,
        op: CompareAtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => unsafe {
                    ep.compare_atomic_to(
                        buf,
                        desc,
                        comp,
                        comp_desc,
                        res,
                        res_desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MyEndpoint::Connected(ep) => unsafe {
                    ep.compare_atomic(
                        buf,
                        desc,
                        comp,
                        comp_desc,
                        res,
                        res_desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }
    pub fn compare_atomic_mr<T: libfabric::AsFiType + Copy>(
        &mut self,
        buf: &MemoryRegionSlice<T>,
        comp: &MemoryRegionSlice<T>,
        res: &mut MemoryRegionSlice<T>,
        dest_addr: u64,
        desc: &mut MemoryRegionDesc,
        comp_desc: &mut MemoryRegionDesc,
        res_desc: &mut MemoryRegionDesc,
        op: CompareAtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => unsafe {
                    ep.compare_atomic_to(
                        buf,
                        desc,
                        comp,
                        comp_desc,
                        res,
                        res_desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MyEndpoint::ConnectedMrLocal(ep) => unsafe {
                    ep.compare_atomic(
                        buf,
                        desc,
                        comp,
                        comp_desc,
                        res,
                        res_desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                _ => panic!("Mr data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn compare_atomicv<T: libfabric::AsFiType>(
        &mut self,
        ioc: &[libfabric::iovec::Ioc<T>],
        comp_ioc: &[libfabric::iovec::Ioc<T>],
        res_ioc: &mut [libfabric::iovec::IocMut<T>],
        dest_addr: u64,
        desc: &mut [MemoryRegionDesc],
        comp_desc: &mut [MemoryRegionDesc],
        res_desc: &mut [MemoryRegionDesc],
        op: CompareAtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => unsafe {
                    ep.compare_atomicv_to(
                        ioc,
                        desc,
                        comp_ioc,
                        comp_desc,
                        res_ioc,
                        res_desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MyEndpoint::Connected(ep) => unsafe {
                    ep.compare_atomicv(
                        ioc,
                        desc,
                        comp_ioc,
                        comp_desc,
                        res_ioc,
                        res_desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn compare_atomicv_mr<T: libfabric::AsFiType + Copy>(
        &mut self,
        ioc: &[libfabric::iovec::IocMr<T>],
        comp_ioc: &[libfabric::iovec::IocMr<T>],
        res_ioc: &mut [libfabric::iovec::IocMutMr<T>],
        dest_addr: u64,
        desc: &mut [MemoryRegionDesc],
        comp_desc: &mut [MemoryRegionDesc],
        res_desc: &mut [MemoryRegionDesc],
        op: CompareAtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => unsafe {
                    ep.compare_atomicv_to(
                        ioc,
                        desc,
                        comp_ioc,
                        comp_desc,
                        res_ioc,
                        res_desc,
                        self.mapped_addr.as_ref().unwrap(),
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MyEndpoint::ConnectedMrLocal(ep) => unsafe {
                    ep.compare_atomicv(
                        ioc,
                        desc,
                        comp_ioc,
                        comp_desc,
                        res_ioc,
                        res_desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn compare_atomicmsg<T: libfabric::AsFiType>(
        &mut self,
        msg: &MsgType<
            MsgCompareAtomic<T>,
            MsgCompareAtomicConnected<T>,
            MsgCompareAtomicMr<T>,
            MsgCompareAtomicConnectedMr<T>,
        >,
        comp_ioc: &[libfabric::iovec::Ioc<T>],
        res_ioc: &mut [libfabric::iovec::IocMut<T>],
        comp_desc: &mut [MemoryRegionDesc],
        res_desc: &mut [MemoryRegionDesc],
    ) {
        let opts = AtomicMsgOptions::new();
        loop {
            let err = match &self.ep {
                MyEndpoint::Connectionless(ep) => match msg {
                    MsgType::ConnectionlessMsg(msg) => unsafe {
                        ep.compare_atomicmsg_to(msg, comp_ioc, comp_desc, res_ioc, res_desc, opts)
                    },
                    MsgType::ConnectedMsg(_) => todo!(),
                    _ => panic!("Plain data only"),
                },
                MyEndpoint::Connected(ep) => match msg {
                    MsgType::ConnectionlessMsg(_) => todo!(),
                    MsgType::ConnectedMsg(msg) => unsafe {
                        ep.compare_atomicmsg(msg, comp_ioc, comp_desc, res_ioc, res_desc, opts)
                    },
                    _ => panic!("Plain data only"),
                },
                _ => panic!("Plain data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }

    pub fn compare_atomicmsg_mr<T: libfabric::AsFiType + Copy>(
        &mut self,
        msg: &MsgType<
            MsgCompareAtomic<T>,
            MsgCompareAtomicConnected<T>,
            MsgCompareAtomicMr<T>,
            MsgCompareAtomicConnectedMr<T>,
        >,
        comp_ioc: &[libfabric::iovec::IocMr<T>],
        res_ioc: &mut [libfabric::iovec::IocMutMr<T>],
        comp_desc: &mut [MemoryRegionDesc],
        res_desc: &mut [MemoryRegionDesc],
    ) {
        let opts = AtomicMsgOptions::new();
        loop {
            let err = match &self.ep {
                MyEndpoint::ConnectionlessMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(msg) => unsafe {
                        ep.compare_atomicmsg_to(msg, comp_ioc, comp_desc, res_ioc, res_desc, opts)
                    },
                    MsgType::ConnectedMsg(_) => todo!(),
                    _ => panic!("Mr data only"),
                },
                MyEndpoint::ConnectedMrLocal(ep) => match msg {
                    MsgType::ConnectionlessMrMsg(_) => todo!(),
                    MsgType::ConnectedMrMsg(msg) => unsafe {
                        ep.compare_atomicmsg(msg, comp_ioc, comp_desc, res_ioc, res_desc, opts)
                    },
                    _ => panic!("Mr data only"),
                },
                _ => panic!("Mr data only"),
            };
            match err {
                Ok(_) => break,
                Err(err) => {
                    if !matches!(err.kind, ErrorKind::TryAgain) {
                        panic!("{:?}", err);
                    }
                }
            }

            ft_progress(self.cq_type.tx_cq());
            ft_progress(self.cq_type.rx_cq());
        }
    }
}

impl<I: CollCap> Ofi<I> {}

macro_rules! gen_info {
    ($ep_type: ident, $caps: ident, $shared_cq: literal, $ip: expr, $server: ident, $name: ident) => {
        Ofi::new(
            {
                let info = Info::new(&Version {
                    major: 1,
                    minor: 19,
                })
                .enter_hints()
                .enter_ep_attr()
                .type_($ep_type)
                .leave_ep_attr()
                .enter_domain_attr()
                .threading(libfabric::enums::Threading::Domain)
                .mr_mode(
                    libfabric::enums::MrMode::new()
                        .prov_key()
                        .allocated()
                        .virt_addr()
                        .local()
                        .endpoint()
                        .raw(),
                )
                .leave_domain_attr()
                .enter_tx_attr()
                .traffic_class(libfabric::enums::TrafficClass::LowLatency)
                .leave_tx_attr()
                .addr_format(libfabric::enums::AddressFormat::Unspec)
                .caps($caps)
                .leave_hints();
                if $server {
                    info.source(libfabric::info::ServiceAddress::Service("9222".to_owned()))
                        .get()
                        .unwrap()
                        .into_iter()
                        .next()
                        .unwrap()
                } else {
                    info.node($ip)
                        .service("9222")
                        .get()
                        .unwrap()
                        .into_iter()
                        .next()
                        .unwrap()
                }
            },
            $shared_cq,
            $server,
            $name,
        )
        .unwrap()
    };
}

fn handshake<I: Caps + MsgDefaultCap + 'static>(
    server: bool,
    name: &str,
    caps: Option<I>,
) -> Ofi<I> {
    let caps = caps.unwrap();
    let ep_type = EndpointType::Msg;
    let hostname = std::process::Command::new("hostname")
        .output()
        .expect("Failed to execute hostname")
        .stdout;
    let hostname = String::from_utf8(hostname[2..].to_vec()).unwrap();
    let ip = "172.17.110.".to_string() + &hostname;

    gen_info!(
        ep_type,
        caps,
        false,
        ip.strip_suffix("\n").unwrap_or(&ip),
        server,
        name
    )
}

#[test]
fn handshake_connected0() {
    handshake(true, "handshake_connected0", Some(InfoCaps::new().msg()));
}

#[test]
fn handshake_connected1() {
    handshake(false, "handshake_connected0", Some(InfoCaps::new().msg()));
}

fn handshake_connectionless<I: MsgDefaultCap + Caps + 'static>(
    server: bool,
    name: &str,
    caps: Option<I>,
) -> Ofi<I> {
    let caps = caps.unwrap();
    let ep_type = EndpointType::Rdm;
    let hostname = std::process::Command::new("hostname")
        .output()
        .expect("Failed to execute hostname")
        .stdout;
    let hostname = String::from_utf8(hostname[2..].to_vec()).unwrap();
    let ip = "172.17.110.".to_string() + &hostname;

    gen_info!(
        ep_type,
        caps,
        false,
        ip.strip_suffix("\n").unwrap_or(&ip),
        server,
        name
    )
}

#[test]
fn handshake_connectionless0() {
    handshake_connectionless(
        true,
        "handshake_connectionless0",
        Some(InfoCaps::new().msg()),
    );
}

#[test]
fn handshake_connectionless1() {
    handshake_connectionless(
        false,
        "handshake_connectionless0",
        Some(InfoCaps::new().msg()),
    );
}

fn sendrecv(server: bool, name: &str, connected: bool) {
    let mut ofi = if connected {
        handshake(server, name, Some(InfoCaps::new().msg()))
    } else {
        handshake_connectionless(server, name, Some(InfoCaps::new().msg()))
    };

    let mut reg_mem: Vec<_> = (0..1024 * 2)
        .into_iter()
        .map(|v: usize| (v % 256) as u8)
        .collect();
    let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
        .access_recv()
        .access_send()
        .build(&ofi.domain)
        .unwrap();

    let mr = match mr {
        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
            bind_mr(&ofi.ep, &mr);
            mr.enable().unwrap()
        }
    };

    let mut desc = [mr.description(), mr.description()];
    let mut ctx = ofi.info_entry.allocate_context();

    if server {
        // Send a single buffer
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.send_with_context(&reg_mem[..512], &mut desc[0], None, &mut ctx)
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr_with_context(&mr.slice(0).slice(..512), &mut desc[0], None, &mut ctx);
            }
            MyEndpoint::Connectionless(_) => {
                ofi.send_with_context(&reg_mem[..512], &mut desc[0], None, &mut ctx)
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr_with_context(&mr.slice(0).slice(..512), &mut desc[0], None, &mut ctx);
            }
        }

        let completion = ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        match completion {
            Completion::Data(entry) => {
                assert!(entry[0].is_op_context_equal(&ctx))
            }
            _ => panic!("unexpected completion type"),
        }

        assert!(std::mem::size_of_val(&reg_mem[..128]) <= ofi.info_entry.tx_attr().inject_size());

        // Inject a buffer
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[..128], &mut desc[0], None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(..128), &mut desc[0], None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[..128], &mut desc[0], None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(..128), &mut desc[0], None)
            }
        }
        // ofi.send(&reg_mem[..128], &mut desc[0], None);
        // No cq.sread since inject does not generate completions

        // // Send single Iov
        let iov = [IoVec::from_slice(&reg_mem[..512])];
        let mem_mrs = (mr.slice(0).slice(..512), mr.slice(0).slice(512..1024));
        let iov_mr = [IoVecMr::from(&mem_mrs.0)];

        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.sendv(&iov, &mut desc[..1]),
            MyEndpoint::ConnectedMrLocal(_) => ofi.sendv_mr(&iov_mr, &mut desc[..1]),
            MyEndpoint::Connectionless(_) => ofi.sendv(&iov, &mut desc[..1]),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.sendv_mr(&iov_mr, &mut desc[..1]),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Send multi Iov
        let iov = [
            IoVec::from_slice(&reg_mem[..512]),
            IoVec::from_slice(&reg_mem[512..1024]),
        ];
        // Send multi Iov
        let iov_mr = [IoVecMr::from(&mem_mrs.0), IoVecMr::from(&mem_mrs.1)];
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.sendv(&iov, &mut desc),
            MyEndpoint::ConnectedMrLocal(_) => ofi.sendv_mr(&iov_mr, &mut desc),
            MyEndpoint::Connectionless(_) => ofi.sendv(&iov, &mut desc),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.sendv_mr(&iov_mr, &mut desc),
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    } else {
        let expected: Vec<_> = (0..1024 * 2)
            .into_iter()
            .map(|v: usize| (v % 256) as u8)
            .collect();
        reg_mem.iter_mut().for_each(|v| *v = 0);

        // Receive a single buffer
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[..512], &mut desc[0]),
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[..512], &mut desc[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(..512), &mut desc[0])
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(..512), &mut desc[0])
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(reg_mem[..512], expected[..512]);

        // Receive inject
        reg_mem.iter_mut().for_each(|v| *v = 0);
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[..128], &mut desc[0]),
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[..128], &mut desc[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(..128), &mut desc[0])
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(..128), &mut desc[0])
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(reg_mem[..128], expected[..128]);

        reg_mem.iter_mut().for_each(|v| *v = 0);
        // // Receive into a single Iov
        let mut iov = [IoVecMut::from_slice(&mut reg_mem[..512])];
        let mem_mrs = (
            &mut mr.slice(0).slice(..512),
            &mut mr.slice(0).slice(512..1024),
        );

        let mut iov_mr = [IoVecMutMr::from(mem_mrs.0)];

        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recvv(&mut iov, &mut desc[..1]),
            MyEndpoint::ConnectedMrLocal(_) => ofi.recvv_mr(&mut iov_mr, &mut desc[..1]),
            MyEndpoint::Connectionless(_) => ofi.recvv(&mut iov, &mut desc[..1]),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.recvv_mr(&mut iov_mr, &mut desc[..1]),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(reg_mem[..512], expected[..512]);

        reg_mem.iter_mut().for_each(|v| *v = 0);

        // // Receive into multiple Iovs
        let (mem0, mem1) = reg_mem[..1024].split_at_mut(512);
        let iov = [IoVecMut::from_slice(mem0), IoVecMut::from_slice(mem1)];
        let iov_mr = [IoVecMutMr::from(mem_mrs.0), IoVecMutMr::from(mem_mrs.1)];
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recvv(&iov, &mut desc),
            MyEndpoint::ConnectedMrLocal(_) => ofi.recvv_mr(&iov_mr, &mut desc),
            MyEndpoint::Connectionless(_) => ofi.recvv(&iov, &mut desc),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.recvv_mr(&iov_mr, &mut desc),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();

        assert_eq!(mem0, &expected[..512]);
        assert_eq!(mem1, &expected[512..1024]);
    }
}

#[test]
fn sendrecv0() {
    sendrecv(true, "sendrecv0", false);
}

#[test]
fn sendrecv1() {
    sendrecv(false, "sendrecv0", false);
}

#[test]
fn conn_sendrecv0() {
    sendrecv(true, "conn_sendrecv0", true);
}

#[test]
fn conn_sendrecv1() {
    sendrecv(false, "conn_sendrecv0", true);
}

fn sendrecvdata(server: bool, name: &str, connected: bool) {
    let mut ofi = if connected {
        handshake(server, name, Some(InfoCaps::new().msg()))
    } else {
        handshake_connectionless(server, name, Some(InfoCaps::new().msg()))
    };

    let mut reg_mem: Vec<_> = (0..1024 * 2)
        .into_iter()
        .map(|v: usize| (v % 256) as u8)
        .collect();
    let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
        .access_recv()
        .access_send()
        .build(&ofi.domain)
        .unwrap();

    let mr = match mr {
        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
            bind_mr(&ofi.ep, &mr);
            mr.enable().unwrap()
        }
    };

    let mut desc = [mr.description(), mr.description()];
    let data = Some(128u64);
    if server {
        // Send a single buffer

        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[..512], &mut desc[0], data),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(..512), &mut desc[0], data)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[..512], &mut desc[0], data),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(..512), &mut desc[0], data)
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    } else {
        let expected: Vec<_> = (0..1024 * 2)
            .into_iter()
            .map(|v: usize| (v % 256) as u8)
            .collect();
        reg_mem.iter_mut().for_each(|v| *v = 0);

        // Receive a single buffer
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[..512], &mut desc[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(..512), &mut desc[0])
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[..512], &mut desc[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(..512), &mut desc[0])
            }
        }

        let entry = ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        match entry {
            Completion::Data(entry) => assert_eq!(entry[0].data(), data.unwrap()),
            _ => panic!("Unexpected CQ entry format"),
        }
        assert_eq!(reg_mem[..512], expected[..512]);
    }
}

#[test]
fn sendrecvdata0() {
    sendrecvdata(true, "sendrecvdata0", false);
}

#[test]
fn sendrecvdata1() {
    sendrecvdata(false, "sendrecvdata0", false);
}

#[test]
fn conn_sendrecvdata0() {
    sendrecvdata(true, "conn_sendrecvdata0", true);
}

#[test]
fn conn_sendrecvdata1() {
    sendrecvdata(false, "conn_sendrecvdata0", true);
}

fn bind_mr<E: 'static>(ep: &MyEndpoint<E>, mr: &DisabledMemoryRegion) {
    match ep {
        MyEndpoint::ConnectedMrLocal(ep) => mr.bind_ep(ep).unwrap(),
        MyEndpoint::ConnectionlessMrLocal(ep) => mr.bind_ep(ep).unwrap(),
        MyEndpoint::Connected(ep) => mr.bind_ep(ep).unwrap(),
        MyEndpoint::Connectionless(ep) => mr.bind_ep(ep).unwrap(),
    }
}

fn tsendrecv(server: bool, name: &str, connected: bool) {
    let mut ofi = if connected {
        handshake(server, name, Some(InfoCaps::new().msg().tagged()))
    } else {
        handshake_connectionless(server, name, Some(InfoCaps::new().msg().tagged()))
    };

    let mut reg_mem: Vec<_> = (0..1024 * 2)
        .into_iter()
        .map(|v: usize| (v % 256) as u8)
        .collect();
    let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
        .access_recv()
        .access_send()
        .build(&ofi.domain)
        .unwrap();

    let mr = match mr {
        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
            bind_mr(&ofi.ep, &mr);
            mr.enable().unwrap()
        }
    };

    let mut desc = [mr.description(), mr.description()];
    let data = Some(128u64);

    if server {
        // Send a single buffer
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.tsend(&reg_mem[..512], &mut desc[0], 10, data),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.tsend_mr(&mr.slice(0).slice(..512), &mut desc[0], 10, data)
            }
            MyEndpoint::Connectionless(_) => ofi.tsend(&reg_mem[..512], &mut desc[0], 10, data),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.tsend_mr(&mr.slice(0).slice(..512), &mut desc[0], 10, data)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        // match entry {
        //     Completion::Tagged(entry) => {assert_eq!(entry[0].data(), data.unwrap()); assert_eq!(entry[0].tag(), 10)},
        //     _ => panic!("Unexpected CQ entry format"),
        // }

        assert!(std::mem::size_of_val(&reg_mem[..128]) <= ofi.info_entry.tx_attr().inject_size());

        // Inject a buffer
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.tsend(&reg_mem[..128], &mut desc[0], 1, data),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.tsend_mr(&mr.slice(0).slice(..128), &mut desc[0], 1, data)
            }
            MyEndpoint::Connectionless(_) => ofi.tsend(&reg_mem[..128], &mut desc[0], 1, data),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.tsend_mr(&mr.slice(0).slice(..128), &mut desc[0], 1, data)
            }
        }

        // No cq.sread since inject does not generate completions

        // // Send single Iov
        let iov = [IoVec::from_slice(&reg_mem[..512])];
        let mem_mr0 = mr.slice(0).slice(..512);
        let mem_mr1 = mr.slice(0).slice(512..1024);
        let iov_mr = [IoVecMr::from(&mem_mr0)];
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.tsendv(&iov, &mut desc[..1], 2),
            MyEndpoint::ConnectedMrLocal(_) => ofi.tsendv_mr(&iov_mr, &mut desc[..1], 2),
            MyEndpoint::Connectionless(_) => ofi.tsendv(&iov, &mut desc[..1], 2),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.tsendv_mr(&iov_mr, &mut desc[..1], 2),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Send multi Iov
        let iov = [
            IoVec::from_slice(&reg_mem[..512]),
            IoVec::from_slice(&reg_mem[512..1024]),
        ];
        let iov_mr = [IoVecMr::from(&mem_mr0), IoVecMr::from(&mem_mr1)];

        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.tsendv(&iov, &mut desc, 3),
            MyEndpoint::ConnectedMrLocal(_) => ofi.tsendv_mr(&iov_mr, &mut desc, 3),
            MyEndpoint::Connectionless(_) => ofi.tsendv(&iov, &mut desc, 3),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.tsendv_mr(&iov_mr, &mut desc, 3),
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    } else {
        let expected: Vec<_> = (0..1024 * 2)
            .into_iter()
            .map(|v: usize| (v % 256) as u8)
            .collect();
        reg_mem.iter_mut().for_each(|v| *v = 0);

        // Receive a single buffer
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.trecv(&mut reg_mem[..512], &mut desc[0], 10),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.trecv_mr(&mut mr.slice(0).slice(..512), &mut desc[0], 10)
            }
            MyEndpoint::Connectionless(_) => ofi.trecv(&mut reg_mem[..512], &mut desc[0], 10),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.trecv_mr(&mut mr.slice(0).slice(..512), &mut desc[0], 10)
            }
        }

        let entry = ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        match entry {
            Completion::Tagged(entry) => {
                assert_eq!(entry[0].data(), data.unwrap());
                assert_eq!(entry[0].tag(), 10)
            }
            _ => panic!("Unexpected CQ entry format"),
        }
        assert_eq!(reg_mem[..512], expected[..512]);

        // Receive inject
        reg_mem.iter_mut().for_each(|v| *v = 0);

        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.trecv(&mut reg_mem[..128], &mut desc[0], 1),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.trecv_mr(&mut mr.slice(0).slice(..128), &mut desc[0], 1)
            }
            MyEndpoint::Connectionless(_) => ofi.trecv(&mut reg_mem[..128], &mut desc[0], 1),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.trecv_mr(&mut mr.slice(0).slice(..128), &mut desc[0], 1)
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(reg_mem[..128], expected[..128]);

        reg_mem.iter_mut().for_each(|v| *v = 0);
        // // Receive into a single Iov
        let mut iov = [IoVecMut::from_slice(&mut reg_mem[..512])];
        let mut mem_mr0 = mr.slice(0).slice(..512);
        let mut mem_mr1 = mr.slice(0).slice(512..1024);
        let mut iov_mr = [IoVecMutMr::from(&mut mem_mr0)];

        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.trecvv(&mut iov, &mut desc[..1], 2),
            MyEndpoint::ConnectedMrLocal(_) => ofi.trecvv_mr(&mut iov_mr, &mut desc[..1], 2),
            MyEndpoint::Connectionless(_) => ofi.trecvv(&mut iov, &mut desc[..1], 2),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.trecvv_mr(&mut iov_mr, &mut desc[..1], 2),
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(reg_mem[..512], expected[..512]);

        reg_mem.iter_mut().for_each(|v| *v = 0);

        // // Receive into multiple Iovs
        let (mem0, mem1) = reg_mem[..1024].split_at_mut(512);
        let iov = [IoVecMut::from_slice(mem0), IoVecMut::from_slice(mem1)];
        let iov_mr = [
            IoVecMutMr::from(&mut mem_mr0),
            IoVecMutMr::from(&mut mem_mr1),
        ];
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.trecvv(&iov, &mut desc, 3),
            MyEndpoint::ConnectedMrLocal(_) => ofi.trecvv_mr(&iov_mr, &mut desc, 3),
            MyEndpoint::Connectionless(_) => ofi.trecvv(&iov, &mut desc, 3),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.trecvv_mr(&iov_mr, &mut desc, 3),
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();

        assert_eq!(mem0, &expected[..512]);
        assert_eq!(mem1, &expected[512..1024]);
    }
}

#[test]
fn tsendrecv0() {
    tsendrecv(true, "tsendrecv0", false);
}

#[test]
fn tsendrecv1() {
    tsendrecv(false, "tsendrecv0", false);
}

#[test]
fn conn_tsendrecv0() {
    tsendrecv(true, "conn_tsendrecv0", true);
}

#[test]
fn conn_tsendrecv1() {
    tsendrecv(false, "conn_tsendrecv0", true);
}

fn sendrecvmsg(server: bool, name: &str, connected: bool) {
    let mut ofi = if connected {
        handshake(server, name, Some(InfoCaps::new().msg()))
    } else {
        handshake_connectionless(server, name, Some(InfoCaps::new().msg()))
    };

    let mut reg_mem: Vec<_> = (0..1024 * 2)
        .into_iter()
        .map(|v: usize| (v % 256) as u8)
        .collect();
    let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
        .access_recv()
        .access_send()
        .build(&ofi.domain)
        .unwrap();

    let mr = match mr {
        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
            bind_mr(&ofi.ep, &mr);
            mr.enable().unwrap()
        }
    };

    let desc = mr.description();
    let mut descs = [desc.clone(), desc];
    let mapped_addr = ofi.mapped_addr.clone();

    if server {
        // Single iov message
        let (mem0, mem1) = (&reg_mem[..512], &reg_mem[1024..1536]);
        let (mem_mr0, mem_mr1) = (&mr.slice(0).slice(..512), &mr.slice(0).slice(1024..1536));

        let iov0 = IoVec::from_slice(mem0);
        let iov1 = IoVec::from_slice(mem1);

        let iov_mr0 = IoVecMr::from(mem_mr0);
        let iov_mr1 = IoVecMr::from(mem_mr1);

        let msg =
            match &ofi.ep {
                MyEndpoint::Connected(_) => {
                    MsgType::ConnectedMsg(MsgConnected::from_iov(&iov0, &mut descs[0], 128))
                }
                MyEndpoint::ConnectedMrLocal(_) => {
                    MsgType::ConnectedMrMsg(MsgConnectedMr::from_iov(&iov_mr0, &mut descs[0], 128))
                }
                MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(Msg::from_iov(
                    &iov0,
                    &mut descs[0],
                    mapped_addr.as_ref().unwrap(),
                    128,
                )),
                MyEndpoint::ConnectionlessMrLocal(_) => MsgType::ConnectionlessMrMsg(
                    MsgMr::from_iov(&iov_mr0, &mut descs[0], mapped_addr.as_ref().unwrap(), 128),
                ),
            };

        ofi.sendmsg(&msg);
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        // let entry =
        // match entry {
        //     Completion::Data(entry) => assert_eq!(entry[0].data(), 128),
        //     _ => panic!("Unexpected CQ entry format"),
        // }

        // Multi iov message with stride
        let iovs = [iov0, iov1];
        let iovs_mr = [iov_mr0, iov_mr1];

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => {
                MsgType::ConnectedMsg(MsgConnected::from_iov_slice(&iovs, &mut descs, 128))
            }
            MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                MsgConnectedMr::from_iov_mr_slice(&iovs_mr, &mut descs, 128),
            ),
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(Msg::from_iov_slice(
                &iovs,
                &mut descs,
                mapped_addr.as_ref().unwrap(),
                128,
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => MsgType::ConnectionlessMrMsg(
                MsgMr::from_iov_mr_slice(&iovs_mr, &mut descs, mapped_addr.as_ref().unwrap(), 128),
            ),
        };

        ofi.sendmsg(&msg);
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        // let entry =
        // match entry {
        //     Completion::Data(entry) => assert_eq!(entry[0].data(), 128),
        //     _ => panic!("Unexpected CQ entry format"),
        // }

        // Single iov message
        let msg =
            match &ofi.ep {
                MyEndpoint::Connected(_) => {
                    MsgType::ConnectedMsg(MsgConnected::from_iov(&iovs[0], &mut descs[0], 0))
                }
                MyEndpoint::ConnectedMrLocal(_) => {
                    MsgType::ConnectedMrMsg(MsgConnectedMr::from_iov(&iovs_mr[0], &mut descs[0], 0))
                }
                MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(Msg::from_iov(
                    &iovs[0],
                    &mut descs[0],
                    mapped_addr.as_ref().unwrap(),
                    0,
                )),
                MyEndpoint::ConnectionlessMrLocal(_) => MsgType::ConnectionlessMrMsg(
                    MsgMr::from_iov(&iovs_mr[0], &mut descs[0], mapped_addr.as_ref().unwrap(), 0),
                ),
            };

        ofi.sendmsg(&msg);
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => {
                MsgType::ConnectedMsg(MsgConnected::from_iov_slice(&iovs, &mut descs, 0))
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                MsgType::ConnectedMrMsg(MsgConnectedMr::from_iov_mr_slice(&iovs_mr, &mut descs, 0))
            }
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(Msg::from_iov_slice(
                &iovs,
                &mut descs,
                mapped_addr.as_ref().unwrap(),
                0,
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => MsgType::ConnectionlessMrMsg(
                MsgMr::from_iov_mr_slice(&iovs_mr, &mut descs, mapped_addr.as_ref().unwrap(), 0),
            ),
        };

        ofi.sendmsg(&msg);
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    } else {
        reg_mem.iter_mut().for_each(|v| *v = 0);
        let (mem0, mem1) = reg_mem.split_at_mut(512);
        let (mem_mr0, mut mem_mr1) = (
            &mut mr.slice(0).slice(..512),
            &mut mr.slice(0).slice(512..1536),
        );

        let expected: Vec<_> = (0..1024).map(|v: usize| (v % 256) as u8).collect();

        // Receive a single message in a single buffer
        let mut iov = IoVecMut::from_slice(mem0);
        let mut iov_mr = IoVecMutMr::from(mem_mr0);

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => {
                MsgType::ConnectedMsg(MsgConnectedMut::from_iov(&mut iov, &mut descs[0]))
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                MsgType::ConnectedMrMsg(MsgConnectedMutMr::from_iov(&mut iov_mr, &mut descs[0]))
            }
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgMut::from_iov(
                &mut iov,
                &mut descs[0],
                mapped_addr.as_ref().unwrap(),
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => MsgType::ConnectionlessMrMsg(
                MsgMutMr::from_iov(&mut iov_mr, &mut descs[0], mapped_addr.as_ref().unwrap()),
            ),
        };

        ofi.recvmsg(&msg);
        // ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        let entry = ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        match entry {
            Completion::Data(entry) => assert_eq!(entry[0].data(), 128),
            _ => panic!("Unexpected CQ entry format"),
        }
        assert_eq!(mem0.len(), expected[..512].len());
        assert_eq!(mem0, &expected[..512]);

        // Receive a multi iov message in a single buffer
        let mut iov = IoVecMut::from_slice(&mut mem1[..1024]);
        let mut iov_mr = IoVecMutMr::from(&mut mem_mr1);
        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => {
                MsgType::ConnectedMsg(MsgConnectedMut::from_iov(&mut iov, &mut descs[0]))
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                MsgType::ConnectedMrMsg(MsgConnectedMutMr::from_iov(&mut iov_mr, &mut descs[0]))
            }
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgMut::from_iov(
                &mut iov,
                &mut descs[0],
                mapped_addr.as_ref().unwrap(),
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => MsgType::ConnectionlessMrMsg(
                MsgMutMr::from_iov(&mut iov_mr, &mut descs[0], mapped_addr.as_ref().unwrap()),
            ),
        };

        ofi.recvmsg(&msg);
        let err = ofi.cq_type.rx_cq().sread(1, -1);
        match err {
            Ok(_) => {}
            Err(e) => match e.kind {
                ErrorKind::ErrorAvailable => {
                    let err = ofi.cq_type.rx_cq().readerr(0);
                    err.unwrap();
                }
                _ => todo!(),
            },
        }
        // let entry =
        // match entry {
        //     Completion::Data(entry) => assert_eq!(entry[0].data(), 128),
        //     _ => panic!("Unexpected CQ entry format"),
        // }
        assert_eq!(mem1[..1024], expected);

        // Receive a single iov message into two buffers
        reg_mem.iter_mut().for_each(|v| *v = 0);
        let (mem0, mem1) = reg_mem.split_at_mut(512);
        let (mut mem_mr0, mut mem_mr1) = (
            &mut mr.slice(0).slice(..256),
            &mut mr.slice(0).slice(512..768),
        );
        let iov = IoVecMut::from_slice(&mut mem0[..256]);
        let iov_mr = IoVecMutMr::from(&mut mem_mr0);
        let iov_mr1 = IoVecMutMr::from(&mut mem_mr1);

        let iov1 = IoVecMut::from_slice(&mut mem1[..256]);
        let mut iovs = [iov, iov1];
        let mut iovs_mr = [iov_mr, iov_mr1];

        let msg =
            match &ofi.ep {
                MyEndpoint::Connected(_) => {
                    MsgType::ConnectedMsg(MsgConnectedMut::from_iov_slice(&mut iovs, &mut descs))
                }
                MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                    MsgConnectedMutMr::from_iov_mr_slice(&mut iovs_mr, &mut descs),
                ),
                MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(
                    MsgMut::from_iov_slice(&mut iovs, &mut descs, mapped_addr.as_ref().unwrap()),
                ),
                MyEndpoint::ConnectionlessMrLocal(_) => {
                    MsgType::ConnectionlessMrMsg(MsgMutMr::from_iov_mr_slice(
                        &mut iovs_mr,
                        &mut descs,
                        mapped_addr.as_ref().unwrap(),
                    ))
                }
            };

        ofi.recvmsg(&msg);
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(mem0[..256], expected[..256]);
        assert_eq!(mem1[..256], expected[256..512]);

        // Receive a two iov message into two buffers
        reg_mem.iter_mut().for_each(|v| *v = 0);
        let (mem0, mem1) = reg_mem.split_at_mut(512);
        let iov = IoVecMut::from_slice(&mut mem0[..512]);
        let iov1 = IoVecMut::from_slice(&mut mem1[..512]);
        let (mut mem_mr0, mut mem_mr1) = (
            &mut mr.slice(0).slice(..512),
            &mut mr.slice(0).slice(512..1024),
        );
        let iov_mr = IoVecMutMr::from(&mut mem_mr0);
        let iov_mr1 = IoVecMutMr::from(&mut mem_mr1);
        let mut iovs = [iov, iov1];
        let mut iovs_mr = [iov_mr, iov_mr1];

        let msg =
            match &ofi.ep {
                MyEndpoint::Connected(_) => {
                    MsgType::ConnectedMsg(MsgConnectedMut::from_iov_slice(&mut iovs, &mut descs))
                }
                MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                    MsgConnectedMutMr::from_iov_mr_slice(&mut iovs_mr, &mut descs),
                ),
                MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(
                    MsgMut::from_iov_slice(&mut iovs, &mut descs, mapped_addr.as_ref().unwrap()),
                ),
                MyEndpoint::ConnectionlessMrLocal(_) => {
                    MsgType::ConnectionlessMrMsg(MsgMutMr::from_iov_mr_slice(
                        &mut iovs_mr,
                        &mut descs,
                        mapped_addr.as_ref().unwrap(),
                    ))
                }
            };

        ofi.recvmsg(&msg);
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(mem0[..512], expected[..512]);
        assert_eq!(mem1[..512], expected[512..1024]);
    }
}

#[test]
fn sendrecvmsg0() {
    sendrecvmsg(true, "sendrecvmsg0", false);
}

#[test]
fn sendrecvmsg1() {
    sendrecvmsg(false, "sendrecvmsg0", false);
}

#[test]
fn conn_sendrecvmsg0() {
    sendrecvmsg(true, "conn_sendrecvmsg0", true);
}

#[test]
fn conn_sendrecvmsg1() {
    sendrecvmsg(false, "conn_sendrecvmsg0", true);
}

fn tsendrecvmsg(server: bool, name: &str, connected: bool) {
    let mut ofi = if connected {
        handshake(server, name, Some(InfoCaps::new().msg().tagged()))
    } else {
        handshake_connectionless(server, name, Some(InfoCaps::new().msg().tagged()))
    };

    let mut reg_mem: Vec<_> = (0..1024 * 2)
        .into_iter()
        .map(|v: usize| (v % 256) as u8)
        .collect();
    let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
        .access_recv()
        .access_send()
        .build(&ofi.domain)
        .unwrap();
    let mr = match mr {
        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
            bind_mr(&ofi.ep, &mr);
            mr.enable().unwrap()
        }
    };

    let desc = mr.description();
    let mut descs = [desc.clone(), desc];
    let mapped_addr = ofi.mapped_addr.clone();

    if server {
        // Single iov message
        let (mem0, mem1) = (&reg_mem[..512], &reg_mem[1024..1536]);
        let (mem0_slice, mem1_slice) = (&mr.slice(0).slice(..512), &mr.slice(0).slice(1024..1536));
        let iov0 = IoVec::from_slice(mem0);
        let iov1 = IoVec::from_slice(mem1);
        let iov0_mr = IoVecMr::from(mem0_slice);
        let iov1_mr = IoVecMr::from(mem1_slice);
        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => MsgType::ConnectedMsg(MsgTaggedConnected::from_iov(
                &iov0,
                &mut descs[0],
                128,
                0,
                0,
            )),
            MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                MsgTaggedConnectedMr::from_iov_mr(&iov0_mr, &mut descs[0], 128, 0, 0),
            ),
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgTagged::from_iov(
                &iov0,
                &mut descs[0],
                mapped_addr.as_ref().unwrap(),
                128,
                0,
                0,
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgTaggedMr::from_iov_mr(
                    &iov0_mr,
                    &mut descs[0],
                    mapped_addr.as_ref().unwrap(),
                    128,
                    0,
                    0,
                ))
            }
        };
        ofi.tsendmsg(&msg);
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        // let entry =
        // match entry {
        //     Completion::Tagged(entry) => assert_eq!(entry[0].data(), 128),
        //     _ => panic!("Unexpected CQ entry format"),
        // }

        // Multi iov message with stride
        let iovs = [iov0, iov1];
        let mr_iovs = [iov0_mr, iov1_mr];
        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => MsgType::ConnectedMsg(MsgTaggedConnected::from_iov_slice(
                &iovs, &mut descs, 0, 1, 0,
            )),
            MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                MsgTaggedConnectedMr::from_iov_slice(&mr_iovs, &mut descs, 0, 1, 0),
            ),
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgTagged::from_iov_slice(
                &iovs,
                &mut descs,
                mapped_addr.as_ref().unwrap(),
                0,
                1,
                0,
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgTaggedMr::from_iov_slice(
                    &mr_iovs,
                    &mut descs,
                    mapped_addr.as_ref().unwrap(),
                    0,
                    1,
                    0,
                ))
            }
        };

        ofi.tsendmsg(&msg);
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Single iov message
        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => MsgType::ConnectedMsg(MsgTaggedConnected::from_iov(
                &iovs[0],
                &mut descs[0],
                0,
                2,
                0,
            )),
            MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                MsgTaggedConnectedMr::from_iov_mr(&mr_iovs[0], &mut descs[0], 0, 2, 0),
            ),
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgTagged::from_iov(
                &iovs[0],
                &mut descs[0],
                mapped_addr.as_ref().unwrap(),
                0,
                2,
                0,
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgTaggedMr::from_iov_mr(
                    &mr_iovs[0],
                    &mut descs[0],
                    mapped_addr.as_ref().unwrap(),
                    0,
                    2,
                    0,
                ))
            }
        };

        ofi.tsendmsg(&msg);
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => MsgType::ConnectedMsg(MsgTaggedConnected::from_iov_slice(
                &iovs, &mut descs, 0, 3, 0,
            )),
            MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                MsgTaggedConnectedMr::from_iov_slice(&mr_iovs, &mut descs, 0, 3, 0),
            ),
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgTagged::from_iov_slice(
                &iovs,
                &mut descs,
                mapped_addr.as_ref().unwrap(),
                0,
                3,
                0,
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgTaggedMr::from_iov_slice(
                    &mr_iovs,
                    &mut descs,
                    mapped_addr.as_ref().unwrap(),
                    0,
                    3,
                    0,
                ))
            }
        };

        ofi.tsendmsg(&msg);
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    } else {
        reg_mem.iter_mut().for_each(|v| *v = 0);
        let (mem0, mem1) = reg_mem.split_at_mut(512);
        let (mem0_mr, mem1_mr) = (&mut mr.slice(0).slice(..512), &mut mr.slice(0).slice(512..));
        let expected: Vec<_> = (0..1024).map(|v: usize| (v % 256) as u8).collect();

        // Receive a single message in a single buffer
        let mut iov = IoVecMut::from_slice(mem0);
        let mut iov_mr = IoVecMutMr::from(mem0_mr);

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => MsgType::ConnectedMsg(MsgTaggedConnectedMut::from_iov(
                &mut iov,
                &mut descs[0],
                0,
                0,
            )),
            MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                MsgTaggedConnectedMutMr::from_iov_mr(&mut iov_mr, &mut descs[0], 0, 0),
            ),
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgTaggedMut::from_iov(
                &mut iov,
                &mut descs[0],
                mapped_addr.as_ref().unwrap(),
                0,
                0,
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgTaggedMutMr::from_iov_mr(
                    &mut iov_mr,
                    &mut descs[0],
                    mapped_addr.as_ref().unwrap(),
                    0,
                    0,
                ))
            }
        };

        ofi.trecvmsg(&msg);
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        // let entry =
        // match entry {
        //     Completion::Tagged(entry) => assert_eq!(entry[0].data(), 128),
        //     _ => panic!("Unexpected CQ entry format"),
        // }
        assert_eq!(mem0.len(), expected[..512].len());
        assert_eq!(mem0, &expected[..512]);

        // Receive a multi iov message in a single buffer
        let mut iov = IoVecMut::from_slice(&mut mem1[..1024]);
        let mem1_mr_1024 = &mut mem1_mr.slice(..1024);
        let mut iov_mr = IoVecMutMr::from(mem1_mr_1024);

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => MsgType::ConnectedMsg(MsgTaggedConnectedMut::from_iov(
                &mut iov,
                &mut descs[0],
                1,
                0,
            )),
            MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                MsgTaggedConnectedMutMr::from_iov_mr(&mut iov_mr, &mut descs[0], 0, 0),
            ),
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgTaggedMut::from_iov(
                &mut iov,
                &mut descs[0],
                mapped_addr.as_ref().unwrap(),
                1,
                0,
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgTaggedMutMr::from_iov_mr(
                    &mut iov_mr,
                    &mut descs[0],
                    mapped_addr.as_ref().unwrap(),
                    1,
                    0,
                ))
            }
        };

        ofi.trecvmsg(&msg);
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();

        assert_eq!(mem1[..1024], expected);

        // Receive a single iov message into two buffers
        reg_mem.iter_mut().for_each(|v| *v = 0);
        let (mem0, mem1) = reg_mem.split_at_mut(512);
        let (mem0_mr, mem1_mr) = (
            &mut mr.slice(0).slice(..256),
            &mut mr.slice(0).slice(512..768),
        );
        let iov = IoVecMut::from_slice(&mut mem0[..256]);
        let iov1 = IoVecMut::from_slice(&mut mem1[..256]);
        let iov_mr = IoVecMutMr::from(mem0_mr);
        let iov1_mr = IoVecMutMr::from(mem1_mr);

        let mut iovs = [iov, iov1];
        let mut iovs_mr = [iov_mr, iov1_mr];

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => MsgType::ConnectedMsg(
                MsgTaggedConnectedMut::from_iov_slice(&mut iovs, &mut descs, 2, 0),
            ),
            MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                MsgTaggedConnectedMutMr::from_iov_mr_slice(&mut iovs_mr, &mut descs, 2, 0),
            ),
            MyEndpoint::Connectionless(_) => {
                MsgType::ConnectionlessMsg(MsgTaggedMut::from_iov_slice(
                    &mut iovs,
                    &mut descs,
                    mapped_addr.as_ref().unwrap(),
                    2,
                    0,
                ))
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgTaggedMutMr::from_iov_mr_slice(
                    &mut iovs_mr,
                    &mut descs,
                    mapped_addr.as_ref().unwrap(),
                    2,
                    0,
                ))
            }
        };

        ofi.trecvmsg(&msg);
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(mem0[..256], expected[..256]);
        assert_eq!(mem1[..256], expected[256..512]);

        // Receive a two iov message into two buffers
        reg_mem.iter_mut().for_each(|v| *v = 0);
        let (mem0, mem1) = reg_mem.split_at_mut(512);
        let iov = IoVecMut::from_slice(&mut mem0[..512]);
        let iov1 = IoVecMut::from_slice(&mut mem1[..512]);
        let (mem0_mr, mem1_mr) = (
            &mut mr.slice(0).slice(..512),
            &mut mr.slice(0).slice(512..1024),
        );
        let mut iovs = [iov, iov1];
        let mut iovs_mr = [IoVecMutMr::from(mem0_mr), IoVecMutMr::from(mem1_mr)];

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => MsgType::ConnectedMsg(
                MsgTaggedConnectedMut::from_iov_slice(&mut iovs, &mut descs, 3, 0),
            ),
            MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                MsgTaggedConnectedMutMr::from_iov_mr_slice(&mut iovs_mr, &mut descs, 3, 0),
            ),
            MyEndpoint::Connectionless(_) => {
                MsgType::ConnectionlessMsg(MsgTaggedMut::from_iov_slice(
                    &mut iovs,
                    &mut descs,
                    mapped_addr.as_ref().unwrap(),
                    3,
                    0,
                ))
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgTaggedMutMr::from_iov_mr_slice(
                    &mut iovs_mr,
                    &mut descs,
                    mapped_addr.as_ref().unwrap(),
                    3,
                    0,
                ))
            }
        };

        ofi.trecvmsg(&msg);
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(mem0[..512], expected[..512]);
        assert_eq!(mem1[..512], expected[512..1024]);
    }
}

#[test]
fn tsendrecvmsg0() {
    tsendrecvmsg(true, "tsendrecvmsg0", false);
}

#[test]
fn tsendrecvmsg1() {
    tsendrecvmsg(false, "tsendrecvmsg0", false);
}

#[test]
fn conn_tsendrecvmsg0() {
    tsendrecvmsg(true, "conn_tsendrecvmsg0", true);
}

#[test]
fn conn_tsendrecvmsg1() {
    tsendrecvmsg(false, "conn_tsendrecvmsg0", true);
}

fn writeread(server: bool, name: &str, connected: bool) {
    let mut ofi = if connected {
        handshake(server, name, Some(InfoCaps::new().msg().rma()))
    } else {
        handshake_connectionless(server, name, Some(InfoCaps::new().msg().rma()))
    };

    let mut reg_mem: Vec<_> = if server {
        (0..1024 * 2)
            .into_iter()
            .map(|v: usize| (v % 256) as u8)
            .collect()
    } else {
        vec![0; 1024 * 2]
    };
    let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
        .access_recv()
        .access_send()
        .access_write()
        .access_read()
        .access_remote_write()
        .access_remote_read()
        .build(&ofi.domain)
        .unwrap();
    let mr = match mr {
        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
            bind_mr(&ofi.ep, &mr);
            mr.enable().unwrap()
        }
    };

    let desc = mr.description();
    let mut descs = [desc.clone(), desc];
    // let mapped_addr = ofi.mapped_addr.clone();
    let key = mr.key().unwrap();
    ofi.exchange_keys(key, reg_mem.as_ptr() as usize, 1024 * 2);
    let expected: Vec<_> = (0..1024).map(|v: usize| (v % 256) as u8).collect();
    if server {
        // Write inject a single buffer
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.write(&reg_mem[..128], 0, &mut descs[0], None);

                // Send completion ack
                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.write_mr(&mr.slice(0).slice(..128), 0, &mut descs[0], None);

                // Send completion ack
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
            }
            MyEndpoint::Connectionless(_) => {
                ofi.write(&reg_mem[..128], 0, &mut descs[0], None);

                // Send completion ack
                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.write_mr(&mr.slice(0).slice(..128), 0, &mut descs[0], None);

                // Send completion ack
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Write a single buffer
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.write(&reg_mem[..512], 0, &mut descs[0], None);
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.write_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], None);
            }
            MyEndpoint::Connectionless(_) => {
                ofi.write(&reg_mem[..512], 0, &mut descs[0], None);
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.write_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], None);
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
            }
            MyEndpoint::Connectionless(_) => {
                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Write vector of buffers
        let iovs = [
            IoVec::from_slice(&reg_mem[..512]),
            IoVec::from_slice(&reg_mem[512..1024]),
        ];

        let slices = (mr.slice(0).slice(..512), mr.slice(0).slice(512..1024));
        let iov_slices = [IoVecMr::from(&slices.0), IoVecMr::from(&slices.1)];

        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.writev(&iovs, 0, &mut descs),
            MyEndpoint::ConnectedMrLocal(_) => ofi.writev_mr(&iov_slices, 0, &mut descs),
            MyEndpoint::Connectionless(_) => ofi.writev(&iovs, 0, &mut descs),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.writev_mr(&iov_slices, 0, &mut descs),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
            }
            MyEndpoint::Connectionless(_) => {
                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.recv(&mut reg_mem[512..1024], &mut descs[0]);
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0]);
            }
            MyEndpoint::Connectionless(_) => {
                ofi.recv(&mut reg_mem[512..1024], &mut descs[0]);
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0]);
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.recv(&mut reg_mem[512..1024], &mut descs[0]);
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0]);
            }
            MyEndpoint::Connectionless(_) => {
                ofi.recv(&mut reg_mem[512..1024], &mut descs[0]);
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0]);
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..128], &expected[..128]);

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.recv(&mut reg_mem[512..1024], &mut descs[0]);
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0]);
            }
            MyEndpoint::Connectionless(_) => {
                ofi.recv(&mut reg_mem[512..1024], &mut descs[0]);
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0]);
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..512], &expected[..512]);

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.recv(&mut reg_mem[1024..1536], &mut descs[0]);
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(1024..1536), &mut descs[0]);
            }
            MyEndpoint::Connectionless(_) => {
                ofi.recv(&mut reg_mem[1024..1536], &mut descs[0]);
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(1024..1536), &mut descs[0]);
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..1024], &expected[..1024]);

        reg_mem.iter_mut().for_each(|v| *v = 0);

        // Read buffer from remote memory
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.read(&mut reg_mem[1024..1536], 0, &mut descs[0]);
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.read_mr(&mut mr.slice(0).slice(1024..1536), 0, &mut descs[0]);
            }
            MyEndpoint::Connectionless(_) => {
                ofi.read(&mut reg_mem[1024..1536], 0, &mut descs[0]);
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.read_mr(&mut mr.slice(0).slice(1024..1536), 0, &mut descs[0]);
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[1024..1536], &expected[512..1024]);

        // Read vector of buffers from remote memory
        let (mem0, mem1) = reg_mem[1536..].split_at_mut(256);
        let iovs = [IoVecMut::from_slice(mem0), IoVecMut::from_slice(mem1)];
        let mut slices = (mr.slice(0).slice(1536..1792), mr.slice(0).slice(1792..));
        let iov_slices = [
            IoVecMutMr::from(&mut slices.0),
            IoVecMutMr::from(&mut slices.1),
        ];

        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.readv(&iovs, 0, &mut descs),
            MyEndpoint::ConnectedMrLocal(_) => ofi.readv_mr(&iov_slices, 0, &mut descs),
            MyEndpoint::Connectionless(_) => ofi.readv(&iovs, 0, &mut descs),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.readv_mr(&iov_slices, 0, &mut descs),
        };
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        assert_eq!(mem0, &expected[..256]);
        assert_eq!(mem1, &expected[..256]);

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
            }
            MyEndpoint::Connectionless(_) => {
                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    }
}

#[test]
fn conn_writeread0() {
    writeread(true, "conn_writeread0", true);
}

#[test]
fn conn_writeread1() {
    writeread(false, "conn_writeread0", true);
}

#[test]
fn writeread0() {
    writeread(true, "writeread0", false);
}

#[test]
fn writeread1() {
    writeread(false, "writeread0", false);
}

fn writereadmsg(server: bool, name: &str, connected: bool) {
    let mut ofi = if connected {
        handshake(server, name, Some(InfoCaps::new().msg().rma()))
    } else {
        handshake_connectionless(server, name, Some(InfoCaps::new().msg().rma()))
    };

    let mut reg_mem: Vec<_> = if server {
        (0..1024 * 2)
            .into_iter()
            .map(|v: usize| (v % 256) as u8)
            .collect()
    } else {
        vec![0; 1024 * 2]
    };
    let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
        .access_recv()
        .access_send()
        .access_write()
        .access_read()
        .access_remote_write()
        .access_remote_read()
        .build(&ofi.domain)
        .unwrap();

    let mr = match mr {
        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
            bind_mr(&ofi.ep, &mr);
            mr.enable().unwrap()
        }
    };
    let desc = mr.description();
    let mut descs = [desc.clone(), desc];
    let mapped_addr = ofi.mapped_addr.clone();

    let key = mr.key().unwrap();
    ofi.exchange_keys(key, reg_mem.as_ptr() as usize, 1024 * 2);
    let expected: Vec<u8> = (0..1024).map(|v: usize| (v % 256) as u8).collect();

    let (start, _end) = ofi.remote_mem_addr.unwrap();
    if server {
        let rma_iov = RmaIoVec::new()
            .address(start)
            .len(128)
            .mapped_key(ofi.remote_key.as_ref().unwrap());

        let iov = IoVec::from_slice(&reg_mem[..128]);
        let mem_mr = mr.slice(0).slice(..128);
        let iov_mr = IoVecMr::from(&mem_mr);

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => {
                MsgType::ConnectedMsg(MsgRmaConnected::from_iov(&iov, &mut descs[0], &rma_iov, 0))
            }
            MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                MsgRmaConnectedMr::from_iov_mr(&iov_mr, &mut descs[0], &rma_iov, 0),
            ),
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgRma::from_iov(
                &iov,
                &mut descs[0],
                mapped_addr.as_ref().unwrap(),
                &rma_iov,
                0,
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgRmaMr::from_iov_mr(
                    &iov_mr,
                    &mut descs[0],
                    mapped_addr.as_ref().unwrap(),
                    &rma_iov,
                    0,
                ))
            }
        };

        // Write inject a single buffer
        ofi.writemsg(&msg);
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        let iov = IoVec::from_slice(&reg_mem[..512]);
        let mem_mr = mr.slice(0).slice(..512);
        let iov_mr = IoVecMr::from(&mem_mr);

        let rma_iov = RmaIoVec::new()
            .address(start)
            .len(512)
            .mapped_key(ofi.remote_key.as_ref().unwrap());

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => MsgType::ConnectedMsg(MsgRmaConnected::from_iov(
                &iov,
                &mut descs[0],
                &rma_iov,
                128,
            )),
            MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                MsgRmaConnectedMr::from_iov_mr(&iov_mr, &mut descs[0], &rma_iov, 128),
            ),
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgRma::from_iov(
                &iov,
                &mut descs[0],
                mapped_addr.as_ref().unwrap(),
                &rma_iov,
                128,
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgRmaMr::from_iov_mr(
                    &iov_mr,
                    &mut descs[0],
                    mapped_addr.as_ref().unwrap(),
                    &rma_iov,
                    128,
                ))
            }
        };

        // let msg = if connected {
        //     MsgType::ConnectedMsg(MsgRmaConnected::from_iov(
        //         &iov,
        //         &mut descs[0],
        //         &rma_iov,
        //         128,
        //     ))
        // } else {
        //     MsgType::ConnectionlessMsg(MsgRma::from_iov(
        //         &iov,
        //         &mut descs[0],
        //         mapped_addr.as_ref().unwrap(),
        //         &rma_iov,
        //         128,
        //     ))
        // };

        // Write a single buffer
        ofi.writemsg(&msg);
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        let iov0 = IoVec::from_slice(&reg_mem[..512]);
        let iov1 = IoVec::from_slice(&reg_mem[512..1024]);
        let (mem_mr0, mem_mr1) = (mr.slice(0).slice(..512), mr.slice(0).slice(512..1024));
        let (iov_mr0, iov_mr1) = (IoVecMr::from(&mem_mr0), IoVecMr::from(&mem_mr1));

        let iovs = [iov0, iov1];
        let iovs_mr = [iov_mr0, iov_mr1];
        let rma_iov0 = RmaIoVec::new()
            .address(start)
            .len(512)
            .mapped_key(ofi.remote_key.as_ref().unwrap());

        let rma_iov1 = RmaIoVec::new()
            .address(start + 512)
            .len(512)
            .mapped_key(ofi.remote_key.as_ref().unwrap());
        let rma_iovs = [rma_iov0, rma_iov1];

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => MsgType::ConnectedMsg(MsgRmaConnected::from_iov_slice(
                &iovs, &mut descs, &rma_iovs, 0,
            )),
            MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                MsgRmaConnectedMr::from_iov_slice(&iovs_mr, &mut descs, &rma_iovs, 0),
            ),
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgRma::from_iov_slice(
                &iovs,
                &mut descs,
                mapped_addr.as_ref().unwrap(),
                &rma_iovs,
                0,
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgRmaMr::from_iov_mr_slice(
                    &iovs_mr,
                    &mut descs,
                    mapped_addr.as_ref().unwrap(),
                    &rma_iovs,
                    0,
                ))
            }
        };

        ofi.writemsg(&msg);
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        // Recv completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..128], &expected[..128]);

        // Recv completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..512], &expected[..512]);

        // Recv completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[1024..1536], &mut descs[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(1024..1536), &mut descs[0])
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[1024..1536], &mut descs[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(1024..1536), &mut descs[0])
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..1024], &expected[..1024]);

        reg_mem.iter_mut().for_each(|v| *v = 0);

        {
            let mut iov = IoVecMut::from_slice(&mut reg_mem[1024..1536]);
            let mut mem_mr = mr.slice(0).slice(1024..1536);
            let mut iov_mr = IoVecMutMr::from(&mut mem_mr);

            let rma_iov = RmaIoVec::new()
                .address(start)
                .len(512)
                .mapped_key(ofi.remote_key.as_ref().unwrap());

            // Read buffer from remote memory
            let msg = match &ofi.ep {
                MyEndpoint::Connected(_) => MsgType::ConnectedMsg(MsgRmaConnectedMut::from_iov(
                    &mut iov,
                    &mut descs[0],
                    &rma_iov,
                )),
                MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                    MsgRmaConnectedMutMr::from_iov_mr(&mut iov_mr, &mut descs[0], &rma_iov),
                ),
                MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgRmaMut::from_iov(
                    &mut iov,
                    &mut descs[0],
                    mapped_addr.as_ref().unwrap(),
                    &rma_iov,
                )),
                MyEndpoint::ConnectionlessMrLocal(_) => {
                    MsgType::ConnectionlessMrMsg(MsgRmaMutMr::from_iov_mr(
                        &mut iov_mr,
                        &mut descs[0],
                        mapped_addr.as_ref().unwrap(),
                        &rma_iov,
                    ))
                }
            };

            ofi.readmsg(&msg);
            ofi.cq_type.tx_cq().sread(1, -1).unwrap();
            assert_eq!(&reg_mem[1024..1536], &expected[512..1024]);
        }

        // // Read vector of buffers from remote memory
        let (mem0, mem1) = reg_mem[1536..].split_at_mut(256);
        let (mut mem_mr0, mut mem_mr1) =
            (mr.slice(0).slice(1536..1892), mr.slice(0).slice(1892..2048));

        let mut iovs = [IoVecMut::from_slice(mem0), IoVecMut::from_slice(mem1)];
        let mut iovs_mr = [
            IoVecMutMr::from(&mut mem_mr0),
            IoVecMutMr::from(&mut mem_mr1),
        ];

        let rma_iov0 = RmaIoVec::new()
            .address(start)
            .len(256)
            .mapped_key(ofi.remote_key.as_ref().unwrap());
        let rma_iov1 = RmaIoVec::new()
            .address(start + 256)
            .len(256)
            .mapped_key(ofi.remote_key.as_ref().unwrap());
        let rma_iovs = [rma_iov0, rma_iov1];

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => MsgType::ConnectedMsg(MsgRmaConnectedMut::from_iov_slice(
                &mut iovs, &mut descs, &rma_iovs,
            )),
            MyEndpoint::ConnectedMrLocal(_) => MsgType::ConnectedMrMsg(
                MsgRmaConnectedMutMr::from_iov_mr_slice(&mut iovs_mr, &mut descs, &rma_iovs),
            ),
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgRmaMut::from_iov_slice(
                &mut iovs,
                &mut descs,
                mapped_addr.as_ref().unwrap(),
                &rma_iovs,
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgRmaMutMr::from_iov_mr_slice(
                    &mut iovs_mr,
                    &mut descs,
                    mapped_addr.as_ref().unwrap(),
                    &rma_iovs,
                ))
            }
        };

        ofi.readmsg(&msg);
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        assert_eq!(mem0, &expected[..256]);
        assert_eq!(mem1, &expected[..256]);

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    }
}

#[test]
fn writereadmsg0() {
    writereadmsg(true, "writereadmsg0", false);
}

#[test]
fn writereadmsg1() {
    writereadmsg(false, "writereadmsg0", false);
}

#[test]
fn conn_writereadmsg0() {
    writereadmsg(true, "conn_writereadmsg0", true);
}

#[test]
fn conn_writereadmsg1() {
    writereadmsg(false, "conn_writereadmsg0", true);
}

fn atomic(server: bool, name: &str, connected: bool) {
    let mut ofi = if connected {
        handshake(server, name, Some(InfoCaps::new().msg().atomic()))
    } else {
        handshake_connectionless(server, name, Some(InfoCaps::new().msg().atomic()))
    };

    let mut reg_mem: Vec<_> = if server {
        vec![2; 1024 * 2]
    } else {
        vec![1; 1024 * 2]
    };
    let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
        .access_recv()
        .access_send()
        .access_write()
        .access_read()
        .access_remote_write()
        .access_remote_read()
        .build(&ofi.domain)
        .unwrap();

    let mr = match mr {
        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
            bind_mr(&ofi.ep, &mr);
            mr.enable().unwrap()
        }
    };
    let desc = mr.description();
    let mut descs = [desc.clone(), desc];
    // let mapped_addr = ofi.mapped_addr.clone();
    let key = mr.key().unwrap();
    ofi.exchange_keys(key, reg_mem.as_ptr() as usize, 1024 * 2);
    if server {
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Min);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Max);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Sum);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Prod);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Bor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Band);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                ofi.recv(&mut reg_mem[512..1024], &mut descs[0]);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Lor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Bxor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.recv(&mut reg_mem[512..1024], &mut descs[0]);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Land);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Lxor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::AtomicWrite);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Min);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Max);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Sum);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Prod);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Bor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Band);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0]);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Lor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Bxor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0]);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Land);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Lxor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(
                    &mr.slice(0).slice(..512),
                    0,
                    &mut descs[0],
                    AtomicOp::AtomicWrite,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
            }
            MyEndpoint::Connectionless(_) => {
                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Min);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Max);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Sum);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Prod);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Bor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Band);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                ofi.recv(&mut reg_mem[512..1024], &mut descs[0]);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Lor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Bxor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.recv(&mut reg_mem[512..1024], &mut descs[0]);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Land);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Lxor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::AtomicWrite);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.send(&reg_mem[512..1024], &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Min);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Max);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Sum);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Prod);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Bor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Band);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0]);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Lor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Bxor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0]);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Land);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(&mr.slice(0).slice(..512), 0, &mut descs[0], AtomicOp::Lxor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(
                    &mr.slice(0).slice(..512),
                    0,
                    &mut descs[0],
                    AtomicOp::AtomicWrite,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
            }
        }

        let iocs = [
            Ioc::from_slice(&reg_mem[..256]),
            Ioc::from_slice(&reg_mem[256..512]),
        ];

        let (mem_mr0, mem_mr1) = (mr.slice(0).slice(..256), mr.slice(0).slice(256..512));
        let iocs_mr = [IocMr::from(&mem_mr0), IocMr::from(&mem_mr1)];

        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.atomicv(&iocs, 0, &mut descs, AtomicOp::Prod),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.atomicv_mr(&iocs_mr, 0, &mut descs, AtomicOp::Prod)
            }
            MyEndpoint::Connectionless(_) => ofi.atomicv(&iocs, 0, &mut descs, AtomicOp::Prod),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.atomicv_mr(&iocs_mr, 0, &mut descs, AtomicOp::Prod)
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        let mut expected = vec![2u8; 1024 * 2];

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..512], &expected[..512]);

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        expected = vec![3; 1024 * 2];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..512], &expected[..512]);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // expected = vec![2;1024*2];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        // assert_eq!(&reg_mem[..512], &expected[..512]);

        expected = vec![4; 1024 * 2];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..512], &expected[..512]);

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    }
}

// [TODO Not sure why, but connected endpoints fail with atomic ops
// #[test]
// fn conn_atomic0() {
//     atomic(true, "conn_atomic0", true);
// }

// #[test]
// fn conn_atomic1() {
//     atomic(false, "conn_atomic0", true);
// }

#[test]
fn atomic0() {
    atomic(true, "atomic0", false);
}

#[test]
fn atomic1() {
    atomic(false, "atomic0", false);
}

fn fetch_atomic(server: bool, name: &str, connected: bool) {
    let mut ofi = if connected {
        handshake(server, name, Some(InfoCaps::new().msg().atomic()))
    } else {
        handshake_connectionless(server, name, Some(InfoCaps::new().msg().atomic()))
    };

    let mut reg_mem: Vec<_> = if server {
        vec![2; 1024 * 2]
    } else {
        vec![1; 1024 * 2]
    };
    let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
        .access_recv()
        .access_send()
        .access_write()
        .access_read()
        .access_remote_write()
        .access_remote_read()
        .build(&ofi.domain)
        .unwrap();

    let mr = match mr {
        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
            bind_mr(&ofi.ep, &mr);
            mr.enable().unwrap()
        }
    };

    let mut desc0 = mr.description();
    let mut desc1 = mr.description();
    // let mapped_addr = ofi.mapped_addr.clone();
    let key = mr.key().unwrap();
    ofi.exchange_keys(key, reg_mem.as_ptr() as usize, 1024 * 2);
    if server {
        let mut expected: Vec<_> = vec![1; 256];
        let (op_mem, ack_mem) = reg_mem.split_at_mut(512);
        let (op_mem_mr, mut ack_mem_mr) = (mr.slice(0).slice(..512), mr.slice(0).slice(512..1024));
        let (mem0, mem1) = op_mem.split_at_mut(256);
        let (mem_mr0, mut mem_mr1) = (op_mem_mr.slice(..256), op_mem_mr.slice(256..));

        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Min);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected[..256]);

                expected = vec![1; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Max);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![2; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Sum);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![4; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Prod);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![8; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Bor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![10; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Band);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                // Send a done ack
                ofi.send(&ack_mem[..512], &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv(&mut ack_mem[..512], &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![2; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Lor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![1; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Bxor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                // Send a done ack
                ofi.send(&ack_mem[..512], &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv(&mut ack_mem[..512], &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![3; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Land);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![1; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Lxor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![0; 256];
                ofi.fetch_atomic(
                    &mem0,
                    mem1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::AtomicWrite,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                // Send a done ack
                ofi.send(&ack_mem[..512], &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv(&mut ack_mem[..512], &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![2; 256];
                ofi.fetch_atomic(
                    &mem0,
                    mem1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::AtomicRead,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Min,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected[..256]);

                expected = vec![1; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Max,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![2; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Sum,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![4; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Prod,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![8; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Bor,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![10; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Band,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                // Send a done ack
                ofi.send_mr(&ack_mem_mr, &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv_mr(&mut ack_mem_mr, &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![2; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Lor,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![1; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Bxor,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                // Send a done ack
                ofi.send_mr(&ack_mem_mr, &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv_mr(&mut ack_mem_mr, &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![3; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Land,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![1; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Lxor,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![0; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::AtomicWrite,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                // Send a done ack
                ofi.send_mr(&ack_mem_mr, &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv_mr(&mut ack_mem_mr, &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![2; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::AtomicRead,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);
            }
            MyEndpoint::Connectionless(_) => {
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Min);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected[..256]);

                expected = vec![1; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Max);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![2; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Sum);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![4; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Prod);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![8; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Bor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![10; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Band);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                // Send a done ack
                ofi.send(&ack_mem[..512], &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv(&mut ack_mem[..512], &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![2; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Lor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![1; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Bxor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                // Send a done ack
                ofi.send(&ack_mem[..512], &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv(&mut ack_mem[..512], &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![3; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Land);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![1; 256];
                ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Lxor);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![0; 256];
                ofi.fetch_atomic(
                    &mem0,
                    mem1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::AtomicWrite,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                // Send a done ack
                ofi.send(&ack_mem[..512], &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv(&mut ack_mem[..512], &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![2; 256];
                ofi.fetch_atomic(
                    &mem0,
                    mem1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::AtomicRead,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Min,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected[..256]);

                expected = vec![1; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Max,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![2; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Sum,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![4; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Prod,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![8; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Bor,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![10; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Band,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                // Send a done ack
                ofi.send_mr(&ack_mem_mr, &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv_mr(&mut ack_mem_mr, &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![2; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Lor,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![1; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Bxor,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                // Send a done ack
                ofi.send_mr(&ack_mem_mr, &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv_mr(&mut ack_mem_mr, &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![3; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Land,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![1; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::Lxor,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                expected = vec![0; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::AtomicWrite,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);

                // Send a done ack
                ofi.send_mr(&ack_mem_mr, &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv_mr(&mut ack_mem_mr, &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![2; 256];
                ofi.fetch_atomic_mr(
                    &mem_mr0,
                    &mut mem_mr1,
                    0,
                    &mut desc0,
                    &mut desc1,
                    FetchAtomicOp::AtomicRead,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                assert_eq!(mem1, &expected);
            }
        }

        expected = vec![2; 256];
        let (read_mem, write_mem) = op_mem.split_at_mut(256);
        let (read_mem_mr, write_mem_mr) = (op_mem_mr.slice(..256), op_mem_mr.slice(256..));
        let (read_mem_mr0, read_mem_mr1) = (read_mem_mr.slice(..128), read_mem_mr.slice(128..));
        let (mut write_mem_mr0, mut write_mem_mr1) =
            (write_mem_mr.slice(..128), write_mem_mr.slice(128..));
        let iocs = [
            Ioc::from_slice(&read_mem[..128]),
            Ioc::from_slice(&read_mem[128..256]),
        ];

        let iocs_mr = [IocMr::from(&read_mem_mr0), IocMr::from(&read_mem_mr1)];

        let write_mems = write_mem.split_at_mut(128);
        let mut res_iocs = [
            IocMut::from_slice(write_mems.0),
            IocMut::from_slice(write_mems.1),
        ];

        let mut res_iocs_mr = [
            IocMutMr::from(&mut write_mem_mr0),
            IocMutMr::from(&mut write_mem_mr1),
        ];

        let desc0 = mr.description();
        let desc1 = mr.description();
        let desc2 = mr.description();
        let desc3 = mr.description();
        let mut descs = [desc0, desc1];
        let mut res_descs = [desc2, desc3];

        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.fetch_atomicv(
                &iocs,
                &mut res_iocs,
                0,
                &mut descs,
                &mut res_descs,
                FetchAtomicOp::Prod,
            ),
            MyEndpoint::ConnectedMrLocal(_) => ofi.fetch_atomicv_mr(
                &iocs_mr,
                &mut res_iocs_mr,
                0,
                &mut descs,
                &mut res_descs,
                FetchAtomicOp::Prod,
            ),
            MyEndpoint::Connectionless(_) => ofi.fetch_atomicv(
                &iocs,
                &mut res_iocs,
                0,
                &mut descs,
                &mut res_descs,
                FetchAtomicOp::Prod,
            ),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.fetch_atomicv_mr(
                &iocs_mr,
                &mut res_iocs_mr,
                0,
                &mut descs,
                &mut res_descs,
                FetchAtomicOp::Prod,
            ),
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(write_mem, &expected);

        // Send a done ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&ack_mem[..512], &mut descs[0], None),
            MyEndpoint::ConnectedMrLocal(_) => ofi.send_mr(&ack_mem_mr, &mut descs[0], None),
            MyEndpoint::Connectionless(_) => ofi.send(&ack_mem[..512], &mut descs[0], None),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.send_mr(&ack_mem_mr, &mut descs[0], None),
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut ack_mem[..512], &mut descs[0]),
            MyEndpoint::ConnectedMrLocal(_) => ofi.recv_mr(&mut ack_mem_mr, &mut descs[0]),
            MyEndpoint::Connectionless(_) => ofi.recv(&mut ack_mem[..512], &mut descs[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.recv_mr(&mut ack_mem_mr, &mut descs[0]),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        let mut expected = vec![2u8; 256];

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc0),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc0)
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc0),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc0)
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut desc0, None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut desc0, None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        expected = vec![3; 256];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc0),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc0)
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc0),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc0)
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut desc0, None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut desc0, None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        expected = vec![2; 256];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc0),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc0)
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc0),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc0)
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut desc0, None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut desc0, None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        expected = vec![4; 256];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc0),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc0)
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc0),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc0)
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut desc0, None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut desc0, None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    }
}

#[test]
fn fetch_atomic0() {
    fetch_atomic(true, "fetch_atomic0", false);
}

#[test]
fn fetch_atomic1() {
    fetch_atomic(false, "fetch_atomic0", false);
}

// [TODO Not sure why, but connected endpoints fail with atomic ops
// #[test]
// fn conn_fetch_atomic0() {
//     fetch_atomic(true, "conn_fetch_atomic0", true);
// }

// #[test]
// fn conn_fetch_atomic1() {
//     fetch_atomic(false, "conn_fetch_atomic0", true);
// }

fn compare_atomic(server: bool, name: &str, connected: bool) {
    let mut ofi = if connected {
        handshake(server, name, Some(InfoCaps::new().msg().atomic()))
    } else {
        handshake_connectionless(server, name, Some(InfoCaps::new().msg().atomic()))
    };

    let mut reg_mem: Vec<_> = if server {
        vec![2; 1024 * 2]
    } else {
        vec![1; 1024 * 2]
    };
    let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
        .access_recv()
        .access_send()
        .access_write()
        .access_read()
        .access_remote_write()
        .access_remote_read()
        .build(&ofi.domain)
        .unwrap();

    let mr = match mr {
        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
            bind_mr(&ofi.ep, &mr);
            mr.enable().unwrap()
        }
    };
    let mut desc = mr.description();
    let mut comp_desc = mr.description();
    let mut res_desc = mr.description();
    let key = mr.key().unwrap();
    ofi.exchange_keys(key, reg_mem.as_ptr() as usize, 1024 * 2);
    if server {
        let mut expected: Vec<_> = vec![1; 256];
        let (op_mem, ack_mem) = reg_mem.split_at_mut(768);
        let (op_mem_mr, mut ack_mem_mr) = (mr.slice(0).slice(..768), mr.slice(0).slice(768..1280));
        let (buf, mem1) = op_mem.split_at_mut(256);
        let (comp, res) = mem1.split_at_mut(256);
        comp.iter_mut().for_each(|v| *v = 1);
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.compare_atomic(
                    &buf,
                    comp,
                    res,
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::Cswap,
                );
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.compare_atomic_mr(
                    &mr.slice(0).slice(0..256),
                    &mr.slice(0).slice(256..512),
                    &mut mr.slice(0).slice(512..768),
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::Cswap,
                );
            }
            MyEndpoint::Connectionless(_) => {
                ofi.compare_atomic(
                    &buf,
                    comp,
                    res,
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::Cswap,
                );
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.compare_atomic_mr(
                    &mr.slice(0).slice(0..256),
                    &mr.slice(0).slice(256..512),
                    &mut mr.slice(0).slice(512..768),
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::Cswap,
                );
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(res, &expected[..256]);

        expected = vec![2; 256];
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.compare_atomic(
                    &buf,
                    comp,
                    res,
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapNe,
                );
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.compare_atomic_mr(
                    &mr.slice(0).slice(0..256),
                    &mr.slice(0).slice(256..512),
                    &mut mr.slice(0).slice(512..768),
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapNe,
                );
            }
            MyEndpoint::Connectionless(_) => {
                ofi.compare_atomic(
                    &buf,
                    comp,
                    res,
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapNe,
                );
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.compare_atomic_mr(
                    &mr.slice(0).slice(0..256),
                    &mr.slice(0).slice(256..512),
                    &mut mr.slice(0).slice(512..768),
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapNe,
                );
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(res, &expected);

        buf.iter_mut().for_each(|v| *v = 3);
        expected = vec![2; 256];
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.compare_atomic(
                    &buf,
                    comp,
                    res,
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapLe,
                );
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.compare_atomic_mr(
                    &mr.slice(0).slice(0..256),
                    &mr.slice(0).slice(256..512),
                    &mut mr.slice(0).slice(512..768),
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapLe,
                );
            }
            MyEndpoint::Connectionless(_) => {
                ofi.compare_atomic(
                    &buf,
                    comp,
                    res,
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapLe,
                );
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.compare_atomic_mr(
                    &mr.slice(0).slice(0..256),
                    &mr.slice(0).slice(256..512),
                    &mut mr.slice(0).slice(512..768),
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapLe,
                );
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(res, &expected);

        buf.iter_mut().for_each(|v| *v = 2);
        expected = vec![3; 256];
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.compare_atomic(
                    &buf,
                    comp,
                    res,
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapLt,
                );
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.compare_atomic_mr(
                    &mr.slice(0).slice(0..256),
                    &mr.slice(0).slice(256..512),
                    &mut mr.slice(0).slice(512..768),
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapLt,
                );
            }
            MyEndpoint::Connectionless(_) => {
                ofi.compare_atomic(
                    &buf,
                    comp,
                    res,
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapLt,
                );
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.compare_atomic_mr(
                    &mr.slice(0).slice(0..256),
                    &mr.slice(0).slice(256..512),
                    &mut mr.slice(0).slice(512..768),
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapLt,
                );
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(res, &expected);

        buf.iter_mut().for_each(|v| *v = 3);
        expected = vec![2; 256];
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.compare_atomic(
                    &buf,
                    comp,
                    res,
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapGe,
                );
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.compare_atomic_mr(
                    &mr.slice(0).slice(0..256),
                    &mr.slice(0).slice(256..512),
                    &mut mr.slice(0).slice(512..768),
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapGe,
                );
            }
            MyEndpoint::Connectionless(_) => {
                ofi.compare_atomic(
                    &buf,
                    comp,
                    res,
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapGe,
                );
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.compare_atomic_mr(
                    &mr.slice(0).slice(0..256),
                    &mr.slice(0).slice(256..512),
                    &mut mr.slice(0).slice(512..768),
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapGe,
                );
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(res, &expected);

        expected = vec![2; 256];
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.compare_atomic(
                    &buf,
                    comp,
                    res,
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapGt,
                );
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.compare_atomic_mr(
                    &mr.slice(0).slice(0..256),
                    &mr.slice(0).slice(256..512),
                    &mut mr.slice(0).slice(512..768),
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapGt,
                );
            }
            MyEndpoint::Connectionless(_) => {
                ofi.compare_atomic(
                    &buf,
                    comp,
                    res,
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapGt,
                );
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.compare_atomic_mr(
                    &mr.slice(0).slice(0..256),
                    &mr.slice(0).slice(256..512),
                    &mut mr.slice(0).slice(512..768),
                    0,
                    &mut desc,
                    &mut comp_desc,
                    &mut res_desc,
                    CompareAtomicOp::CswapGt,
                );
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(res, &expected);

        // Send a done ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&ack_mem[..512], &mut desc, None),
            MyEndpoint::ConnectedMrLocal(_) => ofi.send_mr(&ack_mem_mr, &mut desc, None),
            MyEndpoint::Connectionless(_) => ofi.send(&ack_mem[..512], &mut desc, None),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.send_mr(&ack_mem_mr, &mut desc, None),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut ack_mem[..512], &mut desc),
            MyEndpoint::ConnectedMrLocal(_) => ofi.recv_mr(&mut ack_mem_mr, &mut desc),
            MyEndpoint::Connectionless(_) => ofi.recv(&mut ack_mem[..512], &mut desc),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.recv_mr(&mut ack_mem_mr, &mut desc),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();

        // expected = vec![2; 256];
        let (buf0, buf1) = buf.split_at_mut(128);
        let (comp0, comp1) = comp.split_at_mut(128);
        let (res0, res1) = res.split_at_mut(128);

        let buf_iocs = [Ioc::from_slice(&buf0), Ioc::from_slice(&buf1)];
        let comp_iocs = [Ioc::from_slice(&comp0), Ioc::from_slice(&comp1)];
        let mut res_iocs = [IocMut::from_slice(res0), IocMut::from_slice(res1)];

        let buf_slices = (&mr.slice(0).slice(..128), &mr.slice(0).slice(128..256));
        let buf_iocs_mr = [IocMr::from(buf_slices.0), IocMr::from(buf_slices.1)];

        let comp_slices = (&mr.slice(0).slice(256..384), &mr.slice(0).slice(384..512));
        let comp_iocs_mr = [IocMr::from(comp_slices.0), IocMr::from(comp_slices.1)];

        let mut res_slices = (
            &mut mr.slice(0).slice(512..640),
            &mut mr.slice(0).slice(640..768),
        );

        let mut res_iocs_mr = [
            IocMutMr::from(&mut res_slices.0),
            IocMutMr::from(&mut res_slices.1),
        ];
        let mut buf_descs = [mr.description(), mr.description()];
        let mut comp_descs = [mr.description(), mr.description()];
        let mut res_descs = [mr.description(), mr.description()];
        match &ofi.ep {
            MyEndpoint::Connected(_) => {
                ofi.compare_atomicv(
                    &buf_iocs,
                    &comp_iocs,
                    &mut res_iocs,
                    0,
                    &mut buf_descs,
                    &mut comp_descs,
                    &mut res_descs,
                    CompareAtomicOp::CswapLe,
                );
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.compare_atomicv_mr(
                    &buf_iocs_mr,
                    &comp_iocs_mr,
                    &mut res_iocs_mr,
                    0,
                    &mut buf_descs,
                    &mut comp_descs,
                    &mut res_descs,
                    CompareAtomicOp::CswapLe,
                );
            }
            MyEndpoint::Connectionless(_) => {
                ofi.compare_atomicv(
                    &buf_iocs,
                    &comp_iocs,
                    &mut res_iocs,
                    0,
                    &mut buf_descs,
                    &mut comp_descs,
                    &mut res_descs,
                    CompareAtomicOp::CswapLe,
                );
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.compare_atomicv_mr(
                    &buf_iocs_mr,
                    &comp_iocs_mr,
                    &mut res_iocs_mr,
                    0,
                    &mut buf_descs,
                    &mut comp_descs,
                    &mut res_descs,
                    CompareAtomicOp::CswapLe,
                );
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(res, &expected);

        // Send a done ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&ack_mem[..512], &mut desc, None),
            MyEndpoint::ConnectedMrLocal(_) => ofi.send_mr(&ack_mem_mr, &mut desc, None),
            MyEndpoint::Connectionless(_) => ofi.send(&ack_mem[..512], &mut desc, None),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.send_mr(&ack_mem_mr, &mut desc, None),
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut ack_mem[..512], &mut desc),
            MyEndpoint::ConnectedMrLocal(_) => ofi.recv_mr(&mut ack_mem_mr, &mut desc),
            MyEndpoint::Connectionless(_) => ofi.recv(&mut ack_mem[..512], &mut desc),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.recv_mr(&mut ack_mem_mr, &mut desc),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        let mut expected = vec![2u8; 256];

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc)
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc)
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut desc, None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mut mr.slice(0).slice(512..1024), &mut desc, None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut desc, None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mut mr.slice(0).slice(512..1024), &mut desc, None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        expected = vec![3; 256];
        // // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc)
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc)
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut desc, None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mut mr.slice(0).slice(512..1024), &mut desc, None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut desc, None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mut mr.slice(0).slice(512..1024), &mut desc, None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    }
}

#[test]
fn compare_atomic0() {
    compare_atomic(true, "compare_atomic0", false);
}

#[test]
fn compare_atomic1() {
    compare_atomic(false, "compare_atomic0", false);
}

// [TODO Not sure why, but connected endpoints fail with atomic ops
// #[test]
// fn conn_compare_atomic0() {
//     compare_atomic(true, "conn_compare_atomic0", true);
// }

// #[test]
// fn conn_compare_atomic1() {
//     compare_atomic(false, "conn_compare_atomic0", true);
// }

fn atomicmsg(server: bool, name: &str, connected: bool) {
    let mut ofi = if connected {
        handshake(server, name, Some(InfoCaps::new().msg().atomic()))
    } else {
        handshake_connectionless(server, name, Some(InfoCaps::new().msg().atomic()))
    };

    let mut reg_mem: Vec<_> = if server {
        vec![2; 1024 * 2]
    } else {
        vec![1; 1024 * 2]
    };
    let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
        .access_recv()
        .access_send()
        .access_write()
        .access_read()
        .access_remote_write()
        .access_remote_read()
        .build(&ofi.domain)
        .unwrap();

    let mr = match mr {
        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
            bind_mr(&ofi.ep, &mr);
            mr.enable().unwrap()
        }
    };
    let desc = mr.description();
    let mut descs = [desc.clone(), desc];
    let mapped_addr = ofi.mapped_addr.clone();
    let key = mr.key().unwrap();
    ofi.exchange_keys(key, reg_mem.as_ptr() as usize, 1024 * 2);
    let (start, _end) = ofi.remote_mem_addr.unwrap();
    if server {
        let iocs = [
            Ioc::from_slice(&reg_mem[..256]),
            Ioc::from_slice(&reg_mem[256..512]),
        ];
        let (mem_mr0, mem_mr1) = (mr.slice(0).slice(..256), mr.slice(0).slice(256..512));
        let iocs_mr = [IocMr::from(&mem_mr0), IocMr::from(&mem_mr1)];

        let rma_ioc0 = RmaIoc::new(start, 256, ofi.remote_key.as_ref().unwrap());
        let rma_ioc1 = RmaIoc::new(start + 256, 256, ofi.remote_key.as_ref().unwrap());
        let rma_iocs = [rma_ioc0, rma_ioc1];

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => MsgType::ConnectedMsg(MsgAtomicConnected::from_ioc_slice(
                &iocs,
                &mut descs,
                &rma_iocs,
                AtomicOp::Bor,
                128,
            )),
            MyEndpoint::ConnectedMrLocal(_) => {
                MsgType::ConnectedMrMsg(MsgAtomicConnectedMr::from_ioc_mr_slice(
                    &iocs_mr,
                    &mut descs,
                    &rma_iocs,
                    AtomicOp::Bor,
                    128,
                ))
            }
            MyEndpoint::Connectionless(_) => MsgType::ConnectionlessMsg(MsgAtomic::from_ioc_slice(
                &iocs,
                &mut descs,
                mapped_addr.as_ref().unwrap(),
                &rma_iocs,
                AtomicOp::Bor,
                128,
            )),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgAtomicMr::from_ioc_mr_slice(
                    &iocs_mr,
                    &mut descs,
                    mapped_addr.as_ref().unwrap(),
                    &rma_iocs,
                    AtomicOp::Bor,
                    128,
                ))
            }
        };

        ofi.atomicmsg(&msg);
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        let expected = vec![3u8; 1024 * 2];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut descs[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut descs[0])
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..512], &expected[..512]);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    }
}

// [TODO Not sure why, but connected endpoints fail with atomic ops
// #[test]
// fn conn_atomic0() {
//     atomic(true, "conn_atomic0", true);
// }

// #[test]
// fn conn_atomic1() {
//     atomic(false, "conn_atomic0", true);
// }

#[test]
fn atomicmsg0() {
    atomicmsg(true, "atomicmsg0", false);
}

#[test]
fn atomicmsg1() {
    atomicmsg(false, "atomicmsg0", false);
}

fn fetch_atomicmsg(server: bool, name: &str, connected: bool) {
    let mut ofi = if connected {
        handshake(server, name, Some(InfoCaps::new().msg().atomic()))
    } else {
        handshake_connectionless(server, name, Some(InfoCaps::new().msg().atomic()))
    };

    let mut reg_mem: Vec<_> = if server {
        vec![2; 1024 * 2]
    } else {
        vec![1; 1024 * 2]
    };
    let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
        .access_recv()
        .access_send()
        .access_write()
        .access_read()
        .access_remote_write()
        .access_remote_read()
        .build(&ofi.domain)
        .unwrap();
    let mr = match mr {
        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
            bind_mr(&ofi.ep, &mr);
            mr.enable().unwrap()
        }
    };
    let mapped_addr = ofi.mapped_addr.clone();
    let key = mr.key().unwrap();
    ofi.exchange_keys(key, reg_mem.as_ptr() as usize, 1024 * 2);
    let (start, _end) = ofi.remote_mem_addr.unwrap();

    if server {
        let expected = vec![1u8; 256];
        let (op_mem, ack_mem) = reg_mem.split_at_mut(512);
        let (op_mem_mr, mut ack_mem_mr) =
            (mr.slice(0).slice(..512), &mut mr.slice(0).slice(512..1024));

        let (read_mem, write_mem) = op_mem.split_at_mut(256);
        let (read_mem_mr, write_mem_mr) = (op_mem_mr.slice(..256), op_mem_mr.slice(256..));
        let (read_mem_mr0, read_mem_mr1) = (read_mem_mr.slice(..128), read_mem_mr.slice(128..256));

        let iocs = [
            Ioc::from_slice(&read_mem[..128]),
            Ioc::from_slice(&read_mem[128..256]),
        ];
        let iocs_mr = [IocMr::from(&read_mem_mr0), IocMr::from(&read_mem_mr1)];

        let write_mems = write_mem.split_at_mut(128);
        let write_mems_mr = (
            &mut write_mem_mr.slice(..128),
            &mut write_mem_mr.slice(128..),
        );

        let mut res_iocs = [
            IocMut::from_slice(write_mems.0),
            IocMut::from_slice(write_mems.1),
        ];

        let mut res_iocs_mr = [
            IocMutMr::from(write_mems_mr.0),
            IocMutMr::from(write_mems_mr.1),
        ];

        let desc0 = mr.description();
        let desc1 = mr.description();
        let desc2 = mr.description();
        let desc3 = mr.description();
        let mut descs = [desc0, desc1];
        let mut res_descs = [desc2, desc3];
        let rma_ioc0 = RmaIoc::new(start, 128, ofi.remote_key.as_ref().unwrap());
        let rma_ioc1 = RmaIoc::new(start + 128, 128, ofi.remote_key.as_ref().unwrap());
        let rma_iocs = [rma_ioc0, rma_ioc1];

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => {
                MsgType::ConnectedMsg(MsgFetchAtomicConnected::from_ioc_slice(
                    &iocs,
                    &mut descs,
                    &rma_iocs,
                    FetchAtomicOp::Prod,
                    0,
                ))
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                MsgType::ConnectedMrMsg(MsgFetchAtomicConnectedMr::from_ioc_mr_slice(
                    &iocs_mr,
                    &mut descs,
                    &rma_iocs,
                    FetchAtomicOp::Prod,
                    0,
                ))
            }
            MyEndpoint::Connectionless(_) => {
                MsgType::ConnectionlessMsg(MsgFetchAtomic::from_ioc_slice(
                    &iocs,
                    &mut descs,
                    mapped_addr.as_ref().unwrap(),
                    &rma_iocs,
                    FetchAtomicOp::Prod,
                    0,
                ))
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgFetchAtomicMr::from_ioc_mr_slice(
                    &iocs_mr,
                    &mut descs,
                    mapped_addr.as_ref().unwrap(),
                    &rma_iocs,
                    FetchAtomicOp::Prod,
                    0,
                ))
            }
        };
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.fetch_atomicmsg(&msg, &mut res_iocs, &mut res_descs),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.fetch_atomicmsg_mr(&msg, &mut res_iocs_mr, &mut res_descs)
            }
            MyEndpoint::Connectionless(_) => {
                ofi.fetch_atomicmsg(&msg, &mut res_iocs, &mut res_descs)
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.fetch_atomicmsg_mr(&msg, &mut res_iocs_mr, &mut res_descs)
            }
        };

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(write_mem, &expected);

        // Send a done ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&ack_mem[..512], &mut descs[0], None),
            MyEndpoint::ConnectedMrLocal(_) => ofi.send_mr(&ack_mem_mr, &mut descs[0], None),
            MyEndpoint::Connectionless(_) => ofi.send(&ack_mem[..512], &mut descs[0], None),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.send_mr(&ack_mem_mr, &mut descs[0], None),
        };
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv a completion ack

        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut ack_mem[..512], &mut descs[0]),
            MyEndpoint::ConnectedMrLocal(_) => ofi.recv_mr(&mut ack_mem_mr, &mut descs[0]),
            MyEndpoint::Connectionless(_) => ofi.recv(&mut ack_mem[..512], &mut descs[0]),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.recv_mr(&mut ack_mem_mr, &mut descs[0]),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        let mut desc0 = mr.description();
        let expected = vec![2u8; 256];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc0),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc0)
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc0),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc0)
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut desc0, None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut desc0, None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    }
}

#[test]
fn fetch_atomicmsg0() {
    fetch_atomicmsg(true, "fetch_atomicmsg0", false);
}

#[test]
fn fetch_atomicmsg1() {
    fetch_atomicmsg(false, "fetch_atomicmsg0", false);
}

// [TODO Not sure why, but connected endpoints fail with atomic ops
// #[test]
// fn conn_fetch_atomic0() {
//     fetch_atomic(true, "conn_fetch_atomic0", true);
// }

// #[test]
// fn conn_fetch_atomic1() {
//     fetch_atomic(false, "conn_fetch_atomic0", true);
// }

fn compare_atomicmsg(server: bool, name: &str, connected: bool) {
    let mut ofi = if connected {
        handshake(server, name, Some(InfoCaps::new().msg().atomic()))
    } else {
        handshake_connectionless(server, name, Some(InfoCaps::new().msg().atomic()))
    };

    let mut reg_mem: Vec<_> = if server {
        vec![2; 1024 * 2]
    } else {
        vec![1; 1024 * 2]
    };
    let mr = MemoryRegionBuilder::new(&reg_mem, libfabric::enums::HmemIface::System)
        .access_recv()
        .access_send()
        .access_write()
        .access_read()
        .access_remote_write()
        .access_remote_read()
        .build(&ofi.domain)
        .unwrap();

    let mr = match mr {
        libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
        libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
            bind_mr(&ofi.ep, &mr);
            mr.enable().unwrap()
        }
    };

    let mut desc = mr.description();
    let mapped_addr = ofi.mapped_addr.clone();
    let key = mr.key().unwrap();
    ofi.exchange_keys(key, reg_mem.as_ptr() as usize, 1024 * 2);
    let (start, _end) = ofi.remote_mem_addr.unwrap();

    if server {
        let expected = vec![1u8; 256];
        let (op_mem, ack_mem) = reg_mem.split_at_mut(768);
        let op_mem_mr = mr.slice(0).slice(..768);
        let ack_mem_mr = mr.slice(0).slice(768..);
        let (buf, mem1) = op_mem.split_at_mut(256);
        let (buf_mr, mem1_mr) = (op_mem_mr.slice(..256), op_mem_mr.slice(256..));
        let (comp, res) = mem1.split_at_mut(256);
        let (comp_mr, res_mr) = (mem1_mr.slice(..256), mem1_mr.slice(256..));
        comp.iter_mut().for_each(|v| *v = 1);

        // expected = vec![2; 256];
        let (buf0, buf1) = buf.split_at_mut(128);
        let (comp0, comp1) = comp.split_at_mut(128);
        let (res0, res1) = res.split_at_mut(128);
        let (buf0_mr, buf1_mr) = (buf_mr.slice(..128), buf_mr.slice(128..));
        let (comp0_mr, comp1_mr) = (comp_mr.slice(..128), comp_mr.slice(128..));
        let (res0_mr, res1_mr) = (&mut res_mr.slice(..128), &mut res_mr.slice(128..));

        let buf_iocs = [Ioc::from_slice(&buf0), Ioc::from_slice(&buf1)];
        let comp_iocs = [Ioc::from_slice(&comp0), Ioc::from_slice(&comp1)];
        let mut res_iocs = [IocMut::from_slice(res0), IocMut::from_slice(res1)];
        let buf_iocs_mr = [IocMr::from(&buf0_mr), IocMr::from(&buf1_mr)];
        let comp_iocs_mr = [IocMr::from(&comp0_mr), IocMr::from(&comp1_mr)];
        let mut res_iocs_mr = [IocMutMr::from(res0_mr), IocMutMr::from(res1_mr)];
        let mut buf_descs = [mr.description(), mr.description()];
        let mut comp_descs = [mr.description(), mr.description()];
        let mut res_descs = [mr.description(), mr.description()];
        let rma_ioc0 = RmaIoc::new(start, 128, ofi.remote_key.as_ref().unwrap());
        let rma_ioc1 = RmaIoc::new(start + 128, 128, ofi.remote_key.as_ref().unwrap());
        let rma_iocs = [rma_ioc0, rma_ioc1];

        let msg = match &ofi.ep {
            MyEndpoint::Connected(_) => {
                MsgType::ConnectedMsg(MsgCompareAtomicConnected::from_ioc_slice(
                    &buf_iocs,
                    &mut buf_descs,
                    &rma_iocs,
                    CompareAtomicOp::CswapGe,
                    0,
                ))
            }
            MyEndpoint::ConnectedMrLocal(_) => {
                MsgType::ConnectedMrMsg(MsgCompareAtomicConnectedMr::from_ioc_mr_slice(
                    &buf_iocs_mr,
                    &mut buf_descs,
                    &rma_iocs,
                    CompareAtomicOp::CswapGe,
                    0,
                ))
            }
            MyEndpoint::Connectionless(_) => {
                MsgType::ConnectionlessMsg(MsgCompareAtomic::from_ioc_slice(
                    &buf_iocs,
                    &mut buf_descs,
                    mapped_addr.as_ref().unwrap(),
                    &rma_iocs,
                    CompareAtomicOp::CswapGe,
                    0,
                ))
            }
            MyEndpoint::ConnectionlessMrLocal(_) => {
                MsgType::ConnectionlessMrMsg(MsgCompareAtomicMr::from_ioc_mr_slice(
                    &buf_iocs_mr,
                    &mut buf_descs,
                    mapped_addr.as_ref().unwrap(),
                    &rma_iocs,
                    CompareAtomicOp::CswapGe,
                    0,
                ))
            }
        };

        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.compare_atomicmsg(
                &msg,
                &comp_iocs,
                &mut res_iocs,
                &mut comp_descs,
                &mut res_descs,
            ),
            MyEndpoint::ConnectedMrLocal(_) => ofi.compare_atomicmsg_mr(
                &msg,
                &comp_iocs_mr,
                &mut res_iocs_mr,
                &mut comp_descs,
                &mut res_descs,
            ),
            MyEndpoint::Connectionless(_) => ofi.compare_atomicmsg(
                &msg,
                &comp_iocs,
                &mut res_iocs,
                &mut comp_descs,
                &mut res_descs,
            ),
            MyEndpoint::ConnectionlessMrLocal(_) => ofi.compare_atomicmsg_mr(
                &msg,
                &comp_iocs_mr,
                &mut res_iocs_mr,
                &mut comp_descs,
                &mut res_descs,
            ),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(res, &expected);
        // Send a done ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&ack_mem[..512], &mut desc, None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&ack_mem_mr.slice(..512), &mut desc, None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&ack_mem[..512], &mut desc, None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&ack_mem_mr.slice(..512), &mut desc, None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut ack_mem[..512], &mut desc),
            MyEndpoint::ConnectedMrLocal(_) => ofi.recv_mr(&mut ack_mem_mr.slice(..512), &mut desc),
            MyEndpoint::Connectionless(_) => ofi.recv(&mut ack_mem[..512], &mut desc),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut ack_mem_mr.slice(..512), &mut desc)
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        let expected = vec![2u8; 256];

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc)
            }
            MyEndpoint::Connectionless(_) => ofi.recv(&mut reg_mem[512..1024], &mut desc),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.recv_mr(&mut mr.slice(0).slice(512..1024), &mut desc)
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Connected(_) => ofi.send(&reg_mem[512..1024], &mut desc, None),
            MyEndpoint::ConnectedMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut desc, None)
            }
            MyEndpoint::Connectionless(_) => ofi.send(&reg_mem[512..1024], &mut desc, None),
            MyEndpoint::ConnectionlessMrLocal(_) => {
                ofi.send_mr(&mr.slice(0).slice(512..1024), &mut desc, None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    }
}

#[test]
fn compare_atomicmsg0() {
    compare_atomicmsg(true, "compare_atomicmsg0", false);
}

#[test]
fn compare_atomicmsg1() {
    compare_atomicmsg(false, "compare_atomicmsg0", false);
}

// [TODO Not sure why, but connected endpoints fail with atomic ops
// #[test]
// fn conn_compare_atomic0() {
//     compare_atomic(true, "conn_compare_atomic0", true);
// }

// #[test]
// fn conn_compare_atomic1() {
//     compare_atomic(false, "conn_compare_atomic0", true);
// }
