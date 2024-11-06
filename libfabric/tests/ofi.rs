use libfabric::{
    av::AddressVectorBuilder,
    comm::{
        atomic::{
            AtomicCASMrEp, AtomicFetchMrEp,
            AtomicWriteMrEp, ConnectedAtomicCASMrEp,
            ConnectedAtomicFetchMrEp, ConnectedAtomicWriteMrEp,
        },
        message::{
            ConnectedRecvEp, ConnectedRecvMrEp, ConnectedSendEp, ConnectedSendMrEp, RecvEp,
            RecvMrEp, SendEp, SendMrEp,
        },
        rma::{
            ConnectedReadMrEp, ConnectedWriteMrEp,
            ReadMrEp, WriteMrLocalEp,
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
    ep::{Address, BaseEndpoint, Endpoint, EndpointBuilder},
    eq::{EventQueueBuilder, WaitEq},
    error::{Error, ErrorKind},
    fabric::FabricBuilder,
    info::{Info, InfoEntry, Version},
    infocapsoptions::{
        AtomicDefaultCap, Caps, CollCap, InfoCaps, MsgDefaultCap, RmaDefaultCap, TagDefaultCap,
    },
    iovec::{IoVec, IoVecMr, IoVecMut, IoVecMutMr, IocMr, IocMutMr, RmaIoVec, RmaIoc},
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
    Uninit,
}

impl<L, R, LL, RR> MsgType<L, R, LL, RR> {
    pub fn conless(&self) -> &L {
        if let Self::ConnectionlessMsg(msg) = self {
            msg
        } else {
            panic!("Not ConnectionlessMsg")
        }
    }

    pub fn conless_mr(&self) -> &LL {
        if let Self::ConnectionlessMrMsg(msg) = self {
            msg
        } else {
            panic!("Not ConnectionlessMrMsg")
        }
    }

    pub fn conned(&self) -> &R {
        if let Self::ConnectedMsg(msg) = self {
            msg
        } else {
            panic!("Not ConnectedMsg")
        }
    }

    pub fn conned_mr(&self) -> &RR {
        if let Self::ConnectedMrMsg(msg) = self {
            msg
        } else {
            panic!("Not ConnectedMrMsg")
        }
    }
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

pub enum MrEp<I> {
    Connected(ConnectedMrLocalEndpoint<I>),
    Connectless(ConnectionlessMrLocalEndpoint<I>, MappedAddress),
}

impl<I: TagDefaultCap> MrEp<I> {
    pub(crate) fn tinject<T: Copy>(
        &self,
        buf: &MemoryRegionSlice<T>,
        tag: u64,
        data: Option<u64>,
    ) -> Result<(), Error> {
        match self {
            MrEp::Connected(ep) => match data {
                Some(data) => ep.tinjectdata(buf, data, tag),
                None => ep.tinject(buf, tag),
            },
            MrEp::Connectless(ep, address) => match data {
                Some(data) => ep.tinjectdata_to(buf, data, address, tag),
                None => ep.tinject_to(buf, address, tag),
            },
        }
    }

    pub(crate) fn tsend<T: Copy>(
        &self,
        buf: &MemoryRegionSlice<T>,
        desc: &mut MemoryRegionDesc,
        tag: u64,
        data: Option<u64>,
    ) -> Result<(), Error> {
        match self {
            MrEp::Connected(ep) => match data {
                Some(data) => ep.tsenddata(buf, desc, data, tag),
                None => ep.tsend(buf, desc, tag),
            },
            MrEp::Connectless(ep, address) => match data {
                Some(data) => ep.tsenddata_to(buf, desc, data, address, tag),
                None => ep.tsend_to(buf, desc, address, tag),
            },
        }
    }
}

impl<I: MsgDefaultCap> MrEp<I> {
    pub(crate) fn inject<T: Copy>(
        &self,
        buf: &MemoryRegionSlice<T>,
        data: Option<u64>,
    ) -> Result<(), Error> {
        match self {
            MrEp::Connected(ep) => match data {
                Some(data) => ep.injectdata(buf, data),
                None => ep.inject(buf),
            },
            MrEp::Connectless(ep, address) => match data {
                Some(data) => ep.injectdata_to(buf, data, address),
                None => ep.inject_to(buf, address),
            },
        }
    }

    pub(crate) fn send<T: Copy>(
        &self,
        buf: &MemoryRegionSlice<T>,
        desc: &mut MemoryRegionDesc,
        data: Option<u64>,
        ctx: Option<&mut Context>,
    ) -> Result<(), Error> {
        match self {
            MrEp::Connected(ep) => match data {
                Some(data) => {
                    if let Some(ctx) = ctx {
                        ep.senddata_with_context(buf, desc, data, ctx)
                    } else {
                        ep.senddata(buf, desc, data)
                    }
                }
                None => {
                    if let Some(ctx) = ctx {
                        ep.send_with_context(buf, desc, ctx)
                    } else {
                        ep.send(buf, desc)
                    }
                }
            },
            MrEp::Connectless(ep, address) => match data {
                Some(data) => {
                    if let Some(ctx) = ctx {
                        ep.senddata_to_with_context(buf, desc, data, address, ctx)
                    } else {
                        ep.senddata_to(buf, desc, data, address)
                    }
                }
                None => {
                    if let Some(ctx) = ctx {
                        ep.send_to_with_context(buf, desc, address, ctx)
                    } else {
                        ep.send_to(buf, desc, address)
                    }
                }
            },
        }
    }
}

impl<I: RmaDefaultCap> MrEp<I> {
    pub(crate) unsafe fn write_inject<T: Copy>(
        &self,
        buf: &MemoryRegionSlice<T>,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        data: Option<u64>,
    ) -> Result<(), Error> {
        match self {
            MrEp::Connected(ep) => match data {
                Some(data) => ep.inject_writedata(buf, data, mem_addr, mapped_key),
                None => ep.inject_write(buf, mem_addr, mapped_key),
            },
            MrEp::Connectless(ep, address) => match data {
                Some(data) => ep.inject_writedata_to(buf, data, address, mem_addr, mapped_key),
                None => ep.inject_write_to(buf, address, mem_addr, mapped_key),
            },
        }
    }

    pub(crate) unsafe fn write<T: Copy>(
        &self,
        buf: &MemoryRegionSlice<T>,
        desc: &mut MemoryRegionDesc,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        data: Option<u64>,
    ) -> Result<(), Error> {
        match self {
            MrEp::Connected(ep) => match data {
                Some(data) => ep.writedata(buf, desc, data, mem_addr, mapped_key),
                None => ep.write(buf, desc, mem_addr, mapped_key),
            },
            MrEp::Connectless(ep, address) => match data {
                Some(data) => ep.writedata_to(buf, desc, data, address, mem_addr, mapped_key),
                None => ep.write_to(buf, desc, address, mem_addr, mapped_key),
            },
        }
    }
}

pub enum PlainEp<I> {
    Connected(ConnectedEndpoint<I>),
    Connectless(ConnectionlessEndpoint<I>, MappedAddress),
}

impl<I: MsgDefaultCap> PlainEp<I> {
    pub(crate) fn inject<T: Copy>(
        &self,
        buf: &[T],
        data: Option<u64>,
    ) -> Result<(), Error> {
        match self {
            PlainEp::Connected(ep) => match data {
                Some(data) => ep.injectdata(buf, data),
                None => ep.inject(buf),
            },
            PlainEp::Connectless(ep, address) => match data {
                Some(data) => ep.injectdata_to(buf, data, address),
                None => ep.inject_to(buf, address),
            },
        }
    }

    pub(crate) fn send<T: Copy>(
        &self,
        buf: &[T],
        desc: &mut MemoryRegionDesc,
        data: Option<u64>,
        ctx: Option<&mut Context>,
    ) -> Result<(), Error> {
        match self {
            PlainEp::Connected(ep) => match data {
                Some(data) => {
                    if let Some(ctx) = ctx {
                        ep.senddata_with_context(buf, desc, data, ctx)
                    } else {
                        ep.senddata(buf, desc, data)
                    }
                }
                None => {
                    if let Some(ctx) = ctx {
                        ep.send_with_context(buf, desc, ctx)
                    } else {
                        ep.send(buf, desc)
                    }
                }
            },
            PlainEp::Connectless(ep, address) => match data {
                Some(data) => {
                    if let Some(ctx) = ctx {
                        ep.senddata_to_with_context(buf, desc, data, address, ctx)
                    } else {
                        ep.senddata_to(buf, desc, data, address)
                    }
                }
                None => {
                    if let Some(ctx) = ctx {
                        ep.send_to_with_context(buf, desc, address, ctx)
                    } else {
                        ep.send_to(buf, desc, address)
                    }
                }
            },
        }
    }
}

impl<I: TagDefaultCap> PlainEp<I> {
    pub(crate) fn tinject<T: Copy>(
        &self,
        buf: &[T],
        tag: u64,
        data: Option<u64>,
    ) -> Result<(), Error> {
        match self {
            PlainEp::Connected(ep) => match data {
                Some(data) => ep.tinjectdata(buf, data, tag),
                None => ep.tinject(buf, tag),
            },
            PlainEp::Connectless(ep, address) => match data {
                Some(data) => ep.tinjectdata_to(buf, data, address, tag),
                None => ep.tinject_to(buf, address, tag),
            },
        }
    }

    pub(crate) fn tsend<T: Copy>(
        &self,
        buf: &[T],
        tag: u64,
        data: Option<u64>,
    ) -> Result<(), Error> {
        match self {
            PlainEp::Connected(ep) => match data {
                Some(data) => ep.tsenddata(buf, data, tag),
                None => ep.tsend(buf, tag),
            },
            PlainEp::Connectless(ep, address) => match data {
                Some(data) => ep.tsenddata_to(buf, data, address, tag),
                None => ep.tsend_to(buf, address, tag),
            },
        }
    }
}
pub enum ConnectedEp<I> {
    Plain(ConnectedEndpoint<I>),
    Mr(ConnectedMrLocalEndpoint<I>),
}
pub enum TempConnectlessEp<I> {
    Plain(ConnectionlessEndpoint<I>),
    Mr(ConnectionlessMrLocalEndpoint<I>, MemoryRegion),
}

pub enum ConnectlessEp<I> {
    Plain(ConnectionlessEndpoint<I>),
    Mr(ConnectionlessMrLocalEndpoint<I>),
}

pub enum MyEndpoint<I> {
    Mr(MrEp<I>),
    Plain(PlainEp<I>),
}

pub struct Ofi<I> {
    pub info_entry: InfoEntry<I>,
    pub remote_key: Option<MappedMemoryRegionKey>,
    pub remote_mem_addr: Option<(u64, u64)>,
    pub domain: Domain,
    pub cq_type: CqType,
    pub ep: MyEndpoint<I>,
    // pub tx_pending_cnt: AtomicUsize,
    // pub tx_complete_cnt: AtomicUsize,
    // pub rx_pending_cnt: AtomicUsize,
    // pub rx_complete_cnt: AtomicUsize,
}

impl<I> Drop for Ofi<I> {
    fn drop(&mut self) {
        match self.ep {
            MyEndpoint::Mr(ref mr_ep) => match mr_ep {
                MrEp::Connected(conn_ep) => conn_ep.shutdown().unwrap(),
                MrEp::Connectless(_, _) => {}
            },
            MyEndpoint::Plain(ref plain_ep) => match plain_ep {
                PlainEp::Connected(conn_ep) => conn_ep.shutdown().unwrap(),
                PlainEp::Connectless(_, _) => {}
            },
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

        // let mut tx_pending_cnt: usize = 0;
        // let mut tx_complete_cnt: usize = 0;
        // let mut rx_pending_cnt: usize = 0;
        // let mut rx_complete_cnt: usize = 0;

        let eq = EventQueueBuilder::new(&fabric).build().unwrap();
        let info_entry = match ep_type {
            EndpointType::Msg | EndpointType::SockStream => {
                if server {
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
                }
            }
            _ => info_entry,
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
            Endpoint::ConnectionOriented(unconnected_ep) => {
                unconnected_ep.bind_eq(&eq).unwrap();
                match cq_type {
                    CqType::Separate((ref tx_cq, ref rx_cq)) => unconnected_ep
                        .bind_separate_cqs(tx_cq, false, rx_cq, false)
                        .unwrap(),
                    CqType::Shared(ref scq) => unconnected_ep.bind_shared_cq(&scq, false).unwrap(),
                }
                match unconnected_ep.enable().unwrap() {
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

                        MyEndpoint::Plain(PlainEp::Connected(ep))
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
                        MyEndpoint::Mr(MrEp::Connected(ep))
                    }
                }
            }
            Endpoint::Connectionless(connless_ep) => {
                let mut mem = vec![0u8; 1024 * 1024];
                let epname = connless_ep.getname().unwrap();
                let epname_bytes = epname.as_bytes();
                let addrlen = epname_bytes.len();

                match cq_type {
                    CqType::Separate((ref tx_cq, ref rx_cq)) => connless_ep
                        .bind_separate_cqs(tx_cq, false, rx_cq, false)
                        .unwrap(),
                    CqType::Shared(ref scq) => connless_ep.bind_shared_cq(&scq, false).unwrap(),
                }

                let av = match info_entry.domain_attr().av_type() {
                    libfabric::enums::AddressVectorType::Unspec => AddressVectorBuilder::new(),
                    _ => AddressVectorBuilder::new().type_(*info_entry.domain_attr().av_type()),
                }
                .build(&domain)
                .unwrap();
                connless_ep.bind_av(&av).unwrap();

                let ep = match connless_ep.enable().unwrap() {
                    libfabric::connless_ep::ConnectionlessEndpointB::PlainData(ep) => {
                        TempConnectlessEp::Plain(ep)
                    }
                    libfabric::connless_ep::ConnectionlessEndpointB::MrLocalData(ep) => {
                        let mr =
                            MemoryRegionBuilder::new(&mut mem, libfabric::enums::HmemIface::System)
                                .access_read()
                                .access_write()
                                .access_send()
                                .access_recv()
                                .build(&domain)?;

                        let mr = match mr {
                            libfabric::mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
                            libfabric::mr::MaybeDisabledMemoryRegion::Disabled(mr) => {
                                mr.bind_ep(&ep).unwrap();
                                mr.enable().unwrap()
                            }
                        };
                        TempConnectlessEp::Mr(ep, mr)
                    }
                };

                if let Some(dest_addr) = info_entry.dest_addr() {
                    let mapped_address = av
                        .insert(std::slice::from_ref(dest_addr).into(), AVOptions::new())
                        .unwrap()
                        .pop()
                        .unwrap()
                        .unwrap();

                    let addrlen = epname_bytes.len();
                    mem[..addrlen].copy_from_slice(epname_bytes);

                    let ep = match ep {
                        TempConnectlessEp::Plain(ep) => {
                            post!(
                                send_to,
                                ft_progress,
                                cq_type.tx_cq(),
                                ep,
                                &mem[..addrlen],
                                &mut default_desc(),
                                &mapped_address
                            );

                            cq_type.tx_cq().sread(1, -1).unwrap();

                            post!(
                                recv_from_any,
                                ft_progress,
                                cq_type.rx_cq(),
                                ep,
                                &mut mem[0..1]
                            );

                            cq_type.rx_cq().sread(1, -1).unwrap();
                            MyEndpoint::Plain(PlainEp::Connectless(ep, mapped_address))
                        }
                        TempConnectlessEp::Mr(ep, mr) => {
                            let mut mr_desc = mr.description();

                            post!(
                                send_to,
                                ft_progress,
                                cq_type.tx_cq(),
                                ep,
                                &mr.slice(0).slice(..addrlen),
                                &mut mr_desc,
                                &mapped_address
                            );
                            cq_type.tx_cq().sread(1, -1).unwrap();

                            post!(
                                recv_from_any,
                                ft_progress,
                                cq_type.rx_cq(),
                                ep,
                                &mut mr.slice(0).slice(0..1),
                                &mut mr_desc
                            );

                            cq_type.rx_cq().sread(1, -1).unwrap();
                            MyEndpoint::Mr(MrEp::Connectless(ep, mapped_address))
                        }
                    };

                    ep
                } else {
                    match ep {
                        TempConnectlessEp::Plain(ref ep) => {
                            post!(
                                recv_from_any,
                                ft_progress,
                                cq_type.rx_cq(),
                                ep,
                                &mut mem[0..addrlen]
                            );

                            cq_type.rx_cq().sread(1, -1).unwrap();
                        }
                        TempConnectlessEp::Mr(ref ep, ref mr) => {
                            let mut mr_desc = mr.description();
                            post!(
                                recv_from_any,
                                ft_progress,
                                cq_type.rx_cq(),
                                ep,
                                &mut mr.slice(0).slice(0..addrlen),
                                &mut mr_desc
                            );

                            cq_type.rx_cq().sread(1, -1).unwrap();
                        }
                    }

                    let remote_address = unsafe { Address::from_bytes(&mem) };
                    let mapped_address = av
                        .insert(
                            std::slice::from_ref(&remote_address).into(),
                            AVOptions::new(),
                        )
                        .unwrap()
                        .pop()
                        .unwrap()
                        .unwrap();

                    let ep = match ep {
                        TempConnectlessEp::Plain(ep) => {
                            post!(
                                send_to,
                                ft_progress,
                                cq_type.tx_cq(),
                                ep,
                                &mem[..1],
                                &mut default_desc(),
                                &mapped_address
                            );

                            cq_type.tx_cq().sread(1, -1).unwrap();

                            MyEndpoint::Plain(PlainEp::Connectless(ep, mapped_address))
                        }
                        TempConnectlessEp::Mr(ep, mr) => {
                            let mut mr_desc = mr.description();

                            post!(
                                send_to,
                                ft_progress,
                                cq_type.tx_cq(),
                                ep,
                                &mr.slice(0).slice(..1),
                                &mut mr_desc,
                                &mapped_address
                            );
                            cq_type.tx_cq().sread(1, -1).unwrap();
                            MyEndpoint::Mr(MrEp::Connectless(ep, mapped_address))
                        }
                    };

                    ep
                }
            } 
        };
        if server {
            unsafe { std::env::remove_var(name) };
        }

        Ok(Self {
            info_entry,
            remote_key: None,
            remote_mem_addr: None,
            cq_type,
            domain,
            ep,
        })
    }
}

impl<I: TagDefaultCap> Ofi<I> {
    pub fn tsend<T: Copy>(
        &self,
        ep: &PlainEp<I>,
        buf: &[T],
        tag: u64,
        data: Option<u64>,
    ) {
        loop {
            let err = if buf.len() <= self.info_entry.tx_attr().inject_size() {
                ep.tinject(buf, tag, data)
            } else {
                ep.tsend(buf, tag, data)
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
        ep: &MrEp<I>,
        buf: &MemoryRegionSlice<T>,
        desc: &mut MemoryRegionDesc,
        tag: u64,
        data: Option<u64>,
    ) {
        loop {
            let err = if buf.len() <= self.info_entry.tx_attr().inject_size() {
                ep.tinject(buf, tag, data)
            } else {
                ep.tsend(buf, desc, tag, data)
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

    pub fn tsendv(&self, ep: &PlainEp<I>, iov: &[IoVec], tag: u64) {
        loop {
            let err = match ep {
                PlainEp::Connectless(ep, mapped_addr) => ep.tsendv_to(iov, mapped_addr, tag),
                PlainEp::Connected(ep) => ep.tsendv(iov, tag),
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

    pub fn tsendv_mr(
        &self,
        ep: &MrEp<I>,
        iov: &[IoVecMr],
        desc: &mut [MemoryRegionDesc],
        tag: u64,
    ) {
        loop {
            let err = match &ep {
                MrEp::Connectless(ep, mapped_address) => {
                    ep.tsendv_to(iov, desc, mapped_address, tag)
                }
                MrEp::Connected(ep) => ep.tsendv(iov, desc, tag),
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

    pub fn trecvv(
        &self,
        ep: &PlainEp<I>,
        iov: &[IoVecMut],
        tag: u64,
    ) {
        loop {
            let err = match ep {
                PlainEp::Connectless(ep, mapped_addr) => ep.trecvv_from(iov, mapped_addr, tag, 0),
                PlainEp::Connected(ep) => ep.trecvv(iov, 0, tag),
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

    pub fn trecvv_mr(
        &self,
        ep: &MrEp<I>,
        iov: &[IoVecMutMr],
        desc: &mut [MemoryRegionDesc],
        tag: u64,
    ) {
        loop {
            let err = match ep {
                MrEp::Connectless(ep, mapped_addr) => {
                    ep.trecvv_from(iov, desc, mapped_addr, tag, 0)
                }
                MrEp::Connected(ep) => ep.trecvv(iov, desc, 0, tag),
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

    pub fn trecv<T>(&self, ep: &PlainEp<I>, buf: &mut [T], tag: u64) {
        loop {
            let err = match ep {
                PlainEp::Connectless(ep, mapped_addr) => ep.trecv_from(buf, mapped_addr, tag, 0),
                PlainEp::Connected(ep) => ep.trecv(buf, tag, 0),
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
        &self,
        ep: &MrEp<I>,
        buf: &mut MemoryRegionSlice<T>,
        desc: &mut MemoryRegionDesc,
        tag: u64,
    ) {
        loop {
            let err = match ep {
                MrEp::Connectless(ep, mapped_addr) => ep.trecv_from(buf, desc, mapped_addr, tag, 0),
                MrEp::Connected(ep) => ep.trecv(buf, desc, tag, 0),
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

    pub fn tsendmsg<'a>(
        &self,
        msg: &mut MsgType<
            MsgTagged<'a>,
            MsgTaggedConnected<'a>,
            MsgTaggedMr<'a>,
            MsgTaggedConnectedMr<'a>,
        >,
        ep: &'a PlainEp<I>,
        iov: &'a [IoVec],
        desc: &'a mut [MemoryRegionDesc],
        tag: u64,
        data: u64,
    ) {
        let opts = TferOptions::new().remote_cq_data();
        match ep {
            PlainEp::Connected(ep) => {
                *msg = MsgType::ConnectedMsg(MsgTaggedConnected::from_iov_slice(
                    iov, desc, data, tag, 0,
                ));
                loop {
                    let err = ep.tsendmsg(&msg.conned(), opts);

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
            PlainEp::Connectless(ep, mapped_addr) => {
                *msg = MsgType::ConnectionlessMsg(MsgTagged::from_iov_slice(
                    iov,
                    desc,
                    mapped_addr,
                    data,
                    tag,
                    0,
                ));
                loop {
                    let err = ep.tsendmsg_to(&msg.conless(), opts);

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
    }

    pub fn tsendmsg_mr<'a>(
        &self,
        msg: &mut MsgType<MsgTagged, MsgTaggedConnected, MsgTaggedMr<'a>, MsgTaggedConnectedMr<'a>>,
        ep: &'a MrEp<I>,
        iov: &'a [IoVecMr<'a>],
        desc: &'a mut [MemoryRegionDesc],
        tag: u64,
        data: u64,
    ) {
        let opts = TferOptions::new().remote_cq_data();
        match ep {
            MrEp::Connected(ep) => {
                *msg = MsgType::ConnectedMrMsg(MsgTaggedConnectedMr::from_iov_slice(
                    iov, desc, data, tag, 0,
                ));
                loop {
                    let err = ep.tsendmsg(msg.conned_mr(), opts);

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
            MrEp::Connectless(ep, mapped_addr) => {
                *msg = MsgType::ConnectionlessMrMsg(MsgTaggedMr::from_iov_slice(
                    iov,
                    desc,
                    mapped_addr,
                    data,
                    tag,
                    0,
                ));
                loop {
                    let err = ep.tsendmsg_to(&msg.conless_mr(), opts);

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
    }
    // }

    pub fn trecvmsg_mr<'a>(
        &self,
        msg: &mut MsgType<
            MsgTaggedMut<'a>,
            MsgTaggedConnectedMut<'a>,
            MsgTaggedMutMr<'a>,
            MsgTaggedConnectedMutMr<'a>,
        >,
        ep: &'a MrEp<I>,
        iov: &'a mut [IoVecMutMr],
        desc: &'a mut [MemoryRegionDesc],
        tag: u64,
        _data: u64,
    ) {
        let opts = TferOptions::new();
        match ep {
            MrEp::Connected(ep) => {
                *msg = MsgType::ConnectedMrMsg(MsgTaggedConnectedMutMr::from_iov_mr_slice(
                    iov, desc, tag, 0,
                ));
                loop {
                    let err = ep.trecvmsg(msg.conned_mr(), opts);

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
            MrEp::Connectless(ep, mapped_addr) => {
                *msg = MsgType::ConnectionlessMrMsg(MsgTaggedMutMr::from_iov_mr_slice(
                    iov,
                    desc,
                    mapped_addr,
                    tag,
                    0,
                ));
                loop {
                    let err = ep.trecvmsg_from(&msg.conless_mr(), opts);

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
    }

    pub fn trecvmsg<'a>(
        &self,
        msg: &mut MsgType<
            MsgTaggedMut<'a>,
            MsgTaggedConnectedMut<'a>,
            MsgTaggedMutMr,
            MsgTaggedConnectedMutMr,
        >,
        ep: &'a PlainEp<I>,
        iov: &'a mut [IoVecMut<'a>],
        desc: &'a mut [MemoryRegionDesc],
        tag: u64,
        _data: u64,
    ) {
        let opts = TferOptions::new();
        match ep {
            PlainEp::Connected(ep) => {
                *msg =
                    MsgType::ConnectedMsg(MsgTaggedConnectedMut::from_iov_slice(iov, desc, tag, 0));
                loop {
                    let err = ep.trecvmsg(&msg.conned(), opts);

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
            PlainEp::Connectless(ep, mapped_addr) => {
                *msg = MsgType::ConnectionlessMsg(MsgTaggedMut::from_iov_slice(
                    iov,
                    desc,
                    mapped_addr,
                    tag,
                    0,
                ));
                loop {
                    let err = ep.trecvmsg_from(&msg.conless(), opts);

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
    }
}

impl<I: MsgDefaultCap + 'static> Ofi<I> {
    pub fn send<T: Copy>(
        &self,
        ep: &PlainEp<I>,
        buf: &[T],
        desc: &mut MemoryRegionDesc,
        data: Option<u64>,
    ) {
        loop {
            let err = if buf.len() <= self.info_entry.tx_attr().inject_size() {
                ep.inject(buf, data)
            } else {
                ep.send(buf, desc, data, None)
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
        ep: &MrEp<I>,
        buf: &MemoryRegionSlice<T>,
        desc: &mut MemoryRegionDesc,
        data: Option<u64>,
    ) {
        loop {
            let err = if buf.len() <= self.info_entry.tx_attr().inject_size() {
                ep.inject(buf, data)
            } else {
                ep.send(buf, desc, data, None)
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

    pub fn send_with_context<T: Copy>(
        &self,
        ep: &PlainEp<I>,
        buf: &[T],
        desc: &mut MemoryRegionDesc,
        data: Option<u64>,
        context: &mut Context,
    ) {
        loop {
            let err = if buf.len() <= self.info_entry.tx_attr().inject_size() {
                ep.inject(buf, data)
            } else {
                ep.send(buf, desc, data, Some(context))
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
        ep: &MrEp<I>,
        buf: &MemoryRegionSlice<T>,
        desc: &mut MemoryRegionDesc,
        data: Option<u64>,
        context: &mut Context,
    ) {
        loop {
            let err = if buf.len() <= self.info_entry.tx_attr().inject_size() {
                ep.inject(buf, data)
            } else {
                ep.send(buf, desc, data, Some(context))
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

    pub fn sendv(&self, ep: &PlainEp<I>, iov: &[IoVec], desc: &mut [MemoryRegionDesc]) {
        loop {
            let err = match ep {
                PlainEp::Connectless(ep, mapped_addr) => ep.sendv_to(iov, desc, mapped_addr),
                PlainEp::Connected(ep) => ep.sendv(iov, desc),
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

    pub fn sendv_mr(&self, ep: &MrEp<I>, iov: &[IoVecMr], desc: &mut [MemoryRegionDesc]) {
        loop {
            let err = match ep {
                MrEp::Connectless(ep, mapped_addr) => ep.sendv_to(iov, desc, mapped_addr),
                MrEp::Connected(ep) => ep.sendv(iov, desc),
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

    pub fn sendmsg_mr<'a>(
        &self,
        msg: &mut MsgType<Msg, MsgConnected, MsgMr, MsgConnectedMr<'a>>,
        ep: &MrEp<I>,
        iov: &'a [IoVecMr<'a>],
        desc: &'a mut [MemoryRegionDesc],
        data: u64,
    ) {
        let opts = TferOptions::new().remote_cq_data();
        match ep {
            MrEp::Connected(ep) => {
                *msg = MsgType::ConnectedMrMsg(MsgConnectedMr::from_iov_mr_slice(iov, desc, data));
                loop {
                    let err = ep.sendmsg(msg.conned_mr(), opts);

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
            MrEp::Connectless(ep, mapped_addr) => {
                let msg = MsgMr::from_iov_mr_slice(iov, desc, mapped_addr, data);
                loop {
                    let err = ep.sendmsg_to(&msg, opts);

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
    }

    pub fn sendmsg<'a>(
        &self,
        msg: &mut MsgType<Msg<'a>, MsgConnected<'a>, MsgMr<'a>, MsgConnectedMr<'a>>,
        ep: &'a PlainEp<I>,
        iov: &'a [IoVec],
        desc: &'a mut [MemoryRegionDesc],
        data: u64,
    ) {
        let opts = TferOptions::new().remote_cq_data();
        match ep {
            PlainEp::Connected(ep) => {
                *msg = MsgType::ConnectedMsg(MsgConnected::from_iov_slice(iov, desc, data));
                loop {
                    let err = ep.sendmsg(&msg.conned(), opts);

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
            PlainEp::Connectless(ep, mapped_addr) => {
                *msg =
                    MsgType::ConnectionlessMsg(Msg::from_iov_slice(iov, desc, mapped_addr, data));
                loop {
                    let err = ep.sendmsg_to(&msg.conless(), opts);

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
    }

    pub fn recvmsg_mr<'a>(
        &self,
        msg: &mut MsgType<MsgMut<'a>, MsgConnectedMut<'a>, MsgMutMr<'a>, MsgConnectedMutMr<'a>>,
        ep: &'a MrEp<I>,
        iov: &'a mut [IoVecMutMr],
        desc: &'a mut [MemoryRegionDesc],
        _data: u64,
    ) {
        let opts = TferOptions::new();
        match ep {
            MrEp::Connected(ep) => {
                *msg = MsgType::ConnectedMrMsg(MsgConnectedMutMr::from_iov_mr_slice(iov, desc));
                loop {
                    let err = ep.recvmsg(msg.conned_mr(), opts);

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
            MrEp::Connectless(ep, mapped_addr) => {
                *msg = MsgType::ConnectionlessMrMsg(MsgMutMr::from_iov_mr_slice(
                    iov,
                    desc,
                    mapped_addr,
                ));
                loop {
                    let err = ep.recvmsg_from(&msg.conless_mr(), opts);

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
    }

    pub fn recvmsg<'a>(
        &self,
        msg: &mut MsgType<MsgMut<'a>, MsgConnectedMut<'a>, MsgMutMr, MsgConnectedMutMr>,
        ep: &'a PlainEp<I>,
        iov: &'a mut [IoVecMut<'a>],
        desc: &'a mut [MemoryRegionDesc],
        _data: u64,
    ) {
        let opts = TferOptions::new();
        match ep {
            PlainEp::Connected(ep) => {
                *msg = MsgType::ConnectedMsg(MsgConnectedMut::from_iov_slice(iov, desc));
                loop {
                    let err = ep.recvmsg(&msg.conned(), opts);

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
            PlainEp::Connectless(ep, mapped_addr) => {
                *msg = MsgType::ConnectionlessMsg(MsgMut::from_iov_slice(iov, desc, mapped_addr));
                loop {
                    let err = ep.recvmsg_from(&msg.conless(), opts);

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
    }

    pub fn recvv(&self, ep: &PlainEp<I>, iov: &[IoVecMut]) {
        loop {
            let err = match ep {
                PlainEp::Connectless(ep, mapped_addr) => ep.recvv_from(iov, mapped_addr),
                PlainEp::Connected(ep) => ep.recvv(iov),
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

    pub fn recvv_mr(&self, ep: &MrEp<I>, iov: &[IoVecMutMr], desc: &mut [MemoryRegionDesc]) {
        loop {
            let err = match ep {
                MrEp::Connectless(ep, mapped_addr) => ep.recvv_from(iov, desc, mapped_addr),
                MrEp::Connected(ep) => ep.recvv(iov, desc),
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

    pub fn recv<T>(&self, ep: &PlainEp<I>, buf: &mut [T]) {
        loop {
            let err = match ep {
                PlainEp::Connectless(ep, mapped_addr) => ep.recv_from(buf, mapped_addr),
                PlainEp::Connected(ep) => ep.recv(buf),
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
        &self,
        ep: &MrEp<I>,
        buf: &mut MemoryRegionSlice<T>,
        desc: &mut MemoryRegionDesc,
    ) {
        loop {
            let err = match ep {
                MrEp::Connectless(ep, mapped_addr) => ep.recv_from(buf, desc, mapped_addr),
                MrEp::Connected(ep) => ep.recv(buf, desc),
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
        match &self.ep {
            MyEndpoint::Plain(ep) => {
                self.send(
                    ep,
                    &reg_mem[..key_bytes.len() + 2 * std::mem::size_of::<usize>()],
                    &mut desc,
                    None,
                );
                self.recv(
                    ep,
                    &mut reg_mem[key_bytes.len() + 2 * std::mem::size_of::<usize>()
                        ..2 * key_bytes.len() + 4 * std::mem::size_of::<usize>()],
                );
            }
            MyEndpoint::Mr(ep) => {
                self.send_mr(
                    ep,
                    &mr.slice(0)
                        .slice(..key_bytes.len() + 2 * std::mem::size_of::<usize>()),
                    &mut desc,
                    None,
                );
                self.recv_mr(
                    ep,
                    &mut mr.slice(0).slice(
                        key_bytes.len() + 2 * std::mem::size_of::<usize>()
                            ..2 * key_bytes.len() + 4 * std::mem::size_of::<usize>(),
                    ),
                    &mut desc,
                );
            }
        }

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
    // pub fn write<T>(
    //     &mut self,
    //     ep: &PlainEp<I>,
    //     buf: &[T],
    //     dest_addr: u64,
    //     desc: &mut MemoryRegionDesc,
    //     data: Option<u64>,
    // ) {
    //     let (start, _end) = self.remote_mem_addr.unwrap();
    //     loop {
    //         let err =
    //                 if buf.len() <= self.info_entry.tx_attr().inject_size() {
    //                     ep.write_inject
    //                     } else {
    //                         unsafe {
    //                             ep.inject_write_to(
    //                                 buf,
    //                                 self.mapped_addr.as_ref().unwrap(),
    //                                 start + dest_addr,
    //                                 self.remote_key.as_ref().unwrap(),
    //                             )
    //                         }
    //                     }
    //                 } else {
    //                     if data.is_some() {
    //                         unsafe {
    //                             ep.writedata_to(
    //                                 buf,
    //                                 desc,
    //                                 data.unwrap(),
    //                                 self.mapped_addr.as_ref().unwrap(),
    //                                 start + dest_addr,
    //                                 self.remote_key.as_ref().unwrap(),
    //                             )
    //                         }
    //                     } else {
    //                         unsafe {
    //                             ep.write_to(
    //                                 buf,
    //                                 desc,
    //                                 self.mapped_addr.as_ref().unwrap(),
    //                                 start + dest_addr,
    //                                 self.remote_key.as_ref().unwrap(),
    //                             )
    //                         }
    //                     }
    //                 }
    //             }
    //             MyEndpoint::Connected(ep) => {
    //                 if buf.len() <= self.info_entry.tx_attr().inject_size() {
    //                     if data.is_some() {
    //                         unsafe {
    //                             ep.inject_writedata(
    //                                 buf,
    //                                 data.unwrap(),
    //                                 start + dest_addr,
    //                                 self.remote_key.as_ref().unwrap(),
    //                             )
    //                         }
    //                     } else {
    //                         unsafe {
    //                             ep.inject_write(
    //                                 buf,
    //                                 start + dest_addr,
    //                                 self.remote_key.as_ref().unwrap(),
    //                             )
    //                         }
    //                     }
    //                 } else {
    //                     if data.is_some() {
    //                         unsafe {
    //                             ep.writedata(
    //                                 buf,
    //                                 desc,
    //                                 data.unwrap(),
    //                                 start + dest_addr,
    //                                 self.remote_key.as_ref().unwrap(),
    //                             )
    //                         }
    //                     } else {
    //                         unsafe {
    //                             ep.write(
    //                                 buf,
    //                                 desc,
    //                                 start + dest_addr,
    //                                 self.remote_key.as_ref().unwrap(),
    //                             )
    //                         }
    //                     }
    //                 }
    //             }
    //             _ => panic!("Plain data only"),
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }

    pub fn write_mr<T: Copy>(
        &self,
        ep: &MrEp<I>,
        buf: &MemoryRegionSlice<T>,
        dest_addr: u64,
        desc: &mut MemoryRegionDesc,
        data: Option<u64>,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = if buf.len() <= self.info_entry.tx_attr().inject_size() {
                unsafe {
                    ep.write_inject(
                        buf,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        data,
                    )
                }
            } else {
                unsafe {
                    ep.write(
                        buf,
                        desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        data,
                    )
                }
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

    // pub fn read<T>(&mut self, buf: &mut [T], dest_addr: u64, desc: &mut MemoryRegionDesc) {
    //     let (start, _end) = self.remote_mem_addr.unwrap();

    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::Connectionless(ep) => unsafe {
    //                 ep.read_from(
    //                     buf,
    //                     desc,
    //                     self.mapped_addr.as_ref().unwrap(),
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                 )
    //             },
    //             MyEndpoint::Connected(ep) => unsafe {
    //                 ep.read(
    //                     buf,
    //                     desc,
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                 )
    //             },
    //             _ => panic!("Plain data only"),
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }

    pub fn read_mr<T: Copy>(
        &self,
        ep: &MrEp<I>,
        buf: &mut MemoryRegionSlice<T>,
        dest_addr: u64,
        desc: &mut MemoryRegionDesc,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();

        loop {
            let err = match ep {
                MrEp::Connectless(ep, address) => unsafe {
                    ep.read_from(
                        buf,
                        desc,
                        address,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                MrEp::Connected(ep) => unsafe {
                    ep.read(
                        buf,
                        desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
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

    // pub fn writev(&mut self, iov: &[IoVec], dest_addr: u64, desc: &mut [MemoryRegionDesc]) {
    //     let (start, _end) = self.remote_mem_addr.unwrap();
    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::Connectionless(ep) => unsafe {
    //                 ep.writev_to(
    //                     iov,
    //                     desc,
    //                     self.mapped_addr.as_ref().unwrap(),
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                 )
    //             },
    //             MyEndpoint::Connected(ep) => unsafe {
    //                 ep.writev(
    //                     iov,
    //                     desc,
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                 )
    //             },
    //             _ => panic!("Plain data only"),
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }

    pub fn writev_mr(
        &self,
        ep: &MrEp<I>,
        iov: &[IoVecMr],
        dest_addr: u64,
        desc: &mut [MemoryRegionDesc],
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match ep {
                MrEp::Connectless(ep, address) => unsafe {
                    ep.writev_to(
                        iov,
                        desc,
                        address,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                MrEp::Connected(ep) => unsafe {
                    ep.writev(
                        iov,
                        desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
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

    // pub fn readv(&mut self, iov: &[IoVecMut], dest_addr: u64, desc: &mut [MemoryRegionDesc]) {
    //     let (start, _end) = self.remote_mem_addr.unwrap();
    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::Connectionless(ep) => unsafe {
    //                 ep.readv_from(
    //                     iov,
    //                     desc,
    //                     self.mapped_addr.as_ref().unwrap(),
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                 )
    //             },
    //             MyEndpoint::Connected(ep) => unsafe {
    //                 ep.readv(
    //                     iov,
    //                     desc,
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                 )
    //             },
    //             _ => panic!("Plain data only"),
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }

    pub fn readv_mr(
        &self,
        ep: &MrEp<I>,
        iov: &[IoVecMutMr],
        dest_addr: u64,
        desc: &mut [MemoryRegionDesc],
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match ep {
                MrEp::Connectless(ep, address) => unsafe {
                    ep.readv_from(
                        iov,
                        desc,
                        address,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
                },
                MrEp::Connected(ep) => unsafe {
                    ep.readv(
                        iov,
                        desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                    )
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

    // [TODO] Enabling .remote_cq_data causes the buffer not being written correctly
    // on the remote side.
    pub unsafe fn writemsg_mr<'a>(
        &self,
        msg: &mut MsgType<MsgRma, MsgRmaConnected, MsgRmaMr<'a>, MsgRmaConnectedMr<'a>>,
        ep: &'a MrEp<I>,
        iov: &'a [IoVecMr<'a>],
        desc: &'a mut [MemoryRegionDesc],
        rma_iov: &'a [RmaIoVec],
        data: u64,
    ) {
        let options = WriteMsgOptions::new();
        match ep {
            MrEp::Connectless(ep, addr) => {
                *msg = MsgType::ConnectionlessMrMsg(MsgRmaMr::from_iov_mr_slice(
                    iov, desc, addr, rma_iov, data,
                ));
                loop {
                    let err = ep.writemsg_to(msg.conless_mr(), options);
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
            MrEp::Connected(ep) => {
                *msg = MsgType::ConnectedMrMsg(MsgRmaConnectedMr::from_iov_slice(
                    iov, desc, rma_iov, data,
                ));
                loop {
                    let err = ep.writemsg(msg.conned_mr(), options);
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
    }

    pub unsafe fn readmsg_mr<'a>(
        &self,
        msg: &mut MsgType<MsgRmaMut, MsgRmaConnectedMut, MsgRmaMutMr<'a>, MsgRmaConnectedMutMr<'a>>,
        ep: &'a MrEp<I>,
        iov: &'a mut [IoVecMutMr<'a>],
        desc: &'a mut [MemoryRegionDesc],
        rma_iov: &'a [RmaIoVec],
        _data: u64,
    ) {
        let options = ReadMsgOptions::new();
        match ep {
            MrEp::Connectless(ep, addr) => {
                *msg = MsgType::ConnectionlessMrMsg(MsgRmaMutMr::from_iov_mr_slice(
                    iov, desc, addr, rma_iov,
                ));
                loop {
                    let err = ep.readmsg_from(msg.conless_mr(), options);
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
            MrEp::Connected(ep) => {
                *msg = MsgType::ConnectedMrMsg(MsgRmaConnectedMutMr::from_iov_mr_slice(
                    iov, desc, rma_iov,
                ));
                loop {
                    let err = ep.readmsg(msg.conned_mr(), options);
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
    }

    // pub fn readmsg(
    //     &self,
    //     msg: &MsgType<MsgRmaMut, MsgRmaConnectedMut, MsgRmaMutMr, MsgRmaConnectedMutMr>,
    // ) {
    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::Connectionless(ep) => match msg {
    //                 MsgType::ConnectionlessMsg(msg) => unsafe {
    //                     ep.readmsg_from(msg, ReadMsgOptions::new())
    //                 },
    //                 MsgType::ConnectedMsg(_) => todo!(),
    //                 _ => panic!("Plain data only"),
    //             },
    //             MyEndpoint::Connected(ep) => match msg {
    //                 MsgType::ConnectionlessMsg(_) => panic!("Wrong message type"),
    //                 MsgType::ConnectedMsg(msg) => unsafe { ep.readmsg(msg, ReadMsgOptions::new()) },
    //                 _ => panic!("Plain data only"),
    //             },
    //             MyEndpoint::ConnectionlessMrLocal(ep) => match msg {
    //                 MsgType::ConnectionlessMrMsg(msg) => unsafe {
    //                     ep.readmsg_from(msg, ReadMsgOptions::new())
    //                 },
    //                 MsgType::ConnectedMrMsg(_) => todo!(),
    //                 _ => panic!("Plain data only"),
    //             },
    //             MyEndpoint::ConnectedMrLocal(ep) => match msg {
    //                 MsgType::ConnectionlessMrMsg(_) => panic!("Wrong message type"),
    //                 MsgType::ConnectedMrMsg(msg) => unsafe {
    //                     ep.readmsg(msg, ReadMsgOptions::new())
    //                 },
    //                 _ => panic!("Plain data only"),
    //             },
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }
}

impl<I: AtomicDefaultCap> Ofi<I> {
    // pub fn atomic<T: libfabric::AsFiType>(
    //     &self,
    //     buf: &[T],
    //     dest_addr: u64,
    //     desc: &mut MemoryRegionDesc,
    //     op: AtomicOp,
    // ) {
    //     let (start, _end) = self.remote_mem_addr.unwrap();
    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::Connectionless(ep) => {
    //                 if buf.len() <= self.info_entry.tx_attr().inject_size() {
    //                     unsafe {
    //                         ep.inject_atomic_to(
    //                             buf,
    //                             self.mapped_addr.as_ref().unwrap(),
    //                             start + dest_addr,
    //                             self.remote_key.as_ref().unwrap(),
    //                             op,
    //                         )
    //                     }
    //                 } else {
    //                     unsafe {
    //                         ep.atomic_to(
    //                             buf,
    //                             desc,
    //                             self.mapped_addr.as_ref().unwrap(),
    //                             start + dest_addr,
    //                             self.remote_key.as_ref().unwrap(),
    //                             op,
    //                         )
    //                     }
    //                 }
    //             }
    //             MyEndpoint::Connected(ep) => {
    //                 if buf.len() <= self.info_entry.tx_attr().inject_size() {
    //                     unsafe {
    //                         ep.inject_atomic(
    //                             buf,
    //                             start + dest_addr,
    //                             self.remote_key.as_ref().unwrap(),
    //                             op,
    //                         )
    //                     }
    //                 } else {
    //                     unsafe {
    //                         ep.atomic(
    //                             buf,
    //                             desc,
    //                             start + dest_addr,
    //                             self.remote_key.as_ref().unwrap(),
    //                             op,
    //                         )
    //                     }
    //                 }
    //             }
    //             _ => panic!("Plain data only"),
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }

    pub fn atomic_mr<T: libfabric::AsFiType + Copy>(
        &self,
        ep: &MrEp<I>,
        buf: &MemoryRegionSlice<T>,
        dest_addr: u64,
        desc: &mut MemoryRegionDesc,
        op: AtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match &ep {
                MrEp::Connectless(ep, addr) => {
                    if buf.len() <= self.info_entry.tx_attr().inject_size() {
                        unsafe {
                            ep.inject_atomic_to(
                                buf,
                                addr,
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
                                addr,
                                start + dest_addr,
                                self.remote_key.as_ref().unwrap(),
                                op,
                            )
                        }
                    }
                }
                MrEp::Connected(ep) => {
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

    // pub fn atomicv<T: libfabric::AsFiType>(
    //     &mut self,
    //     ioc: &[libfabric::iovec::Ioc<T>],
    //     dest_addr: u64,
    //     desc: &mut [MemoryRegionDesc],
    //     op: AtomicOp,
    // ) {
    //     let (start, _end) = self.remote_mem_addr.unwrap();
    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::Connectionless(ep) => unsafe {
    //                 ep.atomicv_to(
    //                     ioc,
    //                     desc,
    //                     self.mapped_addr.as_ref().unwrap(),
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                     op,
    //                 )
    //             },
    //             MyEndpoint::Connected(ep) => unsafe {
    //                 ep.atomicv(
    //                     ioc,
    //                     desc,
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                     op,
    //                 )
    //             },
    //             _ => panic!("Plain data only"),
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }

    pub fn atomicv_mr<T: libfabric::AsFiType + Copy>(
        &self,
        ep: &MrEp<I>,
        ioc: &[libfabric::iovec::IocMr<T>],
        dest_addr: u64,
        desc: &mut [MemoryRegionDesc],
        op: AtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match ep {
                MrEp::Connectless(ep, addr) => unsafe {
                    ep.atomicv_to(
                        ioc,
                        desc,
                        addr,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MrEp::Connected(ep) => unsafe {
                    ep.atomicv(
                        ioc,
                        desc,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
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

    // pub fn atomicmsg<T: libfabric::AsFiType + std::marker::Copy>(
    //     &mut self,
    //     msg: &MsgType<MsgAtomic<T>, MsgAtomicConnected<T>, MsgAtomicMr<T>, MsgAtomicConnectedMr<T>>,
    // ) {
    //     let opts = AtomicMsgOptions::new();
    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::Connectionless(ep) => match msg {
    //                 MsgType::ConnectionlessMsg(msg) => unsafe { ep.atomicmsg_to(msg, opts) },
    //                 MsgType::ConnectedMsg(_) => todo!(),
    //                 _ => panic!("Plain data only"),
    //             },
    //             MyEndpoint::Connected(ep) => match msg {
    //                 MsgType::ConnectionlessMsg(_) => todo!(),
    //                 MsgType::ConnectedMsg(msg) => unsafe { ep.atomicmsg(msg, opts) },
    //                 _ => panic!("Plain data only"),
    //             },
    //             MyEndpoint::ConnectionlessMrLocal(ep) => match msg {
    //                 MsgType::ConnectionlessMrMsg(msg) => unsafe { ep.atomicmsg_to(msg, opts) },
    //                 MsgType::ConnectedMsg(_) => todo!(),
    //                 _ => panic!("Mr data only"),
    //             },
    //             MyEndpoint::ConnectedMrLocal(ep) => match msg {
    //                 MsgType::ConnectionlessMrMsg(_) => todo!(),
    //                 MsgType::ConnectedMrMsg(msg) => unsafe { ep.atomicmsg(msg, opts) },
    //                 _ => panic!("Mr data only"),
    //             },
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }

    // pub fn fetch_atomic<T: libfabric::AsFiType>(
    //     &mut self,
    //     buf: &[T],
    //     res: &mut [T],
    //     dest_addr: u64,
    //     desc: &mut MemoryRegionDesc,
    //     res_desc: &mut MemoryRegionDesc,
    //     op: FetchAtomicOp,
    // ) {
    //     let (start, _end) = self.remote_mem_addr.unwrap();
    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::Connectionless(ep) => unsafe {
    //                 ep.fetch_atomic_from(
    //                     buf,
    //                     desc,
    //                     res,
    //                     res_desc,
    //                     self.mapped_addr.as_ref().unwrap(),
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                     op,
    //                 )
    //             },
    //             MyEndpoint::Connected(ep) => unsafe {
    //                 ep.fetch_atomic(
    //                     buf,
    //                     desc,
    //                     res,
    //                     res_desc,
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                     op,
    //                 )
    //             },
    //             _ => panic!("Plain data only"),
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }

    pub fn fetch_atomic_mr<T: libfabric::AsFiType + Copy>(
        &self,
        ep: &MrEp<I>,
        buf: &MemoryRegionSlice<T>,
        res: &mut MemoryRegionSlice<T>,
        dest_addr: u64,
        desc: &mut MemoryRegionDesc,
        res_desc: &mut MemoryRegionDesc,
        op: FetchAtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match ep {
                MrEp::Connectless(ep, addr) => unsafe {
                    ep.fetch_atomic_from(
                        buf,
                        desc,
                        res,
                        res_desc,
                        addr,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MrEp::Connected(ep) => unsafe {
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

    // pub fn fetch_atomicv<T: libfabric::AsFiType>(
    //     &mut self,
    //     ioc: &[libfabric::iovec::Ioc<T>],
    //     res_ioc: &mut [libfabric::iovec::IocMut<T>],
    //     dest_addr: u64,
    //     desc: &mut [MemoryRegionDesc],
    //     res_desc: &mut [MemoryRegionDesc],
    //     op: FetchAtomicOp,
    // ) {
    //     let (start, _end) = self.remote_mem_addr.unwrap();
    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::Connectionless(ep) => unsafe {
    //                 ep.fetch_atomicv_from(
    //                     ioc,
    //                     desc,
    //                     res_ioc,
    //                     res_desc,
    //                     self.mapped_addr.as_ref().unwrap(),
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                     op,
    //                 )
    //             },
    //             MyEndpoint::Connected(ep) => unsafe {
    //                 ep.fetch_atomicv(
    //                     ioc,
    //                     desc,
    //                     res_ioc,
    //                     res_desc,
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                     op,
    //                 )
    //             },
    //             _ => panic!("Plain data only"),
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }

    pub fn fetch_atomicv_mr<T: libfabric::AsFiType + Copy>(
        &self,
        ep: &MrEp<I>,
        ioc: &[libfabric::iovec::IocMr<T>],
        res_ioc: &mut [libfabric::iovec::IocMutMr<T>],
        dest_addr: u64,
        desc: &mut [MemoryRegionDesc],
        res_desc: &mut [MemoryRegionDesc],
        op: FetchAtomicOp,
    ) {
        let (start, _end) = self.remote_mem_addr.unwrap();
        loop {
            let err = match ep {
                MrEp::Connectless(ep, addr) => unsafe {
                    ep.fetch_atomicv_from(
                        ioc,
                        desc,
                        res_ioc,
                        res_desc,
                        addr,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MrEp::Connected(ep) => unsafe {
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

    // pub fn fetch_atomicmsg<T: libfabric::AsFiType>(
    //     &mut self,
    //     msg: &MsgType<
    //         MsgFetchAtomic<T>,
    //         MsgFetchAtomicConnected<T>,
    //         MsgFetchAtomicMr<T>,
    //         MsgFetchAtomicConnectedMr<T>,
    //     >,
    //     res_ioc: &mut [libfabric::iovec::IocMut<T>],
    //     res_desc: &mut [MemoryRegionDesc],
    // ) {
    //     let opts = AtomicMsgOptions::new();
    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::Connectionless(ep) => match msg {
    //                 MsgType::ConnectionlessMsg(msg) => unsafe {
    //                     ep.fetch_atomicmsg_from(msg, res_ioc, res_desc, opts)
    //                 },
    //                 MsgType::ConnectedMsg(_) => todo!(),
    //                 _ => panic!("Plain data only"),
    //             },
    //             MyEndpoint::Connected(ep) => match msg {
    //                 MsgType::ConnectionlessMsg(_) => todo!(),
    //                 MsgType::ConnectedMsg(msg) => unsafe {
    //                     ep.fetch_atomicmsg(msg, res_ioc, res_desc, opts)
    //                 },
    //                 _ => panic!("Plain data only"),
    //             },
    //             _ => panic!("Plain data only"),
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }

    // pub fn fetch_atomicmsg_mr<T: libfabric::AsFiType + Copy>(
    //     &self,
    //     msg: &mut MsgType<
    //         MsgFetchAtomic<T>,
    //         MsgFetchAtomicConnected<T>,
    //         MsgFetchAtomicMr<T>,
    //         MsgFetchAtomicConnectedMr<T>,
    //     >,
    //     res_ioc: &mut [libfabric::iovec::IocMutMr<T>],
    //     res_desc: &mut [MemoryRegionDesc],
    // ) {
    //     let opts = AtomicMsgOptions::new();
    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::ConnectionlessMrLocal(ep) => match msg {
    //                 MsgType::ConnectionlessMrMsg(msg) => unsafe {
    //                     ep.fetch_atomicmsg_from(msg, res_ioc, res_desc, opts)
    //                 },
    //                 MsgType::ConnectedMrMsg(_) => todo!(),
    //                 _ => panic!("Mr data only"),
    //             },
    //             MyEndpoint::ConnectedMrLocal(ep) => match msg {
    //                 MsgType::ConnectionlessMrMsg(_) => todo!(),
    //                 MsgType::ConnectedMrMsg(msg) => unsafe {
    //                     ep.fetch_atomicmsg(msg, res_ioc, res_desc, opts)
    //                 },
    //                 _ => panic!("Mr data only"),
    //             },
    //             _ => panic!("Mr data only"),
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }
    // pub fn compare_atomic<T: libfabric::AsFiType>(
    //     &mut self,
    //     buf: &[T],
    //     comp: &[T],
    //     res: &mut [T],
    //     dest_addr: u64,
    //     desc: &mut MemoryRegionDesc,
    //     comp_desc: &mut MemoryRegionDesc,
    //     res_desc: &mut MemoryRegionDesc,
    //     op: CompareAtomicOp,
    // ) {
    //     let (start, _end) = self.remote_mem_addr.unwrap();
    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::Connectionless(ep) => unsafe {
    //                 ep.compare_atomic_to(
    //                     buf,
    //                     desc,
    //                     comp,
    //                     comp_desc,
    //                     res,
    //                     res_desc,
    //                     self.mapped_addr.as_ref().unwrap(),
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                     op,
    //                 )
    //             },
    //             MyEndpoint::Connected(ep) => unsafe {
    //                 ep.compare_atomic(
    //                     buf,
    //                     desc,
    //                     comp,
    //                     comp_desc,
    //                     res,
    //                     res_desc,
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                     op,
    //                 )
    //             },
    //             _ => panic!("Plain data only"),
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }
    pub fn compare_atomic_mr<T: libfabric::AsFiType + Copy>(
        &self,
        ep: &MrEp<I>,
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
            let err = match ep {
                MrEp::Connectless(ep, addr) => unsafe {
                    ep.compare_atomic_to(
                        buf,
                        desc,
                        comp,
                        comp_desc,
                        res,
                        res_desc,
                        addr,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MrEp::Connected(ep) => unsafe {
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
    // pub fn compare_atomic_mr<T: libfabric::AsFiType + Copy>(
    //     &self,
    //     ep: &MrEp<I>,
    //     buf: &MemoryRegionSlice<T>,
    //     comp: &MemoryRegionSlice<T>,
    //     res: &mut MemoryRegionSlice<T>,
    //     dest_addr: u64,
    //     desc: &mut MemoryRegionDesc,
    //     comp_desc: &mut MemoryRegionDesc,
    //     res_desc: &mut MemoryRegionDesc,
    //     op: CompareAtomicOp,
    // ) {
    //     let (start, _end) = self.remote_mem_addr.unwrap();
    //     loop {
    //         let err = match ep {
    //             MrEp::Connectless(ep, addr) => unsafe {
    //                 ep.compare_atomic_to(
    //                     buf,
    //                     desc,
    //                     comp,
    //                     comp_desc,
    //                     res,
    //                     res_desc,
    //                     addr,
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                     op,
    //                 )
    //             },
    //             MrEp::Connected(ep) => unsafe {
    //                 ep.compare_atomic(
    //                     buf,
    //                     desc,
    //                     comp,
    //                     comp_desc,
    //                     res,
    //                     res_desc,
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                     op,
    //                 )
    //             },
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }

    // pub fn compare_atomicv<T: libfabric::AsFiType>(
    //     &mut self,
    //     ioc: &[libfabric::iovec::Ioc<T>],
    //     comp_ioc: &[libfabric::iovec::Ioc<T>],
    //     res_ioc: &mut [libfabric::iovec::IocMut<T>],
    //     dest_addr: u64,
    //     desc: &mut [MemoryRegionDesc],
    //     comp_desc: &mut [MemoryRegionDesc],
    //     res_desc: &mut [MemoryRegionDesc],
    //     op: CompareAtomicOp,
    // ) {
    //     let (start, _end) = self.remote_mem_addr.unwrap();
    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::Connectionless(ep) => unsafe {
    //                 ep.compare_atomicv_to(
    //                     ioc,
    //                     desc,
    //                     comp_ioc,
    //                     comp_desc,
    //                     res_ioc,
    //                     res_desc,
    //                     self.mapped_addr.as_ref().unwrap(),
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                     op,
    //                 )
    //             },
    //             MyEndpoint::Connected(ep) => unsafe {
    //                 ep.compare_atomicv(
    //                     ioc,
    //                     desc,
    //                     comp_ioc,
    //                     comp_desc,
    //                     res_ioc,
    //                     res_desc,
    //                     start + dest_addr,
    //                     self.remote_key.as_ref().unwrap(),
    //                     op,
    //                 )
    //             },
    //             _ => panic!("Plain data only"),
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }

    pub fn compare_atomicv_mr<T: libfabric::AsFiType + Copy>(
        &self,
        ep: &MrEp<I>,
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
            let err = match ep {
                MrEp::Connectless(ep, addr) => unsafe {
                    ep.compare_atomicv_to(
                        ioc,
                        desc,
                        comp_ioc,
                        comp_desc,
                        res_ioc,
                        res_desc,
                        addr,
                        start + dest_addr,
                        self.remote_key.as_ref().unwrap(),
                        op,
                    )
                },
                MrEp::Connected(ep) => unsafe {
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

    // pub fn compare_atomicmsg<T: libfabric::AsFiType>(
    //     &mut self,
    //     msg: &MsgType<
    //         MsgCompareAtomic<T>,
    //         MsgCompareAtomicConnected<T>,
    //         MsgCompareAtomicMr<T>,
    //         MsgCompareAtomicConnectedMr<T>,
    //     >,
    //     comp_ioc: &[libfabric::iovec::Ioc<T>],
    //     res_ioc: &mut [libfabric::iovec::IocMut<T>],
    //     comp_desc: &mut [MemoryRegionDesc],
    //     res_desc: &mut [MemoryRegionDesc],
    // ) {
    //     let opts = AtomicMsgOptions::new();
    //     loop {
    //         let err = match &self.ep {
    //             MyEndpoint::Connectionless(ep) => match msg {
    //                 MsgType::ConnectionlessMsg(msg) => unsafe {
    //                     ep.compare_atomicmsg_to(msg, comp_ioc, comp_desc, res_ioc, res_desc, opts)
    //                 },
    //                 MsgType::ConnectedMsg(_) => todo!(),
    //                 _ => panic!("Plain data only"),
    //             },
    //             MyEndpoint::Connected(ep) => match msg {
    //                 MsgType::ConnectionlessMsg(_) => todo!(),
    //                 MsgType::ConnectedMsg(msg) => unsafe {
    //                     ep.compare_atomicmsg(msg, comp_ioc, comp_desc, res_ioc, res_desc, opts)
    //                 },
    //                 _ => panic!("Plain data only"),
    //             },
    //             _ => panic!("Plain data only"),
    //         };
    //         match err {
    //             Ok(_) => break,
    //             Err(err) => {
    //                 if !matches!(err.kind, ErrorKind::TryAgain) {
    //                     panic!("{:?}", err);
    //                 }
    //             }
    //         }

    //         ft_progress(self.cq_type.tx_cq());
    //         ft_progress(self.cq_type.rx_cq());
    //     }
    // }

    pub unsafe fn compare_atomicmsg_mr<'a, T: libfabric::AsFiType + Copy>(
        &self,
        msg: &mut MsgType<
            MsgCompareAtomic<T>,
            MsgCompareAtomicConnected<T>,
            MsgCompareAtomicMr<'a, T>,
            MsgCompareAtomicConnectedMr<'a, T>,
        >,
        ep: &'a MrEp<I>,
        iov: &'a [IocMr<'a, T>],
        desc: &'a mut [MemoryRegionDesc],
        rma_iov: &'a [RmaIoc],
        op: CompareAtomicOp,
        data: u64,
        comp_ioc: &[libfabric::iovec::IocMr<T>],
        res_ioc: &mut [libfabric::iovec::IocMutMr<T>],
        comp_desc: &mut [MemoryRegionDesc],
        res_desc: &mut [MemoryRegionDesc],
    ) {
        let opts = AtomicMsgOptions::new();
        match &ep {
            MrEp::Connectless(ep, addr) => {
                *msg = MsgType::ConnectionlessMrMsg(MsgCompareAtomicMr::from_ioc_mr_slice(
                    iov, desc, addr, rma_iov, op, data,
                ));
                loop {
                    let err = ep.compare_atomicmsg_to(
                        msg.conless_mr(),
                        comp_ioc,
                        comp_desc,
                        res_ioc,
                        res_desc,
                        opts,
                    );

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
            MrEp::Connected(ep) => {
                *msg = MsgType::ConnectedMrMsg(MsgCompareAtomicConnectedMr::from_ioc_mr_slice(
                    iov, desc, rma_iov, op, data,
                ));
                loop {
                    let err = ep.compare_atomicmsg(
                        msg.conned_mr(),
                        comp_ioc,
                        comp_desc,
                        res_ioc,
                        res_desc,
                        opts,
                    );

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
    }

    pub unsafe fn fetch_atomicmsg_mr<'a, T: libfabric::AsFiType + Copy>(
        &self,
        msg: &mut MsgType<
            MsgFetchAtomic<T>,
            MsgFetchAtomicConnected<T>,
            MsgFetchAtomicMr<'a, T>,
            MsgFetchAtomicConnectedMr<'a, T>,
        >,
        ep: &'a MrEp<I>,
        iov: &'a [IocMr<'a, T>],
        desc: &'a mut [MemoryRegionDesc],
        rma_iov: &'a [RmaIoc],
        op: FetchAtomicOp,
        data: u64,
        res_ioc: &mut [libfabric::iovec::IocMutMr<T>],
        res_desc: &mut [MemoryRegionDesc],
    ) {
        let opts = AtomicMsgOptions::new();
        match &ep {
            MrEp::Connectless(ep, addr) => {
                *msg = MsgType::ConnectionlessMrMsg(MsgFetchAtomicMr::from_ioc_mr_slice(
                    iov, desc, addr, rma_iov, op, data,
                ));
                loop {
                    let err = ep.fetch_atomicmsg_from(msg.conless_mr(), res_ioc, res_desc, opts);

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
            MrEp::Connected(ep) => {
                *msg = MsgType::ConnectedMrMsg(MsgFetchAtomicConnectedMr::from_ioc_mr_slice(
                    iov, desc, rma_iov, op, data,
                ));
                loop {
                    let err = ep.fetch_atomicmsg(msg.conned_mr(), res_ioc, res_desc, opts);

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
    }

    pub unsafe fn atomicmsg_mr<'a, T: libfabric::AsFiType + Copy>(
        &self,
        msg: &mut MsgType<
            MsgAtomic<T>,
            MsgAtomicConnected<T>,
            MsgAtomicMr<'a, T>,
            MsgAtomicConnectedMr<'a, T>,
        >,
        ep: &'a MrEp<I>,
        iov: &'a [IocMr<T>],
        desc: &'a mut [MemoryRegionDesc],
        rma_iov: &'a [RmaIoc],
        op: AtomicOp,
        data: u64,
    ) {
        let opts = AtomicMsgOptions::new();
        match &ep {
            MrEp::Connectless(ep, addr) => {
                *msg = MsgType::ConnectionlessMrMsg(MsgAtomicMr::from_ioc_mr_slice(
                    iov, desc, addr, rma_iov, op, data,
                ));
                loop {
                    let err = ep.atomicmsg_to(msg.conless_mr(), opts);

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
            MrEp::Connected(ep) => {
                *msg = MsgType::ConnectedMrMsg(MsgAtomicConnectedMr::from_ioc_mr_slice(
                    iov, desc, rma_iov, op, data,
                ));
                loop {
                    let err = ep.atomicmsg(msg.conned_mr(), opts);

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
    let ofi = if connected {
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
            MyEndpoint::Plain(ep) => {
                ofi.send_with_context(ep, &reg_mem[..512], &mut desc[0], None, &mut ctx)
            }
            MyEndpoint::Mr(ep) => {
                ofi.send_mr_with_context(
                    ep,
                    &mr.slice(0).slice(..512),
                    &mut desc[0],
                    None,
                    &mut ctx,
                );
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
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[..128], &mut desc[0], None),
            MyEndpoint::Mr(ep) => ofi.send_mr(ep, &mr.slice(0).slice(..128), &mut desc[0], None),
        }
        // ofi.send(&reg_mem[..128], &mut desc[0], None);
        // No cq.sread since inject does not generate completions

        // // Send single Iov
        let iov = [IoVec::from_slice(&reg_mem[..512])];
        let mem_mrs = (mr.slice(0).slice(..512), mr.slice(0).slice(512..1024));
        let iov_mr = [IoVecMr::from(&mem_mrs.0)];

        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.sendv(ep, &iov, &mut desc[..1]),
            MyEndpoint::Mr(ep) => ofi.sendv_mr(ep, &iov_mr, &mut desc[..1]),
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
            MyEndpoint::Plain(ep) => ofi.sendv(ep, &iov, &mut desc),
            MyEndpoint::Mr(ep) => ofi.sendv_mr(ep, &iov_mr, &mut desc),
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
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[..512]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(..512), &mut desc[0]),
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(reg_mem[..512], expected[..512]);

        // Receive inject
        reg_mem.iter_mut().for_each(|v| *v = 0);
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[..128]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(..128), &mut desc[0]),
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
            MyEndpoint::Plain(ep) => ofi.recvv(ep, &mut iov),
            MyEndpoint::Mr(ep) => ofi.recvv_mr(ep, &mut iov_mr, &mut desc[..1]),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(reg_mem[..512], expected[..512]);

        reg_mem.iter_mut().for_each(|v| *v = 0);

        // // Receive into multiple Iovs
        let (mem0, mem1) = reg_mem[..1024].split_at_mut(512);
        let iov = [IoVecMut::from_slice(mem0), IoVecMut::from_slice(mem1)];
        let iov_mr = [IoVecMutMr::from(mem_mrs.0), IoVecMutMr::from(mem_mrs.1)];
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recvv(ep, &iov),
            MyEndpoint::Mr(ep) => ofi.recvv_mr(ep, &iov_mr, &mut desc),
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
    let ofi = if connected {
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
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[..512], &mut desc[0], data),
            MyEndpoint::Mr(ep) => ofi.send_mr(ep, &mr.slice(0).slice(..512), &mut desc[0], data),
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
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[..512]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(..512), &mut desc[0]),
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
        MyEndpoint::Mr(ep) => match ep {
            MrEp::Connected(ep) => mr.bind_ep(ep).unwrap(),
            MrEp::Connectless(ep, _) => mr.bind_ep(ep).unwrap(),
        },
        MyEndpoint::Plain(ep) => match ep {
            PlainEp::Connected(ep) => mr.bind_ep(ep).unwrap(),
            PlainEp::Connectless(ep, _) => mr.bind_ep(ep).unwrap(),
        },
    }
}

fn tsendrecv(server: bool, name: &str, connected: bool) {
    let ofi = if connected {
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
            MyEndpoint::Plain(ep) => ofi.tsend(ep, &reg_mem[..512], 10, data),
            MyEndpoint::Mr(ep) => {
                ofi.tsend_mr(ep, &mr.slice(0).slice(..512), &mut desc[0], 10, data)
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
            MyEndpoint::Plain(ep) => ofi.tsend(ep, &reg_mem[..128], 1, data),
            MyEndpoint::Mr(ep) => {
                ofi.tsend_mr(ep, &mr.slice(0).slice(..128), &mut desc[0], 1, data)
            }
        }

        // No cq.sread since inject does not generate completions

        // // Send single Iov
        let iov = [IoVec::from_slice(&reg_mem[..512])];
        let mem_mr0 = mr.slice(0).slice(..512);
        let mem_mr1 = mr.slice(0).slice(512..1024);
        let iov_mr = [IoVecMr::from(&mem_mr0)];
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.tsendv(ep, &iov, 2),
            MyEndpoint::Mr(ep) => ofi.tsendv_mr(ep, &iov_mr, &mut desc[..1], 2),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Send multi Iov
        let iov = [
            IoVec::from_slice(&reg_mem[..512]),
            IoVec::from_slice(&reg_mem[512..1024]),
        ];
        let iov_mr = [IoVecMr::from(&mem_mr0), IoVecMr::from(&mem_mr1)];

        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.tsendv(ep, &iov, 3),
            MyEndpoint::Mr(ep) => ofi.tsendv_mr(ep, &iov_mr, &mut desc, 3),
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
            MyEndpoint::Plain(ep) => ofi.trecv(ep, &mut reg_mem[..512], 10),
            MyEndpoint::Mr(ep) => ofi.trecv_mr(ep, &mut mr.slice(0).slice(..512), &mut desc[0], 10),
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
            MyEndpoint::Plain(ep) => ofi.trecv(ep, &mut reg_mem[..128], 1),
            MyEndpoint::Mr(ep) => ofi.trecv_mr(ep, &mut mr.slice(0).slice(..128), &mut desc[0], 1),
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
            MyEndpoint::Plain(ep) => ofi.trecvv(ep, &mut iov, 2),
            MyEndpoint::Mr(ep) => ofi.trecvv_mr(ep, &mut iov_mr, &mut desc[..1], 2),
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
            MyEndpoint::Plain(ep) => ofi.trecvv(ep, &iov, 3),
            MyEndpoint::Mr(ep) => ofi.trecvv_mr(ep, &iov_mr, &mut desc, 3),
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
    let ofi = if connected {
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

    if server {
        // Single iov message
        let (mem0, mem1) = (&reg_mem[..512], &reg_mem[1024..1536]);
        let (mem_mr0, mem_mr1) = (&mr.slice(0).slice(..512), &mr.slice(0).slice(1024..1536));

        let iov0 = IoVec::from_slice(mem0);
        let iov1 = IoVec::from_slice(mem1);

        let iov_mr0 = IoVecMr::from(mem_mr0);
        let iov_mr1 = IoVecMr::from(mem_mr1);
        let mut msg = MsgType::Uninit;
        let mut desc = mr.description();

        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.sendmsg(
                &mut msg,
                ep,
                std::slice::from_ref(&iov0),
                std::slice::from_mut(&mut descs[0]),
                128,
            ),
            MyEndpoint::Mr(ep) => ofi.sendmsg_mr(
                &mut msg,
                ep,
                std::slice::from_ref(&iov_mr0),
                std::slice::from_mut(&mut desc),
                128,
            ),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Multi iov message with stride
        let iovs = [iov0, iov1];
        let iovs_mr = [iov_mr0, iov_mr1];
        let mut msg = MsgType::Uninit;
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.sendmsg(&mut msg, ep, &iovs, &mut descs, 128),
            MyEndpoint::Mr(ep) => ofi.sendmsg_mr(&mut msg, ep, &iovs_mr, &mut descs, 128),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        let mut msg = MsgType::Uninit;

        // Single iov message
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.sendmsg(
                &mut msg,
                ep,
                std::slice::from_ref(&iovs[0]),
                std::slice::from_mut(&mut descs[0]),
                0,
            ),
            MyEndpoint::Mr(ep) => ofi.sendmsg_mr(
                &mut msg,
                ep,
                std::slice::from_ref(&iovs_mr[0]),
                std::slice::from_mut(&mut descs[0]),
                0,
            ),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        let mut msg = MsgType::Uninit;

        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.sendmsg(&mut msg, ep, &iovs, &mut descs, 0),
            MyEndpoint::Mr(ep) => ofi.sendmsg_mr(&mut msg, ep, &iovs_mr, &mut descs, 0),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    } else {
        let mut desc = mr.description();

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

        let mut msg = MsgType::Uninit;
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recvmsg(
                &mut msg,
                ep,
                std::slice::from_mut(&mut iov),
                std::slice::from_mut(&mut desc),
                0,
            ),
            MyEndpoint::Mr(ep) => ofi.recvmsg_mr(
                &mut msg,
                ep,
                std::slice::from_mut(&mut iov_mr),
                std::slice::from_mut(&mut descs[0]),
                0,
            ),
        }

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
        let mut msg = MsgType::Uninit;

        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recvmsg(
                &mut msg,
                ep,
                std::slice::from_mut(&mut iov),
                std::slice::from_mut(&mut descs[0]),
                0,
            ),
            MyEndpoint::Mr(ep) => ofi.recvmsg_mr(
                &mut msg,
                ep,
                std::slice::from_mut(&mut iov_mr),
                std::slice::from_mut(&mut descs[0]),
                0,
            ),
        }

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
        let mut msg = MsgType::Uninit;

        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recvmsg(&mut msg, ep, &mut iovs, &mut descs, 0),
            MyEndpoint::Mr(ep) => ofi.recvmsg_mr(&mut msg, ep, &mut iovs_mr, &mut descs, 0),
        }

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
        let mut msg = MsgType::Uninit;

        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recvmsg(&mut msg, ep, &mut iovs, &mut descs, 0),
            MyEndpoint::Mr(ep) => ofi.recvmsg_mr(&mut msg, ep, &mut iovs_mr, &mut descs, 0),
        }

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
    let ofi = if connected {
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

    if server {
        let mut msg = MsgType::Uninit;

        // Single iov message
        let (mem0, mem1) = (&reg_mem[..512], &reg_mem[1024..1536]);
        let (mem0_slice, mem1_slice) = (&mr.slice(0).slice(..512), &mr.slice(0).slice(1024..1536));
        let iov0 = IoVec::from_slice(mem0);
        let iov1 = IoVec::from_slice(mem1);
        let iov0_mr = IoVecMr::from(mem0_slice);
        let iov1_mr = IoVecMr::from(mem1_slice);
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.tsendmsg(
                &mut msg,
                ep,
                std::slice::from_ref(&iov0),
                std::slice::from_mut(&mut descs[0]),
                128,
                0,
            ),
            MyEndpoint::Mr(ep) => ofi.tsendmsg_mr(
                &mut msg,
                ep,
                std::slice::from_ref(&iov0_mr),
                std::slice::from_mut(&mut descs[0]),
                128,
                0,
            ),
        };
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Multi iov message with stride
        let iovs = [iov0, iov1];
        let mr_iovs = [iov0_mr, iov1_mr];
        let mut msg = MsgType::Uninit;
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.tsendmsg(&mut msg, ep, &iovs, &mut descs, 1, 0),
            MyEndpoint::Mr(ep) => ofi.tsendmsg_mr(&mut msg, ep, &mr_iovs, &mut descs, 1, 0),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        let mut msg = MsgType::Uninit;

        // Single iov message
        match &ofi.ep {
            MyEndpoint::Plain(ep) => {
                ofi.tsendmsg(
                    &mut msg,
                    ep,
                    std::slice::from_ref(&iovs[0]),
                    std::slice::from_mut(&mut descs[0]),
                    2,
                    0,
                )
            }
            MyEndpoint::Mr(ep) => {
                ofi.tsendmsg_mr(
                    &mut msg,
                    ep,
                    std::slice::from_ref(&mr_iovs[0]),
                    std::slice::from_mut(&mut descs[0]),
                    2,
                    0,
                )
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        let mut msg = MsgType::Uninit;

        match &ofi.ep {
            MyEndpoint::Plain(ep) => {
                ofi.tsendmsg(&mut msg, ep, &iovs, &mut descs, 3, 0)
            }
            MyEndpoint::Mr(ep) => ofi.tsendmsg_mr(&mut msg, ep, &mr_iovs, &mut descs, 3, 0),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
    } else {
        reg_mem.iter_mut().for_each(|v| *v = 0);
        let (mem0, mem1) = reg_mem.split_at_mut(512);
        let (mem0_mr, mem1_mr) = (&mut mr.slice(0).slice(..512), &mut mr.slice(0).slice(512..));
        let expected: Vec<_> = (0..1024).map(|v: usize| (v % 256) as u8).collect();

        // Receive a single message in a single buffer
        let mut iov = IoVecMut::from_slice(mem0);
        let mut iov_mr = IoVecMutMr::from(mem0_mr);

        let mut msg = MsgType::Uninit;
        match &ofi.ep {
            MyEndpoint::Plain(ep) => {
                ofi.trecvmsg(
                    &mut msg,
                    ep,
                    std::slice::from_mut(&mut iov),
                    std::slice::from_mut(&mut descs[0]),
                    128,
                    0,
                )
            }
            MyEndpoint::Mr(ep) => {
                ofi.trecvmsg_mr(
                    &mut msg,
                    ep,
                    std::slice::from_mut(&mut iov_mr),
                    std::slice::from_mut(&mut descs[0]),
                    128,
                    0,
                )
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        let mut msg = MsgType::Uninit;

        assert_eq!(mem0.len(), expected[..512].len());
        assert_eq!(mem0, &expected[..512]);

        // Receive a multi iov message in a single buffer
        let mut iov = IoVecMut::from_slice(&mut mem1[..1024]);
        let mem1_mr_1024 = &mut mem1_mr.slice(..1024);
        let mut iov_mr = IoVecMutMr::from(mem1_mr_1024);

        match &ofi.ep {
            MyEndpoint::Plain(ep) => {
                ofi.trecvmsg(
                    &mut msg,
                    ep,
                    std::slice::from_mut(&mut iov),
                    std::slice::from_mut(&mut descs[0]),
                    1,
                    0,
                )
            }
            MyEndpoint::Mr(ep) => {
                ofi.trecvmsg_mr(
                    &mut msg,
                    ep,
                    std::slice::from_mut(&mut iov_mr),
                    std::slice::from_mut(&mut descs[0]),
                    1,
                    0,
                )
            }
        }

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
        let mut msg = MsgType::Uninit;

        match &ofi.ep {
            MyEndpoint::Plain(ep) => {
                ofi.trecvmsg(&mut msg, ep, &mut iovs, &mut descs, 2, 0)
            }
            MyEndpoint::Mr(ep) => {
                ofi.trecvmsg_mr(&mut msg, ep, &mut iovs_mr, &mut descs, 2, 0)
            }
        }

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
        let mut msg = MsgType::Uninit;

        match &ofi.ep {
            MyEndpoint::Plain(ep) => {
                ofi.trecvmsg(&mut msg, ep, &mut iovs, &mut descs, 3, 0)
            }
            MyEndpoint::Mr(ep) => {
                ofi.trecvmsg_mr(&mut msg, ep, &mut iovs_mr, &mut descs, 3, 0)
            }
        }

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
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // ofi.write(&reg_mem[..128], 0, &mut descs[0], None);

                // // Send completion ack
                // ofi.send(&reg_mem[512..1024], &mut descs[0], None);
            }
            MyEndpoint::Mr(ep) => {
                ofi.write_mr(ep, &mr.slice(0).slice(..128), 0, &mut descs[0], None);

                // Send completion ack
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None);
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Write a single buffer
        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // ofi.write(&reg_mem[..512], 0, &mut descs[0], None);
            }
            MyEndpoint::Mr(ep) => {
                ofi.write_mr(ep, &mr.slice(0).slice(..512), 0, &mut descs[0], None);
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => {
                ofi.send(ep, &reg_mem[512..1024], &mut descs[0], None);
            }
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None);
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Write vector of buffers
        // let iovs = [
        //     IoVec::from_slice(&reg_mem[..512]),
        //     IoVec::from_slice(&reg_mem[512..1024]),
        // ];

        let slices = (mr.slice(0).slice(..512), mr.slice(0).slice(512..1024));
        let iov_slices = [IoVecMr::from(&slices.0), IoVecMr::from(&slices.1)];

        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
            }
            MyEndpoint::Mr(ep) => ofi.writev_mr(ep, &iov_slices, 0, &mut descs),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => {
                ofi.send(ep, &reg_mem[512..1024], &mut descs[0], None);
            }
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None);
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => {
                ofi.recv(ep, &mut reg_mem[512..1024]);
            }
            MyEndpoint::Mr(ep) => {
                ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]);
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => {
                ofi.recv(ep, &mut reg_mem[512..1024]);
            }
            MyEndpoint::Mr(ep) => {
                ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]);
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..128], &expected[..128]);

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => {
                ofi.recv(ep, &mut reg_mem[512..1024]);
            }
            MyEndpoint::Mr(ep) => {
                ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]);
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..512], &expected[..512]);

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => {
                ofi.recv(ep, &mut reg_mem[1024..1536]);
            }
            MyEndpoint::Mr(ep) => {
                ofi.recv_mr(ep, &mut mr.slice(0).slice(1024..1536), &mut descs[0]);
            }
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..1024], &expected[..1024]);

        reg_mem.iter_mut().for_each(|v| *v = 0);

        // Read buffer from remote memory
        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // ofi.read(&mut reg_mem[1024..1536], 0, &mut descs[0]);
            }
            MyEndpoint::Mr(ep) => {
                ofi.read_mr(ep, &mut mr.slice(0).slice(1024..1536), 0, &mut descs[0]);
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[1024..1536], &expected[512..1024]);

        // Read vector of buffers from remote memory
        let (mem0, mem1) = reg_mem[1536..].split_at_mut(256);
        // let iovs = [IoVecMut::from_slice(mem0), IoVecMut::from_slice(mem1)];
        let mut slices = (mr.slice(0).slice(1536..1792), mr.slice(0).slice(1792..));
        let iov_slices = [
            IoVecMutMr::from(&mut slices.0),
            IoVecMutMr::from(&mut slices.1),
        ];

        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
            }
            MyEndpoint::Mr(ep) => ofi.readv_mr(ep, &iov_slices, 0, &mut descs),
        };
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        assert_eq!(mem0, &expected[..256]);
        assert_eq!(mem1, &expected[..256]);

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => {
                ofi.send(ep, &reg_mem[512..1024], &mut descs[0], None);
            }
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None);
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

    let key = mr.key().unwrap();
    ofi.exchange_keys(key, reg_mem.as_ptr() as usize, 1024 * 2);
    let expected: Vec<u8> = (0..1024).map(|v: usize| (v % 256) as u8).collect();

    let (start, _end) = ofi.remote_mem_addr.unwrap();
    if server {
        let rma_iov = RmaIoVec::new()
            .address(start)
            .len(128)
            .mapped_key(ofi.remote_key.as_ref().unwrap());

        // let iov = IoVec::from_slice(&reg_mem[..128]);
        let mem_mr = mr.slice(0).slice(..128);
        let iov_mr = IoVecMr::from(&mem_mr);

        let mut msg = MsgType::Uninit;
        // Write inject a single buffer
        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // MsgType::ConnectedMsg(MsgRmaConnected::from_iov(&iov, &mut descs[0], &rma_iov, 0))
            }
            MyEndpoint::Mr(ep) => unsafe {
                ofi.writemsg_mr(
                    &mut msg,
                    ep,
                    std::slice::from_ref(&iov_mr),
                    std::slice::from_mut(&mut descs[0]),
                    std::slice::from_ref(&rma_iov),
                    0,
                )
            },
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // let iov = IoVec::from_slice(&reg_mem[..512]);
        let mem_mr = mr.slice(0).slice(..512);
        let iov_mr = IoVecMr::from(&mem_mr);

        let rma_iov = RmaIoVec::new()
            .address(start)
            .len(512)
            .mapped_key(ofi.remote_key.as_ref().unwrap());

        let mut msg = MsgType::Uninit;

        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                //     MsgType::ConnectedMsg(MsgRmaConnected::from_iov(
                //     &iov,
                //     &mut descs[0],
                //     &rma_iov,
                //     128,
                // ))
            }
            MyEndpoint::Mr(ep) => {
                unsafe {
                    ofi.writemsg_mr(
                        &mut msg,
                        ep,
                        std::slice::from_ref(&iov_mr),
                        std::slice::from_mut(&mut descs[0]),
                        std::slice::from_ref(&rma_iov),
                        128,
                    )
                }

                // MsgType::ConnectedMrMsg(
                //     MsgRmaConnectedMr::from_iov_mr(&iov_mr, &mut descs[0], &rma_iov, 128),
                // )
            }
        };

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // let iov0 = IoVec::from_slice(&reg_mem[..512]);
        // let iov1 = IoVec::from_slice(&reg_mem[512..1024]);
        let (mem_mr0, mem_mr1) = (mr.slice(0).slice(..512), mr.slice(0).slice(512..1024));
        let (iov_mr0, iov_mr1) = (IoVecMr::from(&mem_mr0), IoVecMr::from(&mem_mr1));

        // let iovs = [iov0, iov1];
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

        let mut msg = MsgType::Uninit;
        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // MsgType::ConnectedMsg(MsgRmaConnected::from_iov_slice(
                //     &iovs, &mut descs, &rma_iovs, 0,
                // ))
            }
            MyEndpoint::Mr(ep) => {
                unsafe { ofi.writemsg_mr(&mut msg, ep, &iovs_mr, &mut descs, &rma_iovs, 0) }
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        // Recv completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]),
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..128], &expected[..128]);

        // Recv completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..512], &expected[..512]);

        // Recv completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[1024..1536]),
            MyEndpoint::Mr(ep) => {
                ofi.recv_mr(ep, &mut mr.slice(0).slice(1024..1536), &mut descs[0])
            }
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..1024], &expected[..1024]);

        reg_mem.iter_mut().for_each(|v| *v = 0);

        {
            // let mut iov = IoVecMut::from_slice(&mut reg_mem[1024..1536]);
            let mut mem_mr = mr.slice(0).slice(1024..1536);
            let mut iov_mr = IoVecMutMr::from(&mut mem_mr);

            let rma_iov = RmaIoVec::new()
                .address(start)
                .len(512)
                .mapped_key(ofi.remote_key.as_ref().unwrap());

            // Read buffer from remote memory
            let mut msg = MsgType::Uninit;
            match &ofi.ep {
                MyEndpoint::Plain(_) => {
                    panic!("Unexpected")
                    //     MsgType::ConnectedMsg(MsgRmaConnectedMut::from_iov(
                    //     &mut iov,
                    //     &mut descs[0],
                    //     &rma_iov,
                    // ))
                }
                MyEndpoint::Mr(ep) => {
                    unsafe {
                        ofi.readmsg_mr(
                            &mut msg,
                            ep,
                            std::slice::from_mut(&mut iov_mr),
                            std::slice::from_mut(&mut descs[0]),
                            std::slice::from_ref(&rma_iov),
                            0,
                        )
                    }
                }
            }

            ofi.cq_type.tx_cq().sread(1, -1).unwrap();
            assert_eq!(&reg_mem[1024..1536], &expected[512..1024]);
        }

        // // Read vector of buffers from remote memory
        let (mem0, mem1) = reg_mem[1536..].split_at_mut(256);
        let (mut mem_mr0, mut mem_mr1) =
            (mr.slice(0).slice(1536..1892), mr.slice(0).slice(1892..2048));

        // let mut iovs = [IoVecMut::from_slice(mem0), IoVecMut::from_slice(mem1)];
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

        let mut msg = MsgType::Uninit;
        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                //     MsgType::ConnectedMsg(MsgRmaConnectedMut::from_iov_slice(
                //        &mut iovs, &mut descs, &rma_iovs,
                //    ))
            }
            MyEndpoint::Mr(ep) => {
                unsafe { ofi.readmsg_mr(&mut msg, ep, &mut iovs_mr, &mut descs, &rma_iovs, 0) }
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        assert_eq!(mem0, &expected[..256]);
        assert_eq!(mem1, &expected[..256]);

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None)
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
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // ofi.atomic(ep, &reg_mem[..512], 0, &mut descs[0], AtomicOp::Min);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                // ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Max);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                // ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Sum);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                // ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Prod);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                // ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Bor);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                // ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Band);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // ofi.send(&reg_mem[512..1024], &mut descs[0], None);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // ofi.recv(&mut reg_mem[512..1024], &mut descs[0]);
                // ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                // ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Lor);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                // ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Bxor);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                // ofi.send(&reg_mem[512..1024], &mut descs[0], None);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                // ofi.recv(&mut reg_mem[512..1024], &mut descs[0]);
                // ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                // ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Land);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                // ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::Lxor);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                // ofi.atomic(&reg_mem[..512], 0, &mut descs[0], AtomicOp::AtomicWrite);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                // ofi.send(&reg_mem[512..1024], &mut descs[0], None);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
            }
            MyEndpoint::Mr(ep) => {
                ofi.atomic_mr(
                    ep,
                    &mr.slice(0).slice(..512),
                    0,
                    &mut descs[0],
                    AtomicOp::Min,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(
                    ep,
                    &mr.slice(0).slice(..512),
                    0,
                    &mut descs[0],
                    AtomicOp::Max,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(
                    ep,
                    &mr.slice(0).slice(..512),
                    0,
                    &mut descs[0],
                    AtomicOp::Sum,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(
                    ep,
                    &mr.slice(0).slice(..512),
                    0,
                    &mut descs[0],
                    AtomicOp::Prod,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(
                    ep,
                    &mr.slice(0).slice(..512),
                    0,
                    &mut descs[0],
                    AtomicOp::Bor,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(
                    ep,
                    &mr.slice(0).slice(..512),
                    0,
                    &mut descs[0],
                    AtomicOp::Band,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(
                    ep,
                    &mr.slice(0).slice(..512),
                    0,
                    &mut descs[0],
                    AtomicOp::Lor,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(
                    ep,
                    &mr.slice(0).slice(..512),
                    0,
                    &mut descs[0],
                    AtomicOp::Bxor,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(
                    ep,
                    &mr.slice(0).slice(..512),
                    0,
                    &mut descs[0],
                    AtomicOp::Land,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(
                    ep,
                    &mr.slice(0).slice(..512),
                    0,
                    &mut descs[0],
                    AtomicOp::Lxor,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.atomic_mr(
                    ep,
                    &mr.slice(0).slice(..512),
                    0,
                    &mut descs[0],
                    AtomicOp::AtomicWrite,
                );
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();

                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
            }
        }

        // let iocs = [
        //     Ioc::from_slice(&reg_mem[..256]),
        //     Ioc::from_slice(&reg_mem[256..512]),
        // ];

        let (mem_mr0, mem_mr1) = (mr.slice(0).slice(..256), mr.slice(0).slice(256..512));
        let iocs_mr = [IocMr::from(&mem_mr0), IocMr::from(&mem_mr1)];

        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
            }
            MyEndpoint::Mr(ep) => ofi.atomicv_mr(ep, &iocs_mr, 0, &mut descs, AtomicOp::Prod),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        let mut expected = vec![2u8; 1024 * 2];

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]),
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..512], &expected[..512]);

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        expected = vec![3; 1024 * 2];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..512], &expected[..512]);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // expected = vec![2;1024*2];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        // assert_eq!(&reg_mem[..512], &expected[..512]);

        expected = vec![4; 1024 * 2];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..512], &expected[..512]);

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None)
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
        let (_mem0, mem1) = op_mem.split_at_mut(256);
        let (mem_mr0, mut mem_mr1) = (op_mem_mr.slice(..256), op_mem_mr.slice(256..));

        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Min);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // assert_eq!(mem1, &expected[..256]);

                // expected = vec![1; 256];
                // ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Max);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // assert_eq!(mem1, &expected);

                // expected = vec![2; 256];
                // ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Sum);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // assert_eq!(mem1, &expected);

                // expected = vec![4; 256];
                // ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Prod);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // assert_eq!(mem1, &expected);

                // expected = vec![8; 256];
                // ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Bor);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // assert_eq!(mem1, &expected);

                // expected = vec![10; 256];
                // ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Band);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // assert_eq!(mem1, &expected);

                // // Send a done ack
                // ofi.send(&ack_mem[..512], &mut desc0, None);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // // Send a done ack

                // ofi.recv(&mut ack_mem[..512], &mut desc0);
                // ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                // expected = vec![2; 256];
                // ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Lor);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // assert_eq!(mem1, &expected);

                // expected = vec![1; 256];
                // ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Bxor);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // assert_eq!(mem1, &expected);

                // // Send a done ack
                // ofi.send(&ack_mem[..512], &mut desc0, None);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // // Send a done ack

                // ofi.recv(&mut ack_mem[..512], &mut desc0);
                // ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                // expected = vec![3; 256];
                // ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Land);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // assert_eq!(mem1, &expected);

                // expected = vec![1; 256];
                // ofi.fetch_atomic(&mem0, mem1, 0, &mut desc0, &mut desc1, FetchAtomicOp::Lxor);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // assert_eq!(mem1, &expected);

                // expected = vec![0; 256];
                // ofi.fetch_atomic(
                //     &mem0,
                //     mem1,
                //     0,
                //     &mut desc0,
                //     &mut desc1,
                //     FetchAtomicOp::AtomicWrite,
                // );
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // assert_eq!(mem1, &expected);

                // // Send a done ack
                // ofi.send(&ack_mem[..512], &mut desc0, None);
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // // Send a done ack

                // ofi.recv(&mut ack_mem[..512], &mut desc0);
                // ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                // expected = vec![2; 256];
                // ofi.fetch_atomic(
                //     &mem0,
                //     mem1,
                //     0,
                //     &mut desc0,
                //     &mut desc1,
                //     FetchAtomicOp::AtomicRead,
                // );
                // ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // assert_eq!(mem1, &expected);
            }
            MyEndpoint::Mr(ep) => {
                ofi.fetch_atomic_mr(
                    ep,
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
                    ep,
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
                    ep,
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
                    ep,
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
                    ep,
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
                    ep,
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
                ofi.send_mr(ep, &ack_mem_mr, &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv_mr(ep, &mut ack_mem_mr, &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![2; 256];
                ofi.fetch_atomic_mr(
                    ep,
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
                    ep,
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
                ofi.send_mr(ep, &ack_mem_mr, &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv_mr(ep, &mut ack_mem_mr, &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![3; 256];
                ofi.fetch_atomic_mr(
                    ep,
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
                    ep,
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
                    ep,
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
                ofi.send_mr(ep, &ack_mem_mr, &mut desc0, None);
                ofi.cq_type.tx_cq().sread(1, -1).unwrap();
                // Send a done ack

                ofi.recv_mr(ep, &mut ack_mem_mr, &mut desc0);
                ofi.cq_type.rx_cq().sread(1, -1).unwrap();

                expected = vec![2; 256];
                ofi.fetch_atomic_mr(
                    ep,
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
        let (_read_mem, write_mem) = op_mem.split_at_mut(256);
        let (read_mem_mr, write_mem_mr) = (op_mem_mr.slice(..256), op_mem_mr.slice(256..));
        let (read_mem_mr0, read_mem_mr1) = (read_mem_mr.slice(..128), read_mem_mr.slice(128..));
        let (mut write_mem_mr0, mut write_mem_mr1) =
            (write_mem_mr.slice(..128), write_mem_mr.slice(128..));
        // let iocs = [
        //     Ioc::from_slice(&read_mem[..128]),
        //     Ioc::from_slice(&read_mem[128..256]),
        // ];

        let iocs_mr = [IocMr::from(&read_mem_mr0), IocMr::from(&read_mem_mr1)];

        // let write_mems = write_mem.split_at_mut(128);
        // let mut res_iocs = [
        //     IocMut::from_slice(write_mems.0),
        //     IocMut::from_slice(write_mems.1),
        // ];

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
            MyEndpoint::Plain(_) => {
                panic!("Unepxected")
                // ofi.fetch_atomicv(
                //     &iocs,
                //     &mut res_iocs,
                //     0,
                //     &mut descs,
                //     &mut res_descs,
                //     FetchAtomicOp::Prod,
                // )
            }
            MyEndpoint::Mr(ep) => ofi.fetch_atomicv_mr(
                ep,
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
            MyEndpoint::Plain(ep) => ofi.send(ep, &ack_mem[..512], &mut descs[0], None),
            MyEndpoint::Mr(ep) => ofi.send_mr(ep, &ack_mem_mr, &mut descs[0], None),
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut ack_mem[..512]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut ack_mem_mr, &mut descs[0]),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        let mut expected = vec![2u8; 256];

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut desc0),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::Mr(ep) => ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut desc0, None),
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        expected = vec![3; 256];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut desc0),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::Mr(ep) => ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut desc0, None),
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        expected = vec![2; 256];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut desc0),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::Mr(ep) => ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut desc0, None),
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        expected = vec![4; 256];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut desc0),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::Mr(ep) => ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut desc0, None),
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
        let (_op_mem_mr, mut ack_mem_mr) = (mr.slice(0).slice(..768), mr.slice(0).slice(768..1280));
        let (buf, mem1) = op_mem.split_at_mut(256);
        let (comp, res) = mem1.split_at_mut(256);
        comp.iter_mut().for_each(|v| *v = 1);
        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // ofi.compare_atomic(
                //     &buf,
                //     comp,
                //     res,
                //     0,
                //     &mut desc,
                //     &mut comp_desc,
                //     &mut res_desc,
                //     CompareAtomicOp::Cswap,
                // );
            }
            MyEndpoint::Mr(ep) => {
                ofi.compare_atomic_mr(
                    ep,
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
            MyEndpoint::Plain(_) => {
                panic!("Unexepceted")
                // ofi.compare_atomic(
                //     &buf,
                //     comp,
                //     res,
                //     0,
                //     &mut desc,
                //     &mut comp_desc,
                //     &mut res_desc,
                //     CompareAtomicOp::CswapNe,
                // );
            }
            MyEndpoint::Mr(ep) => {
                ofi.compare_atomic_mr(
                    ep,
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
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // ofi.compare_atomic(
                //     &buf,
                //     comp,
                //     res,
                //     0,
                //     &mut desc,
                //     &mut comp_desc,
                //     &mut res_desc,
                //     CompareAtomicOp::CswapLe,
                // );
            }
            MyEndpoint::Mr(ep) => {
                ofi.compare_atomic_mr(
                    ep,
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
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // ofi.compare_atomic(
                //     &buf,
                //     comp,
                //     res,
                //     0,
                //     &mut desc,
                //     &mut comp_desc,
                //     &mut res_desc,
                //     CompareAtomicOp::CswapLt,
                // );
            }
            MyEndpoint::Mr(ep) => {
                ofi.compare_atomic_mr(
                    ep,
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
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // ofi.compare_atomic(
                //     &buf,
                //     comp,
                //     res,
                //     0,
                //     &mut desc,
                //     &mut comp_desc,
                //     &mut res_desc,
                //     CompareAtomicOp::CswapGe,
                // );
            }
            MyEndpoint::Mr(ep) => {
                ofi.compare_atomic_mr(
                    ep,
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
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // ofi.compare_atomic(
                //     &buf,
                //     comp,
                //     res,
                //     0,
                //     &mut desc,
                //     &mut comp_desc,
                //     &mut res_desc,
                //     CompareAtomicOp::CswapGt,
                // );
            }
            MyEndpoint::Mr(ep) => {
                ofi.compare_atomic_mr(
                    ep,
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
            MyEndpoint::Plain(ep) => ofi.send(ep, &ack_mem[..512], &mut desc, None),
            MyEndpoint::Mr(ep) => ofi.send_mr(ep, &ack_mem_mr, &mut desc, None),
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut ack_mem[..512]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut ack_mem_mr, &mut desc),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();

        // expected = vec![2; 256];
        // let (buf0, buf1) = buf.split_at_mut(128);
        // let (comp0, comp1) = comp.split_at_mut(128);
        // let (res0, res1) = res.split_at_mut(128);

        // let buf_iocs = [Ioc::from_slice(&buf0), Ioc::from_slice(&buf1)];
        // let comp_iocs = [Ioc::from_slice(&comp0), Ioc::from_slice(&comp1)];
        // let mut res_iocs = [IocMut::from_slice(res0), IocMut::from_slice(res1)];

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
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // ofi.compare_atomicv(
                //     &buf_iocs,
                //     &comp_iocs,
                //     &mut res_iocs,
                //     0,
                //     &mut buf_descs,
                //     &mut comp_descs,
                //     &mut res_descs,
                //     CompareAtomicOp::CswapLe,
                // );
            }
            MyEndpoint::Mr(ep) => {
                ofi.compare_atomicv_mr(
                    ep,
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
            MyEndpoint::Plain(ep) => ofi.send(ep, &ack_mem[..512], &mut desc, None),
            MyEndpoint::Mr(ep) => ofi.send_mr(ep, &ack_mem_mr, &mut desc, None),
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut ack_mem[..512]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut ack_mem_mr, &mut desc),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        let mut expected = vec![2u8; 256];

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut desc),
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut desc, None),
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mut mr.slice(0).slice(512..1024), &mut desc, None)
            }
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        expected = vec![3; 256];
        // // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut desc),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut desc, None),
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mut mr.slice(0).slice(512..1024), &mut desc, None)
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

    let key = mr.key().unwrap();
    ofi.exchange_keys(key, reg_mem.as_ptr() as usize, 1024 * 2);

    let (start, _end) = ofi.remote_mem_addr.unwrap();
    if server {
        // let iocs = [
        //     Ioc::from_slice(&reg_mem[..256]),
        //     Ioc::from_slice(&reg_mem[256..512]),
        // ];
        let (mem_mr0, mem_mr1) = (mr.slice(0).slice(..256), mr.slice(0).slice(256..512));
        let iocs_mr = [IocMr::from(&mem_mr0), IocMr::from(&mem_mr1)];

        let rma_ioc0 = RmaIoc::new(start, 256, ofi.remote_key.as_ref().unwrap());
        let rma_ioc1 = RmaIoc::new(start + 256, 256, ofi.remote_key.as_ref().unwrap());
        let rma_iocs = [rma_ioc0, rma_ioc1];

        let mut msg = MsgType::Uninit;
        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                //     MsgType::ConnectedMsg(MsgAtomicConnected::from_ioc_slice(
                //     &iocs,
                //     &mut descs,
                //     &rma_iocs,
                //     AtomicOp::Bor,
                //     128,
                // ))
            }
            MyEndpoint::Mr(ep) => {
                unsafe {
                    ofi.atomicmsg_mr(
                        &mut msg,
                        ep,
                        &iocs_mr,
                        &mut descs,
                        &rma_iocs,
                        AtomicOp::Bor,
                        128,
                    )
                }
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None)
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]),
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        let expected = vec![3u8; 1024 * 2];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut descs[0]),
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..512], &expected[..512]);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut descs[0], None),
            MyEndpoint::Mr(ep) => {
                ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut descs[0], None)
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

    let key = mr.key().unwrap();
    ofi.exchange_keys(key, reg_mem.as_ptr() as usize, 1024 * 2);
    let (start, _end) = ofi.remote_mem_addr.unwrap();

    if server {
        let expected = vec![1u8; 256];
        let (op_mem, ack_mem) = reg_mem.split_at_mut(512);
        let (op_mem_mr, mut ack_mem_mr) =
            (mr.slice(0).slice(..512), &mut mr.slice(0).slice(512..1024));

        let (_read_mem, write_mem) = op_mem.split_at_mut(256);
        let (read_mem_mr, write_mem_mr) = (op_mem_mr.slice(..256), op_mem_mr.slice(256..));
        let (read_mem_mr0, read_mem_mr1) = (read_mem_mr.slice(..128), read_mem_mr.slice(128..256));

        // let iocs = [
        //     Ioc::from_slice(&read_mem[..128]),
        //     Ioc::from_slice(&read_mem[128..256]),
        // ];
        let iocs_mr = [IocMr::from(&read_mem_mr0), IocMr::from(&read_mem_mr1)];

        // let write_mems = write_mem.split_at_mut(128);
        let write_mems_mr = (
            &mut write_mem_mr.slice(..128),
            &mut write_mem_mr.slice(128..),
        );

        // let mut res_iocs = [
        //     IocMut::from_slice(write_mems.0),
        //     IocMut::from_slice(write_mems.1),
        // ];

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

        let mut msg = MsgType::Uninit;
        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("Unexpected")
                // MsgType::ConnectedMsg(MsgFetchAtomicConnected::from_ioc_slice(
                //     &iocs,
                //     &mut descs,
                //     &rma_iocs,
                //     FetchAtomicOp::Prod,
                //     0,
                // ))
            }
            MyEndpoint::Mr(ep) => {
                unsafe {
                    ofi.fetch_atomicmsg_mr(
                        &mut msg,
                        ep,
                        &iocs_mr,
                        &mut descs,
                        &rma_iocs,
                        FetchAtomicOp::Prod,
                        0,
                        &mut res_iocs_mr,
                        &mut res_descs,
                    )
                }
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(write_mem, &expected);

        // Send a done ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &ack_mem[..512], &mut descs[0], None),
            MyEndpoint::Mr(ep) => ofi.send_mr(ep, &ack_mem_mr, &mut descs[0], None),
        };
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();

        // Recv a completion ack

        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut ack_mem[..512]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut ack_mem_mr, &mut descs[0]),
        }
        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        let mut desc0 = mr.description();
        let expected = vec![2u8; 256];
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut desc0),
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);
        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut desc0, None),
            MyEndpoint::Mr(ep) => ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut desc0, None),
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
    let key = mr.key().unwrap();
    ofi.exchange_keys(key, reg_mem.as_ptr() as usize, 1024 * 2);
    let (start, _end) = ofi.remote_mem_addr.unwrap();

    if server {
        let expected = vec![1u8; 256];
        let (op_mem, ack_mem) = reg_mem.split_at_mut(768);
        let op_mem_mr = mr.slice(0).slice(..768);
        let ack_mem_mr = mr.slice(0).slice(768..);
        let (_buf, mem1) = op_mem.split_at_mut(256);
        let (buf_mr, mem1_mr) = (op_mem_mr.slice(..256), op_mem_mr.slice(256..));
        let (comp, res) = mem1.split_at_mut(256);
        let (comp_mr, res_mr) = (mem1_mr.slice(..256), mem1_mr.slice(256..));
        comp.iter_mut().for_each(|v| *v = 1);

        // expected = vec![2; 256];
        // let (buf0, buf1) = buf.split_at_mut(128);
        // let (comp0, comp1) = comp.split_at_mut(128);
        // let (res0, res1) = res.split_at_mut(128);
        let (buf0_mr, buf1_mr) = (buf_mr.slice(..128), buf_mr.slice(128..));
        let (comp0_mr, comp1_mr) = (comp_mr.slice(..128), comp_mr.slice(128..));
        let (res0_mr, res1_mr) = (&mut res_mr.slice(..128), &mut res_mr.slice(128..));

        // let buf_iocs = [Ioc::from_slice(&buf0), Ioc::from_slice(&buf1)];
        // let comp_iocs = [Ioc::from_slice(&comp0), Ioc::from_slice(&comp1)];
        // let mut res_iocs = [IocMut::from_slice(res0), IocMut::from_slice(res1)];
        let buf_iocs_mr = [IocMr::from(&buf0_mr), IocMr::from(&buf1_mr)];
        let comp_iocs_mr = [IocMr::from(&comp0_mr), IocMr::from(&comp1_mr)];
        let mut res_iocs_mr = [IocMutMr::from(res0_mr), IocMutMr::from(res1_mr)];
        let mut buf_descs = [mr.description(), mr.description()];
        let mut comp_descs = [mr.description(), mr.description()];
        let mut res_descs = [mr.description(), mr.description()];
        let rma_ioc0 = RmaIoc::new(start, 128, ofi.remote_key.as_ref().unwrap());
        let rma_ioc1 = RmaIoc::new(start + 128, 128, ofi.remote_key.as_ref().unwrap());
        let rma_iocs = [rma_ioc0, rma_ioc1];

        let mut msg = MsgType::Uninit;
        match &ofi.ep {
            MyEndpoint::Plain(_) => {
                panic!("unexpected")
                // MsgType::ConnectedMsg(MsgCompareAtomicConnected::from_ioc_slice(
                //     &buf_iocs,
                //     &mut buf_descs,
                //     &rma_iocs,
                //     CompareAtomicOp::CswapGe,
                //     0,
                // ))
            }
            MyEndpoint::Mr(ep) => {
                unsafe {
                    ofi.compare_atomicmsg_mr(
                        &mut msg,
                        ep,
                        &buf_iocs_mr,
                        &mut buf_descs,
                        &rma_iocs,
                        CompareAtomicOp::CswapGe,
                        0,
                        &comp_iocs_mr,
                        &mut res_iocs_mr,
                        &mut comp_descs,
                        &mut res_descs,
                    )
                }
            }
        }

        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        assert_eq!(res, &expected);
        // Send a done ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &ack_mem[..512], &mut desc, None),
            MyEndpoint::Mr(ep) => ofi.send_mr(ep, &ack_mem_mr.slice(..512), &mut desc, None),
        }
        ofi.cq_type.tx_cq().sread(1, -1).unwrap();
        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut ack_mem[..512]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut ack_mem_mr.slice(..512), &mut desc),
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
    } else {
        let expected = vec![2u8; 256];

        // Recv a completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.recv(ep, &mut reg_mem[512..1024]),
            MyEndpoint::Mr(ep) => ofi.recv_mr(ep, &mut mr.slice(0).slice(512..1024), &mut desc),
        }

        ofi.cq_type.rx_cq().sread(1, -1).unwrap();
        assert_eq!(&reg_mem[..256], &expected);

        // Send completion ack
        match &ofi.ep {
            MyEndpoint::Plain(ep) => ofi.send(ep, &reg_mem[512..1024], &mut desc, None),
            MyEndpoint::Mr(ep) => ofi.send_mr(ep, &mr.slice(0).slice(512..1024), &mut desc, None),
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
