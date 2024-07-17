use debug_print::debug_println;
use libfabric::{async_::{av::{AddressVector, AddressVectorBuilder}, cq::{CompletionQueue, CompletionQueueBuilder, AsyncCompletionQueueImpl}, ep::{Endpoint, EndpointBuilder}, domain::Domain, eq::{AsyncEventQueueImplT, AsyncEventQueueImpl}, mr::{MemoryRegionBuilder, MemoryRegion}}, infocapsoptions::Caps, eq::EventQueueImplT, info::{InfoEntry, InfoCapsImpl}, cq::{CompletionQueueImpl, CompletionQueueImplT, Completion}, cntr::{CounterBuilder, Counter}, cntroptions::{self, CntrConfig}, Waitable, mr::MemoryRegionKey, error::Error};

pub(crate) fn init_ep_resources<I: Caps, EQ: AsyncEventQueueImplT + 'static>(info: &InfoEntry<I>, domain: &Domain, eq: &libfabric::eq::EventQueue<EQ>) -> Result<(CompletionQueue<AsyncCompletionQueueImpl>, CompletionQueue<AsyncCompletionQueueImpl>, Counter<cntroptions::Options<cntroptions::WaitNoRetrieve, cntroptions::Off>>, Counter<cntroptions::Options<cntroptions::WaitNoRetrieve, cntroptions::Off>>, AddressVector, Endpoint<I>), libfabric::error::Error> {

    let tx_cntr = CounterBuilder::new(domain).build().unwrap();
    let rx_cntr = CounterBuilder::new(domain).build().unwrap(); // Default : FI_CNTR_EVENTS_COMP, FI_WAIT_UNSPEC

    println!("Creating completion queue");
    let tx_cq =  CompletionQueueBuilder::new(domain)
        .format(libfabric::enums::CqFormat::CONTEXT)
        .size(info.get_tx_attr().get_size())
        .build()
        .unwrap();

    println!("Creating completion queue");
    let rx_cq =  CompletionQueueBuilder::new(domain)
        .format(libfabric::enums::CqFormat::CONTEXT)
        .size(info.get_rx_attr().get_size())
        .build()
        .unwrap();

    let av = 
        match info.get_domain_attr().get_av_type() {
            libfabric::enums::AddressVectorType::Unspec => {
                AddressVectorBuilder::new(domain, eq)
                    .build()
                    .unwrap()
            }
            _ =>  {
                AddressVectorBuilder::new(domain, eq)
                    .type_(info.get_domain_attr().get_av_type())
                    .count(10)
                    .build()
                    .unwrap()
            }
    };

    let ep = EndpointBuilder::new(info).build(domain).unwrap();


    ep.bind_av(&av).unwrap();
    ep.bind_cntr().write().remote_write().cntr(&tx_cntr).unwrap();
    ep.bind_cntr().read().remote_read().cntr(&rx_cntr).unwrap();

    ep.bind_cq().transmit(false).cq(&tx_cq).unwrap();
    ep.bind_cq().recv(false).cq(&rx_cq).unwrap();

    ep.bind_eq(eq).unwrap();

    ep.enable().unwrap();

    Ok((tx_cq, rx_cq, tx_cntr, rx_cntr, av, ep))
}

pub(crate) fn progress(cq: &impl CompletionQueueImplT, _total: u64, cq_cntr: &mut u64) {

    let ret = cq.read(1);
    match ret {
        Ok(completion) => {
            match completion {
                Completion::Ctx(entries) | Completion::Unspec(entries) => {
                    if entries.len() == 1 {
                        *cq_cntr += 1;
                    }
                }
                Completion::Msg(entries) => {
                    if entries.len() == 1 {
                        *cq_cntr += 1;
                    }
                }
                Completion::Data(entries) => {
                    if entries.len() == 1 {
                        *cq_cntr += 1;
                    }
                }
                Completion::Tagged(entries) => {
                    if entries.len() == 1 {
                        *cq_cntr += 1;
                    }
                }
            }
        },
        Err(ref err) => {
            if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                match &err.kind {
                    libfabric::error::ErrorKind::ErrorAvailable => {
                        panic!("Completion error {}", cq.readerr(0).unwrap().error());
                    }
                    _=> {ret.unwrap();},
                }

            }
        }
    }
}

pub(crate) fn wait_on_cntr<CNTR>(pending_cntr: &mut u64, cntr: &Counter<CNTR>) -> Result<(), libfabric::error::Error>
        where CNTR: CntrConfig + Waitable
    {

    let mut cnt = *pending_cntr;
    let mut prev_cnt;

    loop {
        prev_cnt = cnt;
        cntr.wait(prev_cnt, -1)?;
        cnt = *pending_cntr;

        if prev_cnt >= cnt {
            break;
        }
    }

    Ok(())
}

pub(crate) fn check_cntr<CNTR: CntrConfig>(pending_cntr: &u64, cntr: &Counter<CNTR>) -> bool {

    cntr.read() <= *pending_cntr
}

pub(crate) fn wait_on_context_comp(ctx: &libfabric::Context, cq: &impl CompletionQueueImplT, cq_cntr: &mut u64) {

    loop {

        let ret = cq.read(1);
        // println!("Checking");
        match ret {
            Ok(completion) => {
                match completion {
                    Completion::Ctx(entry) => {
                        if entry.len() == 1 && entry[0].is_op_context_equal(ctx) { //[TODO! CRITICAL!]
                            *cq_cntr += 1; 
                            break;
                        }
                    }
                    _ => todo!()
                }
            },
            Err(ref err) => {
                if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                    ret.unwrap();
                }
            }
        }
    }
}

pub(crate) fn wait_on_event_join(eq: &impl EventQueueImplT, tx_cq_cntr: &mut u64, rx_cq_cntr: &mut u64, tx_cq: &impl CompletionQueueImplT, rx_cq: &impl CompletionQueueImplT, ctx: &libfabric::Context) {

    // let mut eq_entry: EventQueueEntry<libfabric::Context> = libfabric::eq::EventQueueEntry::new();
    loop {

        let ret = eq.read();
        
        // debug_println!("Waiting for event");
        match ret {
            Ok(event) => {
                debug_println!("Found event");
                if let libfabric::eq::Event::JoinComplete(entry) = event {
                    debug_println!("Found join event");
                    if entry.is_context_equal(ctx) {
                        break;
                    }
                }
            },
            Err(ref err) => {
                if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                    ret.unwrap();
                }
            }
        }

        progress(tx_cq, 0, tx_cq_cntr);
        progress(rx_cq, 0, rx_cq_cntr);
    } 
}

pub(crate) fn reg_mr<I: Caps>(info: &InfoEntry<I>, domain: &Domain, ep: &Endpoint<I>, buf: &mut [u8], key: u64) -> Result<(MemoryRegion, libfabric::mr::MemoryRegionDesc, MemoryRegionKey), Error> {

    if !need_mr_reg(info) {
        println!("MR not needed");
    }
    // let iov = libfabric::iovec::IoVec::from_slice(buf);
    // let mut mr_attr = libfabric::mr::MemoryRegionAttr::new().iov(std::slice::from_ref(&iov)).requested_key(key).iface(libfabric::enums::HmemIface::SYSTEM);

        // MemoryRegionBuilder::new(domain)
        //     .iov(std::slice::from_ref(&iov))
        //     .requested_key(key)
        //     .iface(libfabric::enums::HmemIface::SYSTEM)

    let mr = info_to_mr_attr(info, domain, &buf)
        // .iov(std::slice::from_ref(&iov))
        .requested_key(key)
        .iface(libfabric::enums::HmemIface::SYSTEM)
        .build()?;

    let desc = mr.description();
    
    // [TODO]
    if info.get_domain_attr().get_mr_mode().is_endpoint() {
        todo!()
    //     println!("MR ENDPOINT");
    //     mr.bind_ep(ep)?;
    }

    let key = mr.key()?;

    Ok((mr, desc, key))
}

fn need_mr_reg<I: Caps>(info: &InfoEntry<I>) -> bool {

    if info.get_caps().is_rma() || info.get_caps().is_atomic() {
        true
    }
    else {
        info.get_domain_attr().get_mr_mode().is_local()
    }
}

fn rma_write_target_allowed(caps: &InfoCapsImpl) -> bool {
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

fn rma_read_target_allowed(caps: &InfoCapsImpl) -> bool {
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

fn check_mr_local_flag<I: Caps>(info: &InfoEntry<I>) -> bool {
    info.get_mode().is_local_mr() || info.get_domain_attr().get_mr_mode().is_local()
}

pub fn info_to_mr_attr<'a, 'b, I: Caps>(info: &'a InfoEntry<I>, domain: &'a Domain, buff: &'b [u8]) -> MemoryRegionBuilder<'a, 'b, u8>{

    let mut mr_region =  MemoryRegionBuilder::new(domain, buff);

    if check_mr_local_flag(info) {
        if info.get_caps().is_msg() || info.get_caps().is_tagged() {
            let mut temp = info.get_caps().is_send();
            if temp {
                mr_region = mr_region.access_send();
            }
            temp |= info.get_caps().is_recv();
            if temp {
                mr_region = mr_region.access_recv();
            }
            if !temp {
                mr_region = mr_region.access_send().access_recv();
            }
        }
    }
    else if info.get_caps().is_rma() || info.get_caps().is_atomic() {
        if rma_read_target_allowed(info.get_caps()) {
            mr_region = mr_region.access_remote_read();
        }
        if rma_write_target_allowed(info.get_caps()) {
            mr_region = mr_region.access_remote_write();
        }
    }

    mr_region
}

macro_rules!  post{
    ($post_fn:ident, $ep:expr, $( $x:expr),* ) => {
        loop {
            let ret = $ep.$post_fn($($x,)*).await;
            if ret.is_ok() {
                break;
            }
            else if let Err(ref err) = ret {
                if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                    match &err.kind {
                        ErrorInQueue(e) => {
                            match e {
                                libfabric::error::QueueError::Completion(entry) => {
                                    panic!("Completion erro {}", entry.error())
                                }
                                _ => todo!()
                            }
                        },
                        _ => {ret.unwrap();},
                    }
                    panic!("Unexpected error in post_rma ");
                }

            }
        }
    };
}

pub(crate) use post;
