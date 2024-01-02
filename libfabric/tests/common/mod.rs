use std::time::Instant;

use libfabric::{domain, fabric, FID, InfoEntry, default_desc, cq::CompletionQueueAttr, cntr::{Counter, CounterAttr}, InfoCaps, mr::MemoryRegionAttr, Context};
use libfabric::enums;
pub enum CompMeth {
    Spin,
    Sread,
    WaitSet,
    WaitFd,
    Yield,
}

pub const FT_OPT_ACTIVE: u64			= 1 << 0;
pub const FT_OPT_ITER: u64			= 1 << 1;
pub const FT_OPT_SIZE: u64			= 1 << 2;
pub const FT_OPT_RX_CQ: u64			= 1 << 3;
pub const FT_OPT_TX_CQ: u64			= 1 << 4;
pub const FT_OPT_RX_CNTR: u64			= 1 << 5;
pub const FT_OPT_TX_CNTR: u64			= 1 << 6;
pub const FT_OPT_VERIFY_DATA: u64		= 1 << 7;
pub const FT_OPT_ALIGN: u64			= 1 << 8;
pub const FT_OPT_BW: u64			= 1 << 9;
pub const FT_OPT_CQ_SHARED: u64		= 1 << 10;
pub const FT_OPT_OOB_SYNC: u64			= 1 << 11;
pub const FT_OPT_SKIP_MSG_ALLOC: u64		= 1 << 12;
pub const FT_OPT_SKIP_REG_MR: u64		= 1 << 13;
pub const FT_OPT_OOB_ADDR_EXCH: u64		= 1 << 14;
pub const FT_OPT_ALLOC_MULT_MR: u64		= 1 << 15;
pub const FT_OPT_SERVER_PERSIST: u64		= 1 << 16;
pub const FT_OPT_ENABLE_HMEM: u64		= 1 << 17;
pub const FT_OPT_USE_DEVICE: u64		= 1 << 18;
pub const FT_OPT_DOMAIN_EQ: u64		= 1 << 19;
pub const FT_OPT_FORK_CHILD: u64		= 1 << 20;
pub const FT_OPT_SRX: u64			= 1 << 21;
pub const FT_OPT_STX: u64			= 1 << 22;
pub const FT_OPT_SKIP_ADDR_EXCH: u64		= 1 << 23;
pub const FT_OPT_PERF: u64			= 1 << 24;
pub const FT_OPT_DISABLE_TAG_VALIDATION: u64	= 1 << 25;
pub const FT_OPT_ADDR_IS_OOB: u64		= 1 << 26;
pub const FT_OPT_OOB_CTRL: u64			= FT_OPT_OOB_SYNC | FT_OPT_OOB_ADDR_EXCH;

pub struct TestsGlobalCtx {
    pub tx_size: usize,
    pub rx_size: usize,
    pub tx_mr_size: usize,
    pub rx_mr_size: usize,
    pub tx_seq: u64,
    pub rx_seq: u64,
    pub tx_cq_cntr: u64,
    pub rx_cq_cntr: u64,
    pub tx_buf_size: usize,
    pub rx_buf_size: usize,
    pub buf_size: usize,
    pub buf: Vec<u8>,
    pub tx_buf_index: usize,
    pub rx_buf_index: usize,
    pub max_msg_size: usize,
    pub remote_address: libfabric::Address,
    pub ft_tag: u64, 
    pub remote_cq_data: u64, 
    pub test_sizes: Vec<usize>,
    pub window_size: usize,
    pub comp_method: CompMeth,
    pub tx_ctx: Context,
    pub rx_ctx: Context,
    pub options: u64,
}

impl TestsGlobalCtx {
    pub fn new( ) -> Self {
        let mem = Vec::new();
        TestsGlobalCtx { tx_size: 0, rx_size: 0, tx_mr_size: 0, rx_mr_size: 0, tx_seq: 0, rx_seq: 0, tx_cq_cntr: 0, rx_cq_cntr: 0, tx_buf_size: 0, rx_buf_size: 0, buf_size: 0, buf: mem, tx_buf_index: 0, rx_buf_index: 0, max_msg_size: 0, remote_address: FI_ADDR_UNSPEC, ft_tag: 0, remote_cq_data: 0,
        test_sizes: vec![
            1 << 0,
            1 << 1,
            (1 << 1) + (1 << 0),
            1 << 2,
            (1 <<  2) + (1 <<  1),
            1 <<  3,
            (1 <<  3) + (1 <<  2),
            1 <<  4,
            (1 <<  4) + (1 <<  3),
            1 <<  5,
            (1 <<  5) + (1 <<  4),
            1 <<  6,
            (1 <<  6) + (1 <<  5),
            1 <<  7,
            (1 <<  7) + (1 <<  6),
            1 <<  8,
            (1 <<  8) + (1 <<  7),
            1 <<  9,
            (1 <<  9) + (1 <<  8),
            1 << 10,
            (1 << 10) + (1 <<  9),
            1 << 11,
            (1 << 11) + (1 << 10),
            1 << 12,
            (1 << 12) + (1 << 11),
            1 << 13,
            (1 << 13) + (1 << 12),
            1 << 14,
            (1 << 14) + (1 << 13),
            1 << 15,
            (1 << 15) + (1 << 14),
            1 << 16,
            (1 << 16) + (1 << 15),
            1 << 17,
            (1 << 17) + (1 << 16),
            1 << 18,
            (1 << 18) + (1 << 17),
            1 << 19,
            (1 << 19) + (1 << 18),
            1 << 20,
            (1 << 20) + (1 << 19),
            1 << 21,
            (1 << 21) + (1 << 20),
            1 << 22,
            (1 << 22) + (1 << 21),
            1 << 23,
        ],
        window_size: 64,
        comp_method: CompMeth::Spin, 
        tx_ctx: Context::new(),
        rx_ctx: Context::new(),
        options: FT_OPT_RX_CQ | FT_OPT_TX_CQ}
    }
}

pub fn ft_open_fabric_res(info: &libfabric::InfoEntry) -> (libfabric::fabric::Fabric, libfabric::eq::EventQueue, libfabric::domain::Domain) {
    
    let fab = libfabric::fabric::Fabric::new(info.get_fabric_attr().clone()).unwrap();
    let eq = fab.eq_open(libfabric::eq::EventQueueAttr::new()).unwrap();
    let domain = fab.domain(info).unwrap();

    (fab, eq, domain)
}

pub fn ft_cq_set_wait_attr(gl_ctx: &mut TestsGlobalCtx, cq_attr: &mut CompletionQueueAttr) {
    match gl_ctx.comp_method {
        CompMeth::Spin => {
            cq_attr.wait_obj(enums::WaitObj::NONE);
        },
        CompMeth::Sread => {
            cq_attr.wait_obj(enums::WaitObj::UNSPEC);
            cq_attr.wait_cond(enums::WaitCond::NONE);
        },
        CompMeth::WaitSet => todo!(),
        CompMeth::WaitFd => {
            cq_attr.wait_obj(enums::WaitObj::YIELD);
            cq_attr.wait_cond(enums::WaitCond::NONE);
        },
        CompMeth::Yield => {
            cq_attr.wait_obj(enums::WaitObj::YIELD);
            cq_attr.wait_cond(enums::WaitCond::NONE);
        },
    }
}

pub fn ft_alloc_ep_res(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, domain: &libfabric::domain::Domain) -> (libfabric::cq::CompletionQueue, Option<libfabric::cntr::Counter>, libfabric::cq::CompletionQueue, Option<libfabric::cntr::Counter>, Option<libfabric::cntr::Counter>, Option<libfabric::av::AddressVector>){

    let mut txcq_attr =  libfabric::cq::CompletionQueueAttr::new();
    let mut rxcq_attr =  libfabric::cq::CompletionQueueAttr::new();

    // [TODO] if cq_attr.form == UNSPEC
    if info.get_caps().is_tagged() {

        txcq_attr.format(enums::CqFormat::TAGGED);
        rxcq_attr.format(enums::CqFormat::TAGGED);
    }
    else {
        txcq_attr.format(enums::CqFormat::CONTEXT);
        rxcq_attr.format(enums::CqFormat::CONTEXT);
    }

    let tx_cq;
    let rx_cq;

    if gl_ctx.options & FT_OPT_CQ_SHARED == 0{
        ft_cq_set_wait_attr(gl_ctx, &mut txcq_attr);
        ft_cq_set_wait_attr(gl_ctx, &mut rxcq_attr);

        txcq_attr.size(info.get_tx_attr().get_size() + info.get_rx_attr().get_size());
        rxcq_attr.size(info.get_tx_attr().get_size() + info.get_rx_attr().get_size());

        tx_cq = domain.cq_open(txcq_attr).unwrap();
        rx_cq = domain.cq_open(rxcq_attr).unwrap();
    }
    else {
        todo!();
    }

    let mut cntr_attr = CounterAttr::new();
    match gl_ctx.comp_method {
        CompMeth::Spin => {
            cntr_attr.wait_obj(enums::WaitObj::NONE);
        }
        CompMeth::Sread => {
            cntr_attr.wait_obj(enums::WaitObj::UNSPEC);
        }
        CompMeth::WaitSet => todo!(),
        CompMeth::WaitFd => {
            cntr_attr.wait_obj(enums::WaitObj::FD);
        }
        CompMeth::Yield => {
            cntr_attr.wait_obj(enums::WaitObj::YIELD);
        }
    }
    
    let tx_cntr = if gl_ctx.options & FT_OPT_TX_CNTR != 0{
        Some(domain.cntr_open(cntr_attr).unwrap())
    }
    else{
        None
    };

    let rx_cntr = if gl_ctx.options & FT_OPT_RX_CNTR != 0{
        Some(domain.cntr_open(cntr_attr.clone()).unwrap())
    }
    else {
        None
    };

    let rma_cntr = if gl_ctx.options & FT_OPT_RX_CNTR != 0 && info.get_caps().is_rma() {
        Some(domain.cntr_open(cntr_attr.clone()).unwrap())
    }
    else {
        None
    };

    let av = match info.get_ep_attr().get_type() {
        libfabric::enums::EndpointType::RDM | libfabric::enums::EndpointType::DGRAM  => {
                let av_attr = match info.get_domain_attr().get_av_type() {
                    libfabric::enums::AddressVectorType::UNSPEC => libfabric::av::AddressVectorAttr::new(),
                    _ => libfabric::av::AddressVectorAttr::new().type_(info.get_domain_attr().get_av_type()),
                }.count(1);
                Option::Some(domain.av_open(av_attr).unwrap())
            }
        _ => None,
    };

    (tx_cq, tx_cntr, rx_cq, rx_cntr, rma_cntr, av)
    
}

pub fn ft_alloc_active_res(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, domain: &libfabric::domain::Domain) -> (libfabric::cq::CompletionQueue, Option<libfabric::cntr::Counter>, libfabric::cq::CompletionQueue, Option<libfabric::cntr::Counter>, Option<libfabric::cntr::Counter>, libfabric::ep::Endpoint, Option<libfabric::av::AddressVector>) {
    
    let (tx_cq, tx_cntr, rx_cq, rx_cntr, rma_cntr, av) = ft_alloc_ep_res(info, gl_ctx, domain);


    let ep = domain.ep(info).unwrap();

    (tx_cq, tx_cntr, rx_cq, rx_cntr, rma_cntr, ep, av)
}

pub fn ft_enable_ep(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, ep: &libfabric::ep::Endpoint, tx_cq: &libfabric::cq::CompletionQueue, rx_cq: &libfabric::cq::CompletionQueue, eq: &libfabric::eq::EventQueue, av: &Option<libfabric::av::AddressVector>, tx_cntr: &Option<Counter>, rx_cntr: &Option<Counter>, rma_cntr: &Option<Counter>) {
    
    match info.get_ep_attr().get_type() {
        libfabric::enums::EndpointType::MSG => ep.bind(eq, 0).unwrap(),
        _ => if info.get_caps().is_collective() || info.get_caps().is_multicast() {
            ep.bind(eq, 0).unwrap();
        }
    }

    if let Some(av_val) = av { ep.bind(av_val, 0).unwrap() }

    let mut flags: u64 = libfabric_sys::FI_TRANSMIT as u64; // [TODO]
    if gl_ctx.options & FT_OPT_TX_CQ == 0 {
        flags |= libfabric_sys::FI_SELECTIVE_COMPLETION;
    }
    ep.bind(tx_cq, flags).unwrap();

    let mut flags: u64 = libfabric_sys::FI_RECV as u64; // [TODO]
    if gl_ctx.options & FT_OPT_TX_CQ == 0 {
        flags |= libfabric_sys::FI_SELECTIVE_COMPLETION;
    }
    ep.bind(rx_cq, flags).unwrap();

    let mut flags: u64 = if gl_ctx.options & FT_OPT_TX_CQ != 0 {
        0_u64
    }
    else {
        libfabric_sys::FI_SEND as u64
    };

    if info.get_caps().is_rma() || info.get_caps().is_atomic() {
        flags |= libfabric_sys::FI_WRITE as u64| libfabric_sys::FI_READ as u64 ; // [TODO]
    }

    if let Some(cntr) = tx_cntr {
        ep.bind(cntr, flags).unwrap();
    }

    let flags: u64 = if gl_ctx.options & FT_OPT_RX_CQ != 0 {
        0
    }
    else {
        libfabric_sys::FI_RECV as u64
    };


    if let Some(cntr) = rx_cntr {
        ep.bind(cntr, flags).unwrap();
    }

    if info.get_caps().is_rma() || info.get_caps().is_atomic() && info.get_caps().is_rma_event() {
        let mut flags = 0_u64;
        if info.get_caps().is_remote_write() {
            flags |= libfabric_sys::FI_REMOTE_WRITE as u64;
        }
        if info.get_caps().is_remote_read() {
            flags |= libfabric_sys::FI_REMOTE_READ as u64;
        }
        if let Some(cntr) = rma_cntr {
            ep.bind(cntr, flags).unwrap();
        }
    }

    ep.enable().unwrap();
}

pub fn ft_complete_connect(eq: &libfabric::eq::EventQueue) { // [TODO] Do not panic, return errors
    
    let mut event = 0;
    let mut eq_cm_entry = [libfabric::eq::EventQueueCmEntry::new()];
    
    let ret = eq.sread(&mut event, &mut eq_cm_entry, -1, 0).unwrap();

    if ret != std::mem::size_of::<libfabric::eq::EventQueueCmEntry>() {
        panic!("Size different {} vs {}", ret, std::mem::size_of::<libfabric::eq::EventQueueCmEntry>());
    }
    
    if event != libfabric_sys::FI_CONNECTED {
        panic!("Unexpected event value returned: {} vs {}", event, libfabric_sys::FI_CONNREQ);
    }
}

pub fn ft_accept_connection(ep: &libfabric::ep::Endpoint, eq: &libfabric::eq::EventQueue) {
    
    ep.accept().unwrap();
    
    ft_complete_connect(eq);
}

pub fn ft_retrieve_conn_req(eq: &libfabric::eq::EventQueue) -> libfabric::InfoEntry { // [TODO] Do not panic, return errors
    
    let mut event = 0;

    let mut eq_cm_entry = libfabric::eq::EventQueueCmEntry::new();
    let ret = eq.sread(&mut event, std::slice::from_mut(&mut eq_cm_entry), -1, 0).unwrap();
    if ret != std::mem::size_of::<libfabric::eq::EventQueueCmEntry>(){
        panic!("Size different {} vs {}", ret, std::mem::size_of::<libfabric::eq::EventQueueCmEntry>());
    }

    if event != libfabric_sys::FI_CONNREQ {
        panic!("Unexpected event value returned: {} vs {}", event, libfabric_sys::FI_CONNREQ);
    }

    eq_cm_entry.get_info()
}

pub fn ft_server_connect(gl_ctx: &mut TestsGlobalCtx, eq: &libfabric::eq::EventQueue, domain: &libfabric::domain::Domain) -> (libfabric::cq::CompletionQueue, libfabric::cq::CompletionQueue, Option<libfabric::cntr::Counter>, Option<libfabric::cntr::Counter>, libfabric::ep::Endpoint, Option<libfabric::mr::MemoryRegion>, Option<libfabric::mr::MemoryRegionDesc>) {

    let new_info = ft_retrieve_conn_req(eq);

    let (tx_cq, tx_cntr, rx_cq, rx_cntr, rma_cntr, ep, _) = ft_alloc_active_res(&new_info, gl_ctx, domain);
    
    let (mr, mr_desc) =  ft_enable_ep_recv(&new_info, gl_ctx,&ep, domain, &tx_cq, &rx_cq, eq, &None, &tx_cntr, &rx_cntr, &rma_cntr);

    ft_accept_connection(&ep, eq);

    (tx_cq, rx_cq, tx_cntr, rx_cntr, ep, mr, mr_desc)
}

pub fn ft_getinfo(hints: libfabric::InfoHints, node: String, service: String, flags: u64) -> libfabric::Info {
    let ep_attr = hints.get_ep_attr();


    let hints = match ep_attr.get_type() {
        libfabric::enums::EndpointType::UNSPEC => hints.ep_attr(ep_attr.ep_type(libfabric::enums::EndpointType::RDM)),
        _ => hints ,
    };

    let info = libfabric::Info::new().service(service.as_str()).flags(flags).hints(&hints);
    if node.is_empty() {
        info.request().unwrap()
    }
    else {
        info.node(node.as_str()).request().unwrap()
    }

}

pub fn ft_connect_ep(ep: &libfabric::ep::Endpoint, eq: &libfabric::eq::EventQueue, addr: &libfabric::Address) {
    
    ep.connect(addr).unwrap();
    ft_complete_connect(eq);
}

// fn ft_av_insert<T0>(addr: T0, count: size, fi_addr: libfabric::Address, flags: u64) {
//pub      a
// }


pub fn ft_rx_prefix_size(info: &libfabric::InfoEntry) -> usize {

    if info.get_rx_attr().get_mode().is_msg_prefix(){ 
        info.get_ep_attr().get_max_msg_size()
    }
    else {
        0
    }
}

pub fn ft_tx_prefix_size(info: &libfabric::InfoEntry) -> usize {

    if info.get_tx_attr().get_mode().is_msg_prefix() { 
        info.get_ep_attr().get_max_msg_size()
    }
    else {
        0
    }
}
pub const WINDOW_SIZE : usize = 64;
pub const FT_MAX_CTRL_MSG : usize = 1024;
pub const FT_RMA_SYNC_MSG_BYTES : usize = 4;
pub const FI_ADDR_UNSPEC : libfabric::Address = u64::MAX;

pub fn ft_set_tx_rx_sizes(info: &libfabric::InfoEntry, max_test_size: usize, tx_size: &mut usize, rx_size: &mut usize) {
    *tx_size = max_test_size;
    if *tx_size > info.get_ep_attr().get_max_msg_size() {
        *tx_size = info.get_ep_attr().get_max_msg_size();
    }
    println!("FT PREFIX = {}", ft_rx_prefix_size(info));
    *rx_size = *tx_size + ft_rx_prefix_size(info);
    *tx_size +=  ft_tx_prefix_size(info);
}

pub fn ft_alloc_msgs(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, domain: &libfabric::domain::Domain, ep: &libfabric::ep::Endpoint) -> (Option<libfabric::mr::MemoryRegion>, Option<libfabric::mr::MemoryRegionDesc>) {

    let alignment: usize = 64;
    ft_set_tx_rx_sizes(info, *gl_ctx.test_sizes.last().unwrap(), &mut gl_ctx.tx_size, &mut gl_ctx.rx_size);
    gl_ctx.rx_buf_size = std::cmp::max(gl_ctx.rx_size, FT_MAX_CTRL_MSG) * WINDOW_SIZE;
    gl_ctx.tx_buf_size = std::cmp::max(gl_ctx.tx_size, FT_MAX_CTRL_MSG) * WINDOW_SIZE;


    let rma_resv_bytes = FT_RMA_SYNC_MSG_BYTES + std::cmp::max(ft_tx_prefix_size(info), ft_rx_prefix_size(info));
    gl_ctx.tx_buf_size += rma_resv_bytes;
    gl_ctx.rx_buf_size += rma_resv_bytes;

    gl_ctx.buf_size = gl_ctx.rx_buf_size + gl_ctx.tx_buf_size;

    gl_ctx.buf_size += alignment;
    gl_ctx.buf.resize(gl_ctx.buf_size, 0);
    println!("Buf size: {}", gl_ctx.buf_size);
    gl_ctx.max_msg_size = gl_ctx.tx_size;
    
    gl_ctx.rx_buf_index = 0;
    println!("rx_buf_index: {}", gl_ctx.rx_buf_index);
    gl_ctx.tx_buf_index = gl_ctx.rx_buf_size;
    println!("tx_buf_index: {}", gl_ctx.tx_buf_index);

    gl_ctx.remote_cq_data = ft_init_cq_data(info);

    ft_reg_mr(info, domain, ep, &mut gl_ctx.buf, 0xC0DE)
}

pub fn ft_alloc_ctx_array(gl_ctx: &mut TestsGlobalCtx) {



}


pub fn ft_enable_ep_recv(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, ep: &libfabric::ep::Endpoint, domain: &libfabric::domain::Domain, tx_cq: &libfabric::cq::CompletionQueue, rx_cq: &libfabric::cq::CompletionQueue, eq: &libfabric::eq::EventQueue, av: &Option<libfabric::av::AddressVector>, tx_cntr: &Option<Counter>, rx_cntr: &Option<Counter>, rma_cntr: &Option<Counter>) -> (Option<libfabric::mr::MemoryRegion>, Option<libfabric::mr::MemoryRegionDesc>) {
    
    ft_enable_ep(info, gl_ctx, ep, tx_cq, rx_cq, eq, av, tx_cntr, rx_cntr, rma_cntr);

    let (mr, mut data_desc) =  ft_alloc_msgs(info, gl_ctx, domain, ep);

    ft_post_rx(info, gl_ctx, ep, gl_ctx.remote_address, std::cmp::max(FT_MAX_CTRL_MSG, gl_ctx.rx_size), NO_CQ_DATA, &mut data_desc, rx_cq);

    (mr, data_desc)
}

pub fn ft_init_fabric(hints: libfabric::InfoHints, gl_ctx: &mut TestsGlobalCtx, node: String, service: String, flags: u64) -> (libfabric::Info, libfabric::fabric::Fabric, libfabric::ep::Endpoint, libfabric::domain::Domain, libfabric::cq::CompletionQueue, libfabric::cq::CompletionQueue, Option<Counter>, Option<Counter>, libfabric::eq::EventQueue, Option<libfabric::mr::MemoryRegion>, libfabric::av::AddressVector, Option<libfabric::mr::MemoryRegionDesc>) {
    
    let info = ft_getinfo(hints, node.clone(), service.clone(), flags);
    let entries: Vec<libfabric::InfoEntry> = info.get();
    
    if entries.is_empty() {
        panic!("No entires in fi_info");
    }
        
    let (fabric, eq, domain) = ft_open_fabric_res(&entries[0]);

    let (tx_cq, tx_cntr, rx_cq, rx_cntr, rma_ctr, ep, av) =  ft_alloc_active_res(&entries[0], gl_ctx, &domain);

    let (mr, mut mr_desc)  = ft_enable_ep_recv(&entries[0], gl_ctx, &ep, &domain, &tx_cq, &rx_cq, &eq, &av, &tx_cntr, &rx_cntr, &rma_ctr);
    println!("Passed enable_ep_recv");
    let av = av.unwrap();
    ft_init_av(&entries[0], gl_ctx, &av , &ep, &tx_cq, &rx_cq, &tx_cntr, &rx_cntr,&mut mr_desc, node.is_empty());

    (info, fabric, ep, domain, tx_cq, rx_cq, tx_cntr, rx_cntr, eq, mr, av, mr_desc)
}

pub fn ft_av_insert<T>(av: &libfabric::av::AddressVector, addr: &T, fi_addr: &mut libfabric::Address, flags: u64) {

    let len_added = av.insert(std::slice::from_ref(addr), fi_addr, flags).unwrap();
    if len_added != 1 {
        panic!("Could not add address to address vector");
    }
}

pub const NO_CQ_DATA: u64 = 0;


macro_rules!  ft_post{
    ($post_fn:ident, $prog_fn:ident, $cq:ident, $seq:expr, $cq_cntr:expr, $op_str:literal, $ep:ident, $( $x:ident),* ) => {
        loop {
            let ret = $ep.$post_fn($($x,)*);
            if matches!(ret, Ok(_)) {
                break;
            }
            else if let Err(ref err) = ret {
                if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                    ret.unwrap();
                }

            }
            let rc = $prog_fn($cq, $seq, $cq_cntr);
            if rc != 0 && -rc as u32 != libfabric_sys::FI_EAGAIN {
                panic!("{} returned error", stringify!(prog_fn));
            }
        }
        $seq+=1;
    };
}

#[allow(non_camel_case_types)]
pub enum RmaOp {
    RMA_WRITE,
    RMA_WRITEDATA,
    RMA_READ,
}

pub fn ft_init_cq_data(info: &InfoEntry) -> u64 {
    if info.get_domain_attr().get_cq_data_size() >= std::mem::size_of::<u64>() as u64 {
        0x0123456789abcdef_u64
    }
    else {
        0x0123456789abcdef & (1 << (info.get_domain_attr().get_cq_data_size() * 8)) - 1
    }
}

pub fn ft_post_rma_inject(_info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, rma_op: &RmaOp, offset: usize, size: usize, remote: &libfabric::RmaIoVec, ep: &libfabric::ep::Endpoint, fi_addr: libfabric::Address, tx_cq: &libfabric::cq::CompletionQueue) {
    match rma_op {
        
        RmaOp::RMA_WRITE => {
            let addr = remote.get_address() + offset as u64;
            let key = remote.get_key();
            let buf = &gl_ctx.buf[gl_ctx.tx_buf_index+offset..gl_ctx.tx_buf_index+offset+size];
            unsafe{ ft_post!(inject_write, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "fi_write", ep, buf, fi_addr, addr, key); }
        }

        RmaOp::RMA_WRITEDATA => {
            let addr = remote.get_address() + offset as u64;
            let key = remote.get_key();
            let buf = &gl_ctx.buf[gl_ctx.tx_buf_index+offset..gl_ctx.tx_buf_index+offset+size];
            let remote_cq_data = gl_ctx.remote_cq_data;
            unsafe{ ft_post!(inject_writedata, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "fi_write", ep, buf, remote_cq_data, fi_addr, addr, key); }
        }
        RmaOp::RMA_READ => {
            panic!("ft_post_rma_inject does not support read");
        }
    }

    gl_ctx.tx_cq_cntr+=1;
}

pub fn ft_post_rma(_info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, rma_op: &RmaOp, offset: usize, size: usize, remote: &libfabric::RmaIoVec, ep: &libfabric::ep::Endpoint, fi_addr: libfabric::Address, data_desc: &mut impl libfabric::DataDescriptor, tx_cq: &libfabric::cq::CompletionQueue) {
    
    match rma_op {
        
        RmaOp::RMA_WRITE => {
            let addr = remote.get_address() + offset as u64;
            let key = remote.get_key();
            let buf = &gl_ctx.buf[gl_ctx.tx_buf_index+offset..gl_ctx.tx_buf_index+offset+size];
            unsafe{ ft_post!(write, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "fi_write", ep, buf, data_desc, fi_addr, addr, key); }
        }

        RmaOp::RMA_WRITEDATA => {
            let addr = remote.get_address() + offset as u64;
            let key = remote.get_key();
            let buf = &gl_ctx.buf[gl_ctx.tx_buf_index+offset..gl_ctx.tx_buf_index+offset+size];
            let remote_cq_data = gl_ctx.remote_cq_data;
            unsafe{ ft_post!(writedata, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "fi_write", ep, buf, data_desc, remote_cq_data, fi_addr, addr, key); }
        }
        
        RmaOp::RMA_READ => {
            let addr = remote.get_address() + offset as u64;
            let key = remote.get_key();
            let buf = &mut gl_ctx.buf[gl_ctx.tx_buf_index+offset..gl_ctx.tx_buf_index+offset+size];
            let _remote_cq_data = gl_ctx.remote_cq_data;
            unsafe{ ft_post!(read, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "fi_write", ep, buf, data_desc, fi_addr, addr, key); }
        }
    }
}


pub fn ft_post_tx(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, ep: &libfabric::ep::Endpoint, fi_addr: libfabric::Address, mut size: usize, data: u64, data_desc: &mut Option<libfabric::mr::MemoryRegionDesc>, tx_cq: &libfabric::cq::CompletionQueue) {
    
    size += ft_tx_prefix_size(info);
    let buf = &mut gl_ctx.buf[gl_ctx.tx_buf_index..gl_ctx.tx_buf_index+size];
    if info.get_caps().is_tagged() {
        let op_tag = if gl_ctx.ft_tag != 0 {gl_ctx.ft_tag} else {gl_ctx.tx_seq};

        if data != NO_CQ_DATA {
            if let Some(mr_desc) = data_desc.as_mut() {
                ft_post!(tsenddata, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "transmit", ep, buf, mr_desc, data, fi_addr, op_tag);
            }
            else {
                let mr_desc = &mut libfabric::default_desc();
                ft_post!(tsenddata, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "transmit", ep, buf, mr_desc, data, fi_addr, op_tag);
            }
        }
        else {
            if let Some(mr_desc) = data_desc.as_mut() {
                ft_post!(tsend, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "transmit", ep, buf, mr_desc, fi_addr, op_tag);
            }
            else {
                let mr_desc = &mut libfabric::default_desc();
                ft_post!(tsend, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "transmit", ep, buf, mr_desc, fi_addr, op_tag);
            }
        }
    }
    else {
        if data != NO_CQ_DATA {
            if let Some(mr_desc) = data_desc.as_mut() {
                ft_post!(senddata, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "transmit", ep, buf, mr_desc, data, fi_addr);
            }
            else {
                let mr_desc = &mut libfabric::default_desc();
                ft_post!(senddata, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "transmit", ep, buf, mr_desc, data, fi_addr);
            }
        }
        else {
            if let Some(mr_desc) = data_desc.as_mut() {
                ft_post!(send, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "transmit", ep, buf, mr_desc, fi_addr);
            }
            else {
                let mr_desc = &mut libfabric::default_desc();
                ft_post!(send, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "transmit", ep, buf, mr_desc, fi_addr);
            }
        }
    }
}

pub fn ft_tx(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, ep: &libfabric::ep::Endpoint, fi_addr: libfabric::Address, size: usize, data_desc: &mut Option<libfabric::mr::MemoryRegionDesc>, tx_cq: &libfabric::cq::CompletionQueue, tx_cntr: &Option<Counter>) {

    ft_post_tx(info, gl_ctx, ep, fi_addr, size, NO_CQ_DATA, data_desc, tx_cq);
    ft_get_tx_comp(gl_ctx, tx_cntr, tx_cq, gl_ctx.tx_seq);
}

pub fn ft_post_rx(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, ep: &libfabric::ep::Endpoint, fi_addr: libfabric::Address, mut size: usize, _data: u64, data_desc: &mut Option<libfabric::mr::MemoryRegionDesc>, rx_cq: &libfabric::cq::CompletionQueue) {
    size = std::cmp::max(size, FT_MAX_CTRL_MSG) +  ft_tx_prefix_size(info);
    let buf = &mut gl_ctx.buf[gl_ctx.rx_buf_index..gl_ctx.rx_buf_index+size];
    if info.get_caps().is_tagged() {
        let op_tag = if gl_ctx.ft_tag != 0 {gl_ctx.ft_tag} else {gl_ctx.rx_seq};
        let zero = 0;
        if let Some(mr_desc) = data_desc.as_mut() {
            ft_post!(trecv, ft_progress, rx_cq, gl_ctx.rx_seq, &mut gl_ctx.rx_cq_cntr, "receive", ep, buf, mr_desc, fi_addr, op_tag, zero);
        }
        else {
            let mr_desc = &mut libfabric::default_desc();
            ft_post!(trecv, ft_progress, rx_cq, gl_ctx.rx_seq, &mut gl_ctx.rx_cq_cntr, "receive", ep, buf, mr_desc, fi_addr, op_tag, zero);
        }
    }
    else {
        if let Some(mr_desc) = data_desc.as_mut() {

            ft_post!(recv, ft_progress, rx_cq, gl_ctx.rx_seq, &mut gl_ctx.rx_cq_cntr, "receive", ep, buf, mr_desc, fi_addr);
        }
        else {
            let mr_desc = &mut libfabric::default_desc();
            ft_post!(recv, ft_progress, rx_cq, gl_ctx.rx_seq, &mut gl_ctx.rx_cq_cntr, "receive", ep, buf, mr_desc, fi_addr);
        }
    }
}

pub fn ft_rx(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, ep: &libfabric::ep::Endpoint, fi_addr: libfabric::Address, _size: usize, data_desc: &mut Option<libfabric::mr::MemoryRegionDesc>, rx_cq: &libfabric::cq::CompletionQueue, rx_cntr: &Option<libfabric::cntr::Counter>) {

    ft_get_rx_comp(gl_ctx, rx_cntr, rx_cq, gl_ctx.rx_seq);
    ft_post_rx(info, gl_ctx, ep, fi_addr, gl_ctx.rx_size, NO_CQ_DATA, data_desc, rx_cq);
}

pub fn ft_post_inject(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, ep: &libfabric::ep::Endpoint, fi_addr: libfabric::Address, mut size: usize, tx_cq: &libfabric::cq::CompletionQueue) {
    size += ft_tx_prefix_size(info);
    let buf = &mut gl_ctx.buf[gl_ctx.tx_buf_index..gl_ctx.tx_buf_index+size];
    
    if info.get_caps().is_tagged() {
        let tag = gl_ctx.tx_seq;

        ft_post!(tinject, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "inject", ep, buf, fi_addr, tag);
    }
    else {
        ft_post!(inject, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "inject", ep, buf, fi_addr);
    }

    gl_ctx.tx_cq_cntr += 1;
}

pub fn ft_inject(info: &InfoEntry, gl_ctx: &mut TestsGlobalCtx, ep: &libfabric::ep::Endpoint, fi_addr: libfabric::Address, size: usize, tx_cq: &libfabric::cq::CompletionQueue) {
    ft_post_inject(info, gl_ctx, ep, fi_addr, size, tx_cq);
}

pub fn ft_progress(cq: &libfabric::cq::CompletionQueue, _total: u64, cq_cntr: &mut u64) -> isize {
    let mut cq_err_entry = libfabric::cq::CqErrEntry::new();
    let ret = cq.read(std::slice::from_mut(&mut cq_err_entry), 1);
    match ret {
        Ok(_) => {*cq_cntr += 1;},
        Err(ref err) => {
            if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                ret.unwrap();
            }
        }
    }

    0
}

pub fn ft_init_av_dst_addr(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx,  av: &libfabric::av::AddressVector, ep: &libfabric::ep::Endpoint, tx_cq: &libfabric::cq::CompletionQueue, rx_cq: &libfabric::cq::CompletionQueue, tx_cntr: &Option<Counter>, rx_cntr: &Option<Counter>, data_desc: &mut Option<libfabric::mr::MemoryRegionDesc>, server: bool) {
    let mut v = [0_u8; FT_MAX_CTRL_MSG];
    if !server {
        ft_av_insert(av, info.get_dest_addr::<libfabric::Address>(), &mut gl_ctx.remote_address, 0);
        let len = ep.getname(&mut v).unwrap();
        
        gl_ctx.buf[gl_ctx.tx_buf_index..gl_ctx.tx_buf_index+len].copy_from_slice(&v[0..len]);

        ft_tx(info, gl_ctx, ep, gl_ctx.remote_address, len, data_desc, tx_cq, tx_cntr);
        ft_rx(info, gl_ctx, ep, gl_ctx.remote_address, 1, data_desc, rx_cq, rx_cntr);

    }
    else {
        ft_get_rx_comp(gl_ctx, rx_cntr, rx_cq, gl_ctx.rx_seq);
        v.copy_from_slice(&gl_ctx.buf[gl_ctx.rx_buf_index..gl_ctx.rx_buf_index+FT_MAX_CTRL_MSG]);


        if matches!(info.get_domain_attr().get_av_type(), libfabric::enums::AddressVectorType::TABLE ) {
            let mut zero = 0;
            ft_av_insert(av, &v, &mut zero, 0);
        }
        else {
            ft_av_insert(av, &v, &mut gl_ctx.remote_address, 0);
        }
        
        ft_post_rx(info, gl_ctx, ep, gl_ctx.remote_address, 1, NO_CQ_DATA,  data_desc, rx_cq);
        
        if matches!(info.get_domain_attr().get_av_type(), libfabric::enums::AddressVectorType::TABLE) {
            gl_ctx.remote_address = 0;
        }
        
        ft_tx(info, gl_ctx, ep, gl_ctx.remote_address, 1, data_desc, tx_cq, tx_cntr);
    }
}

pub fn ft_init_av(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, av: &libfabric::av::AddressVector, ep: &libfabric::ep::Endpoint, tx_cq: &libfabric::cq::CompletionQueue, rx_cq: &libfabric::cq::CompletionQueue, tx_cntr: &Option<Counter>, rx_cntr: &Option<Counter>, mr_desc: &mut Option<libfabric::mr::MemoryRegionDesc>, server: bool) {

    ft_init_av_dst_addr(info, gl_ctx, av, ep,  tx_cq, rx_cq, tx_cntr, rx_cntr, mr_desc, server);
}

pub fn ft_spin_for_comp(cq: &libfabric::cq::CompletionQueue, curr: &mut u64, total: u64, _timeout: i32, _tag: u64) {
    
    let mut comp = libfabric::cq::CqErrEntry::new();

    while total - *curr > 0 {
        loop {

            let err = cq.read(std::slice::from_mut(&mut comp), 1);
            match err {
                Ok(_) => {
                    break
                },
                Err(err) => {
                    if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                        let mut err_entry = libfabric::cq::CqErrEntry::new();
                        cq.readerr(&mut err_entry, 0).unwrap();
            
                        cq.print_error(&err_entry);
                        panic!("ERROR IN CQ_READ {}", err);
                    }
                     
                }
            }
        }
        *curr += 1;
    }
}

pub fn ft_wait_for_comp(cq: &libfabric::cq::CompletionQueue, curr: &mut u64, total: u64, _timeout: i32, _tag: u64) {
    
    let mut comp = libfabric::cq::CqErrEntry::new();

    while total - *curr > 0 {
        let ret = cq.sread(std::slice::from_mut(&mut comp), 1, -1);
        if matches!(ret, Ok(_)) {
            *curr += 1;
        }
    }
}

pub fn ft_read_cq(comp: &CompMeth, cq: &libfabric::cq::CompletionQueue, curr: &mut u64, total: u64, timeout: i32, tag: u64) {

    match comp {
        CompMeth::Spin => {
            ft_spin_for_comp(cq, curr, total, timeout, tag);
        },
        CompMeth::WaitSet => todo!(),
        CompMeth::WaitFd => todo!(),
        CompMeth::Sread | CompMeth::Yield => {
            ft_wait_for_comp(cq, curr, total, timeout, tag);
        }
    }

}

pub fn ft_spin_for_cntr(cntr: &Option<Counter>, total: u64) {

    if let Some(cntr_v) = cntr {
        loop {
            let cur = cntr_v.read();
            if cur >= total {
                break;
            }
        }
    }
    else {
        panic!("Trying to wait on cntr without setting it up");
    }
}

pub fn ft_wait_for_cntr(cntr: &Option<Counter>, total: u64) {

    if let Some(cntr_v) = cntr {
        while total > cntr_v.read() {
            let ret = cntr_v.wait(total, -1);
            if matches!(ret, Ok(())) {
                break;
            }
        }
    }
    else {
        panic!("Trying to wait on cntr without setting it up");
    }
}

pub fn ft_get_cq_comp(comp: &CompMeth, rx_curr: &mut u64, rx_cq: &libfabric::cq::CompletionQueue, total: u64) {
    ft_read_cq(comp, rx_cq, rx_curr, total, -1, 0);
}

pub fn ft_get_cntr_comp(comp: &CompMeth, cntr: &Option<Counter>, total: u64) {
    
    match comp {
        CompMeth::Spin => ft_spin_for_cntr(cntr, total),
        _ => ft_wait_for_cntr(cntr, total),
    }
}

pub fn ft_get_rx_comp(gl_ctx: &mut TestsGlobalCtx, rx_cntr: &Option<Counter>, rx_cq: &libfabric::cq::CompletionQueue, total: u64) {

    if gl_ctx.options & FT_OPT_RX_CQ != 0{
        ft_get_cq_comp(&gl_ctx.comp_method,&mut gl_ctx.rx_cq_cntr, rx_cq, total);
    }
    else {
        ft_get_cntr_comp(&gl_ctx.comp_method, rx_cntr, total);
    }
}

pub fn ft_get_tx_comp(gl_ctx: &mut TestsGlobalCtx, tx_cntr: &Option<Counter>, tx_cq: &libfabric::cq::CompletionQueue, total: u64) {

    if gl_ctx.options & FT_OPT_RX_CQ != 0{
        ft_get_cq_comp(&gl_ctx.comp_method, &mut gl_ctx.tx_cq_cntr, tx_cq, total);
    }
    else {
        ft_get_cntr_comp(&gl_ctx.comp_method, tx_cntr, total);
    }
}

pub fn ft_need_mr_reg(info: &libfabric::InfoEntry) -> bool {

    if info.get_caps().is_rma() || info.get_caps().is_atomic() {
        true
    }
    else {
        info.get_domain_attr().get_mr_mode().is_local()
    }
}

pub fn ft_chek_mr_local_flag(info: &libfabric::InfoEntry) -> bool {
    info.get_mode().is_local_mr() || info.get_domain_attr().get_mr_mode().is_local()
}

pub fn ft_rma_read_target_allowed(caps: &InfoCaps) -> bool {
    if caps.is_rma() || caps.is_atomic() {
        if caps.is_remote_read() {
            return true
        }
        else {
            return ! (caps.is_read() || caps.is_write() || caps.is_remote_write());
        }
    }

    false
}

pub fn ft_rma_write_target_allowed(caps: &InfoCaps) -> bool {
    if caps.is_rma() || caps.is_atomic() {
        if caps.is_remote_write() {
            return true
        }
        else {
            return ! (caps.is_read() || caps.is_write() || caps.is_remote_write());
        }
    }

    false
}

pub fn ft_info_to_mr_attr(info: &InfoEntry) -> libfabric::mr::MemoryRegionAttr {

    let mut mr_attr = libfabric::mr::MemoryRegionAttr::new();

    if ft_chek_mr_local_flag(info) {
        if info.get_caps().is_msg() || info.get_caps().is_tagged() {
            let mut temp = info.get_caps().is_send();
            if temp {
                mr_attr = mr_attr.access_send();
            }
            temp |= info.get_caps().is_recv();
            if temp {
                mr_attr = mr_attr.access_recv();
            }
            if !temp {
                mr_attr = mr_attr.access_send().access_recv();
            }
        }
    }
    else {
        if info.get_caps().is_rma() || info.get_caps().is_atomic() {
            if ft_rma_read_target_allowed(info.get_caps()) {
                mr_attr = mr_attr.access_remote_read();
            }
            if ft_rma_write_target_allowed(info.get_caps()) {
                mr_attr = mr_attr.access_remote_write();
            }
        }
    }

    mr_attr
}

pub fn ft_reg_mr(info: &libfabric::InfoEntry, domain: &libfabric::domain::Domain, ep: &libfabric::ep::Endpoint, buf: &mut [u8], key: u64) -> (Option<libfabric::mr::MemoryRegion>, Option<libfabric::mr::MemoryRegionDesc>) {

    if ! ft_need_mr_reg(info) {
        println!("MR not needed");
        return (None, None)
    }
    let iov = libfabric::IoVec::new(buf);
    // let mut mr_attr = libfabric::mr::MemoryRegionAttr::new().iov(std::slice::from_ref(&iov)).requested_key(key).iface(libfabric::enums::HmemIface::SYSTEM);
    
    let mut mr_attr = ft_info_to_mr_attr(info).iov(std::slice::from_ref(&iov)).requested_key(key).iface(libfabric::enums::HmemIface::SYSTEM);
    // if ft_chek_mr_local_flag(info) {

    //     if info.get_caps().is_msg() || info.get_caps().is_tagged() {
    //         let mut temp = info.get_caps().is_send();
    //         if temp {
    //             mr_attr = mr_attr.access_send();
    //         }
    //         temp |= info.get_caps().is_recv();
    //         if temp {
    //             mr_attr = mr_attr.access_recv();
    //         }
    //         if !temp {
    //             mr_attr = mr_attr.access_send().access_recv();
    //         }
    //     }
    // } 

    let mr = domain.mr_regattr(mr_attr, 0).unwrap();
    let desc = mr.description();

    if info.get_domain_attr().get_mr_mode().is_endpoint() {
        println!("MR ENDPOINT");
        mr.bind(ep, 0).unwrap();
    }

    (mr.into(),desc.into())

}

pub fn ft_sync(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, tx_cq: &libfabric::cq::CompletionQueue, rx_cq: &libfabric::cq::CompletionQueue, tx_cntr: &Option<Counter>, rx_cntr: &Option<Counter>, ep: &libfabric::ep::Endpoint, mr_desc: &mut Option<libfabric::mr::MemoryRegionDesc>) {
    
    // println!("TX SEQ: {},  TX_CTR: {}", gl_ctx.tx_seq, gl_ctx.tx_cq_cntr);
    // println!("RX SEQ: {},  RX_CTR: {}", gl_ctx.rx_seq, gl_ctx.rx_cq_cntr);
    ft_tx(info, gl_ctx, ep, gl_ctx.remote_address, 1, mr_desc, tx_cq, tx_cntr);
    ft_rx(info, gl_ctx, ep, gl_ctx.remote_address, 1, mr_desc, rx_cq, rx_cntr);
}

pub fn ft_exchange_keys(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, mr: &mut libfabric::mr::MemoryRegion, tx_cq: &libfabric::cq::CompletionQueue, rx_cq: &libfabric::cq::CompletionQueue, tx_cntr: &Option<Counter>, rx_cntr: &Option<Counter>, domain: &libfabric::domain::Domain, ep: &libfabric::ep::Endpoint, mr_desc: &mut Option<libfabric::mr::MemoryRegionDesc>) -> libfabric::RmaIoVec{
    let mut addr = 0; 
    let mut key_size = 0;
    let len ;
    let mut rma_iov = libfabric::RmaIoVec::new();
    
    if info.get_domain_attr().get_mr_mode().is_raw() { 
        mr.raw_attr(&mut addr, &mut key_size, 0).unwrap(); // [TODO] Change this to return base_addr, key_size
    }

    len = std::mem::size_of::<libfabric::RmaIoVec>();
    if key_size >= len - std::mem::size_of_val(&rma_iov.get_key()) {
        panic!("Key size does not fit");
    }

    if info.get_domain_attr().get_mr_mode().is_basic() || info.get_domain_attr().get_mr_mode().is_virt_addr() {
        addr = gl_ctx.buf[gl_ctx.rx_buf_index..gl_ctx.rx_buf_index + ft_rx_prefix_size(info)].as_mut_ptr() as u64;
        rma_iov = rma_iov.address(addr);
    }
    
    if info.get_domain_attr().get_mr_mode().is_raw() {
        mr.raw_attr_with_key(&mut addr, &mut (rma_iov.get_key() as u8), &mut key_size, 0).unwrap();
    }
    else {
        rma_iov = rma_iov.key(mr.get_key());
    }
    
    gl_ctx.buf[gl_ctx.tx_buf_index..gl_ctx.tx_buf_index+len].copy_from_slice(unsafe{ std::slice::from_raw_parts(&rma_iov as *const libfabric::RmaIoVec as *const u8, std::mem::size_of::<libfabric::RmaIoVec>())});
    
    ft_tx(info, gl_ctx, ep, gl_ctx.remote_address, len + ft_tx_prefix_size(info), mr_desc, tx_cq, tx_cntr);
    ft_get_rx_comp(gl_ctx, rx_cntr, rx_cq, gl_ctx.rx_seq);
    
    unsafe{ std::slice::from_raw_parts_mut(&mut rma_iov as *mut libfabric::RmaIoVec as *mut u8,std::mem::size_of::<libfabric::RmaIoVec>())}.copy_from_slice(&gl_ctx.buf[gl_ctx.rx_buf_index..gl_ctx.rx_buf_index+len]);
    let mut peer_iov = libfabric::RmaIoVec::new();
    if info.get_domain_attr().get_mr_mode().is_raw() {
        peer_iov = peer_iov.address(rma_iov.get_address());
        peer_iov = peer_iov.len(rma_iov.get_len());
        let mut key = 0;
        domain.map_raw(rma_iov.get_address(), &mut (rma_iov.get_key() as u8), key_size, &mut key, 0).unwrap();
        peer_iov = peer_iov.key(key);
    }
    else {
        peer_iov = rma_iov.clone();
    }
    
    ft_post_rx(info, gl_ctx, ep, gl_ctx.remote_address, gl_ctx.rx_size, NO_CQ_DATA, mr_desc, rx_cq);
    ft_sync(info, gl_ctx,  tx_cq, rx_cq, tx_cntr, rx_cntr, ep, mr_desc);

    peer_iov
}

pub fn start_server(hints: libfabric::InfoHints) -> (libfabric::Info, fabric::Fabric,  libfabric::domain::Domain, libfabric::eq::EventQueue, libfabric::ep::PassiveEndpoint) {
   
   let info = ft_getinfo(hints, "".to_owned(), "9222".to_owned(), libfabric_sys::FI_SOURCE);
   let entries = info.get();
    
    if entries.is_empty() {
        panic!("No entires in fi_info");
    }

    let (fab, eq, domain) = ft_open_fabric_res(&entries[0]);


    let pep = fab.passive_ep(&entries[0]).unwrap();
        pep.bind(&eq, 0).unwrap();
        pep.listen().unwrap();


    (info, fab, domain, eq, pep)
}

pub fn ft_client_connect(hints: libfabric::InfoHints, gl_ctx: &mut TestsGlobalCtx, node: String, service: String) -> (libfabric::Info, fabric::Fabric,  domain::Domain, libfabric::eq::EventQueue, libfabric::cq::CompletionQueue, libfabric::cq::CompletionQueue, Option<libfabric::cntr::Counter>, Option<libfabric::cntr::Counter>, libfabric::ep::Endpoint, Option<libfabric::mr::MemoryRegion>, Option<libfabric::mr::MemoryRegionDesc>) {
    let info = ft_getinfo(hints, node, service, 0);

    let entries: Vec<libfabric::InfoEntry> = info.get();

    if entries.is_empty() {
        panic!("No entires in fi_info");
    }

    let (fab, eq, domain) = ft_open_fabric_res(&entries[0]);
    let (tx_cq, tx_cntr, rx_cq, rx_cntr, rma_cntr, ep, _) = ft_alloc_active_res(&entries[0], gl_ctx,&domain);
    let (mr, mr_desc)  = ft_enable_ep_recv(&entries[0], gl_ctx, &ep, &domain, &tx_cq, &rx_cq, &eq, &None, &tx_cntr, &rx_cntr, &rma_cntr);
    ft_connect_ep(&ep, &eq, entries[0].get_dest_addr::<libfabric::Address>());

    
    (info, fab, domain, eq, rx_cq, tx_cq, tx_cntr, rx_cntr, ep, mr, mr_desc)
}

pub fn ft_finalize_ep(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, ep: &libfabric::ep::Endpoint, data_desc: &mut Option<libfabric::mr::MemoryRegionDesc>, tx_cq: &libfabric::cq::CompletionQueue, rx_cq: &libfabric::cq::CompletionQueue, tx_cntr: &Option<Counter>, rx_cntr: &Option<Counter>) {

    let base = &mut gl_ctx.buf[gl_ctx.tx_buf_index..gl_ctx.tx_buf_index + 4 + ft_tx_prefix_size(info)];
    let iov = libfabric::IoVec::new(base);

    if info.get_caps().is_tagged() {
        let msg = if let Some(mr_desc) = data_desc.as_mut() {
            libfabric::MsgTagged::new(std::slice::from_ref(&iov), mr_desc, gl_ctx.remote_address, 0, gl_ctx.tx_seq, 0)
        }
        else {
            libfabric::MsgTagged::new(std::slice::from_ref(&iov), &mut default_desc(), gl_ctx.remote_address, 0, gl_ctx.tx_seq, 0)
        };
        let msg_ref = &msg;
        let flag = libfabric::enums::TransferOptions::TRANSMIT_COMPLETE;

        ft_post!(tsendmsg, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "sendmsg", ep, msg_ref, flag);
    }
    else {
        let msg = if let Some(mr_desc) = data_desc.as_mut() {
            libfabric::Msg::new(std::slice::from_ref(&iov), mr_desc, gl_ctx.remote_address)
        }
        else {
            libfabric::Msg::new(std::slice::from_ref(&iov), &mut default_desc(), gl_ctx.remote_address)
        };

        let msg_ref = &msg;
        let flag = libfabric::enums::TransferOptions::TRANSMIT_COMPLETE;
        ft_post!(sendmsg, ft_progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "sendmsg", ep, msg_ref, flag);
    }

    ft_get_tx_comp(gl_ctx, tx_cntr, tx_cq, gl_ctx.tx_seq);
    ft_get_rx_comp(gl_ctx, rx_cntr, rx_cq, gl_ctx.rx_seq);

}

pub fn ft_finalize(info: &libfabric::InfoEntry, gl_ctx: &mut TestsGlobalCtx, ep: &libfabric::ep::Endpoint, domain: &libfabric::domain::Domain, tx_cq: &libfabric::cq::CompletionQueue, rx_cq: &libfabric::cq::CompletionQueue, tx_cntr: &Option<Counter>, rx_cntr: &Option<Counter>, data_desc: &mut Option<libfabric::mr::MemoryRegionDesc>) {

    if info.get_domain_attr().get_mr_mode().is_raw() { 
        domain.unmap_key(0xC0DE).unwrap();
    }

    ft_finalize_ep(info, gl_ctx, ep, data_desc, tx_cq, rx_cq, tx_cntr, rx_cntr);
}

pub fn close_all_pep(fab: libfabric::fabric::Fabric, domain: libfabric::domain::Domain, eq :libfabric::eq::EventQueue, rx_cq: libfabric::cq::CompletionQueue, tx_cq: libfabric::cq::CompletionQueue, ep: libfabric::ep::Endpoint, pep: libfabric::ep::PassiveEndpoint, mr: Option<libfabric::mr::MemoryRegion>) {
    ep.close().unwrap();
    pep.close().unwrap();
    eq.close().unwrap();
    tx_cq.close().unwrap();
    rx_cq.close().unwrap();
    if let Some(mr_val) = mr { mr_val.close().unwrap(); }
    domain.close().unwrap();
    fab.close().unwrap();        
}

pub fn close_all(fab: libfabric::fabric::Fabric, domain: libfabric::domain::Domain, eq :libfabric::eq::EventQueue, rx_cq: libfabric::cq::CompletionQueue, tx_cq: libfabric::cq::CompletionQueue, tx_cntr: Option<Counter>, rx_cntr: Option<Counter>, ep: libfabric::ep::Endpoint, mr: Option<libfabric::mr::MemoryRegion>, av: Option<libfabric::av::AddressVector>) {
    
    ep.close().unwrap();
    eq.close().unwrap();
    tx_cq.close().unwrap();
    rx_cq.close().unwrap();
    if let Some(mr_val) = mr { mr_val.close().unwrap() }
    if let Some(av_val) = av { av_val.close().unwrap() }
    if let Some(rxcntr_val) = rx_cntr { rxcntr_val.close().unwrap() }
    if let Some(txcnr_val) = tx_cntr { txcnr_val.close().unwrap() }
    domain.close().unwrap();
    fab.close().unwrap();    
}

pub fn pingpong(info: &InfoEntry, gl_ctx: &mut TestsGlobalCtx, tx_cq: &libfabric::cq::CompletionQueue, rx_cq: &libfabric::cq::CompletionQueue, tx_cntr: &Option<Counter>, rx_cntr: &Option<Counter>, ep: &libfabric::ep::Endpoint, mr_desc: &mut Option<libfabric::mr::MemoryRegionDesc>, iters: usize, warmup: usize, size: usize, server: bool) {
    let inject_size = info.get_tx_attr().get_inject_size();

    ft_sync(info, gl_ctx, tx_cq, rx_cq, tx_cntr, rx_cntr, ep, mr_desc);
    let mut now = Instant::now();
    if ! server {
        for i in 0..warmup+iters {
            if i == warmup {
                now = Instant::now();    // Start timer
            }

            if size < inject_size {
                ft_inject(info, gl_ctx, ep, gl_ctx.remote_address, size, tx_cq);
            }
            else {
                ft_tx(info, gl_ctx, ep, gl_ctx.remote_address, size, mr_desc, tx_cq, tx_cntr);
            }

            ft_rx(info, gl_ctx, ep, gl_ctx.remote_address, size, mr_desc, rx_cq, rx_cntr);
        }
    }
    else {
        for i in 0..warmup+iters {
            if i == warmup {
                // Start timer
            }

            ft_rx(info, gl_ctx, ep, gl_ctx.remote_address, size, mr_desc, rx_cq, rx_cntr);


            if size < inject_size {
                ft_inject(info, gl_ctx, ep, gl_ctx.remote_address, size, tx_cq);
            }
            else {
                ft_tx(info, gl_ctx, ep, gl_ctx.remote_address, size, mr_desc, tx_cq, tx_cntr);
            }
        }
    }
    if size == 1 {
        println!("{} {} {} {} {} {} {}", "bytes", "iters", "total", "time", "MB/sec", "usec/xfer", "Mxfers/sec");
    }
    // println!("Done");
    // Stop timer
    let elapsed = now.elapsed();
    let bytes = iters * size * 2;
    let usec_per_xfer = elapsed.as_micros() as f64 /iters as f64 / 2_f64;
    println!("{} {} {} {} s {} {} {}", size, iters, bytes, elapsed.as_secs(), bytes as f64 /elapsed.as_micros() as f64, usec_per_xfer, 1.0/usec_per_xfer as f64);
    // print perf data
}

pub fn bw_tx_comp(info: &InfoEntry, gl_ctx: &mut TestsGlobalCtx, ep: &libfabric::ep::Endpoint, tx_cq: &libfabric::cq::CompletionQueue, rx_cq: &libfabric::cq::CompletionQueue, tx_cntr: &Option<Counter>, rx_cntr: &Option<Counter>, mr_desc: &mut Option<libfabric::mr::MemoryRegionDesc>) {

    ft_get_tx_comp(gl_ctx, tx_cntr, tx_cq, gl_ctx.tx_seq);
    ft_rx(info, gl_ctx, ep, gl_ctx.remote_address, FT_RMA_SYNC_MSG_BYTES, mr_desc, rx_cq, rx_cntr);
}

pub fn bw_rma_comp(info: &InfoEntry, gl_ctx: &mut TestsGlobalCtx, op: &RmaOp, ep: &libfabric::ep::Endpoint, tx_cq: &libfabric::cq::CompletionQueue, rx_cq: &libfabric::cq::CompletionQueue, tx_cntr: &Option<Counter>, rx_cntr: &Option<Counter>, mr_desc: &mut Option<libfabric::mr::MemoryRegionDesc>, server: bool) {
    if matches!(op, RmaOp::RMA_WRITEDATA) {
        if ! server {
            bw_tx_comp(info, gl_ctx, ep, tx_cq, rx_cq, tx_cntr, rx_cntr, mr_desc);
        }
    }
    else {
        ft_get_tx_comp(gl_ctx, tx_cntr, tx_cq, gl_ctx.tx_seq);
    }
}

pub fn pingpong_rma(info: &InfoEntry, gl_ctx: &mut TestsGlobalCtx, tx_cq: &libfabric::cq::CompletionQueue, rx_cq: &libfabric::cq::CompletionQueue, tx_cntr: &Option<Counter>, rx_cntr: &Option<Counter>, ep: &libfabric::ep::Endpoint, mr_desc: &mut Option<libfabric::mr::MemoryRegionDesc>, op: RmaOp, remote: &libfabric::RmaIoVec, iters: usize, warmup: usize, size: usize, server: bool) {
    let inject_size = info.get_tx_attr().get_inject_size();

    ft_sync(info, gl_ctx, tx_cq, rx_cq, tx_cntr, rx_cntr, ep, mr_desc);
    let offest_rma_start = FT_RMA_SYNC_MSG_BYTES + std::cmp::max(ft_tx_prefix_size(info), ft_rx_prefix_size(info));


    let mut now = Instant::now();
    let mut j = 0;
    let mut offset = 0;
    for i in 0..warmup+iters {
        if i == warmup {
            now = Instant::now();    // Start timer
        }
        if j == 0 {
            offset = offest_rma_start;
        }

        if matches!(&op, RmaOp::RMA_WRITE) {

            if size < inject_size {
                ft_post_rma_inject(info, gl_ctx, &op, offset, size, remote, ep, gl_ctx.remote_address, tx_cq);
            }
            else {
                ft_post_rma(info, gl_ctx, &op, offset, size, remote, ep, gl_ctx.remote_address, mr_desc.as_mut().unwrap(), tx_cq);
            }
        }


        j+=1;

        if j == gl_ctx.window_size {
            bw_rma_comp(info, gl_ctx, &op, ep, tx_cq, rx_cq, tx_cntr, rx_cntr, mr_desc, server);
            j = 0;
        }

        offset += size;
    }

    bw_rma_comp(info, gl_ctx, &op, ep, tx_cq, rx_cq, tx_cntr, rx_cntr, mr_desc, server);
    

    if size == 1 {
        println!("{} {} {} {} {} {} {}", "bytes", "iters", "total", "time", "MB/sec", "usec/xfer", "Mxfers/sec");
    }
    // println!("Done");
    // Stop timer
    let elapsed = now.elapsed();
    let bytes = iters * size * 2;
    let usec_per_xfer = elapsed.as_micros() as f64 /iters as f64 / 2_f64;
    println!("{} {} {} {} s {} {} {}", size, iters, bytes, elapsed.as_secs(), bytes as f64 /elapsed.as_micros() as f64, usec_per_xfer, 1.0/usec_per_xfer as f64);
    // print perf data
}
