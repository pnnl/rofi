use debug_print::debug_println;
use libfabric::{av::{AddressVector, AddressVectorBuilder}, cntr::{Counter, CounterBuilder, ReadCntr, WaitCntr}, cq::{CompletionQueue, CompletionQueueBuilder, Completion, CompletionQueueImpl, ReadCq}, ep::{Endpoint, EndpointBuilder, ActiveEndpoint}, eq::{EventQueue, ReadEq}, error::Error, info::{InfoEntry, InfoCapsImpl}, mr::{MemoryRegionBuilder, MemoryRegionKey}};

pub(crate) fn init_ep_resources<I, EQ: ReadEq + 'static>(info: &InfoEntry<I>, domain: &libfabric::domain::Domain, eq: &libfabric::eq::EventQueue<EQ>) -> Result<(CompletionQueue<CompletionQueueImpl<true, false, false>>, CompletionQueue<CompletionQueueImpl<true, false, false>>, Counter<CntrOptDefault>, Counter<CntrOptDefault>, AddressVector, Endpoint<I>), libfabric::error::Error> {

    let tx_cntr = CounterBuilder::new().build(domain).unwrap();
    let rx_cntr = CounterBuilder::new().build(domain).unwrap(); // Default : FI_CNTR_EVENTS_COMP, FI_WAIT_UNSPEC

    let tx_cq =  CompletionQueueBuilder::new()
        .format(libfabric::enums::CqFormat::CONTEXT)
        .size(info.get_tx_attr().get_size())
        .build(domain)
        .unwrap();

    let rx_cq =  CompletionQueueBuilder::new()
        .format(libfabric::enums::CqFormat::CONTEXT)
        .size(info.get_rx_attr().get_size())
        .build(domain)
        .unwrap();

    let av = 
        match info.get_domain_attr().get_av_type() {
            libfabric::enums::AddressVectorType::Unspec => {
                AddressVectorBuilder::new()
                    .build(domain)
                    .unwrap()
            }
            _ =>  {
                AddressVectorBuilder::new()
                    .type_(info.get_domain_attr().get_av_type())
                    .count(10)
                    .build(domain)
                    .unwrap()
            }
    };

    let ep = EndpointBuilder::new(info).build(domain).unwrap();


    ep.bind_av(&av).unwrap();
    ep.bind_cntr().write().remote_write().cntr(&tx_cntr).unwrap();
    ep.bind_cntr().read().remote_read().cntr(&rx_cntr).unwrap();

    ep.bind_cq().transmit(true).cq(&tx_cq).unwrap();
    ep.bind_cq().recv(true).cq(&rx_cq).unwrap();

    ep.bind_eq(eq).unwrap();

    ep.enable().unwrap();

    Ok((tx_cq, rx_cq, tx_cntr, rx_cntr, av, ep))
}

pub(crate) fn progress(cq: &impl ReadCq, _total: u64, cq_cntr: &mut u64) {

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
                ret.unwrap();
            }
        }
    }
}

pub(crate) fn wait_on_cntr(pending_cntr: &mut u64, cntr: &impl WaitCntr) -> Result<(), libfabric::error::Error>
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

pub(crate) fn check_cntr(pending_cntr: &u64, cntr: &impl ReadCntr) -> bool {

    cntr.read() <= *pending_cntr
}

pub(crate) fn wait_on_context_comp(ctx: &libfabric::Context, cq: &impl ReadCq, cq_cntr: &mut u64) {

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

pub(crate) fn wait_on_event_join(eq: &impl ReadEq, tx_cq_cntr: &mut u64, rx_cq_cntr: &mut u64, tx_cq: &impl ReadCq, rx_cq: &impl ReadCq, ctx: &libfabric::Context) {

    // let mut eq_entry: EventQueueEntry<libfabric::Context> = libfabric::eq::EventQueueEntry::new();
    loop {

        let ret = eq.read();
        
        match ret {
            Ok(event) => {
                debug_println!("Found event");
                if let libfabric::eq::Event::JoinComplete(entry) = event {
                    debug_println!("Found join event {}", ctx as *const libfabric::Context as usize);
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

pub(crate) fn reg_mr<I>(info: &InfoEntry<I>, domain: &libfabric::domain::Domain, ep: &libfabric::ep::Endpoint<I>, buf: &mut [u8], key: u64) -> Result<(libfabric::mr::MemoryRegion, libfabric::mr::MemoryRegionDesc, MemoryRegionKey), Error> {

    if !need_mr_reg(info) {
        println!("MR not needed");
    }
    // let iov = libfabric::iovec::IoVec::from_slice(buf);
    // let mut mr_attr = libfabric::mr::MemoryRegionAttr::new().iov(std::slice::from_ref(&iov)).requested_key(key).iface(libfabric::enums::HmemIface::SYSTEM);

        // MemoryRegionBuilder::new(domain)
        //     .iov(std::slice::from_ref(&iov))
        //     .requested_key(key)
        //     .iface(libfabric::enums::HmemIface::SYSTEM)

    let mr = info_to_mr_attr(info, buf)
        // .iov(std::slice::from_ref(&iov))
        .requested_key(key)
        .iface(libfabric::enums::HmemIface::System)
        .build(domain)?;

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

fn need_mr_reg<I>(info: &InfoEntry<I>) -> bool {

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

fn check_mr_local_flag<I>(info: &InfoEntry<I>) -> bool {
    info.get_mode().is_local_mr() || info.get_domain_attr().get_mr_mode().is_local()
}

pub fn info_to_mr_attr<'a, I>(info: & InfoEntry<I>, buff: &'a [u8]) -> libfabric::mr::MemoryRegionBuilder<'a, u8>{

    let mut mr_region =  MemoryRegionBuilder::new(buff);

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
    ($post_fn:ident, $prog_fn:expr, $cq:expr, $seq:expr, $cq_cntr:expr, $op_str:literal, $ep:expr, $( $x:expr),* ) => {
        loop {
            let ret = $ep.$post_fn($($x,)*);
            if ret.is_ok() {
                break;
            }
            else if let Err(ref err) = ret {
                if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                    panic!("Unexepcted error in post_rma ");
                }

            }
            $prog_fn($cq, $seq, $cq_cntr);
        }
        $seq+=1;
    };
}

pub(crate) use post;

use crate::rofi::CntrOptDefault;
