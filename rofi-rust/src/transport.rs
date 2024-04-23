use debug_print::debug_println;
use libfabric::{av::{AddressVector, AddressVectorBuilder}, cntr::{Counter, CounterBuilder}, cq::{CompletionQueue, CompletionQueueBuilder, Completion}, ep::{Endpoint, EndpointBuilder}, eq::EventQueue, error::Error, info::{InfoEntry, InfoCapsImpl}, cqoptions::{self, CqConfig}, cntroptions::{self, CntrConfig}, eqoptions::EqConfig, infocapsoptions::Caps, Waitable, mr::{MemoryRegionBuilder, MrKey}};

pub(crate) fn init_ep_resources<I: Caps, EQ: EqConfig + 'static>(info: &InfoEntry<I>, domain: &libfabric::domain::Domain, eq: &libfabric::eq::EventQueue<EQ>) -> Result<(CompletionQueue<cqoptions::Options<cqoptions::WaitNoRetrieve, cqoptions::Off>>, CompletionQueue<cqoptions::Options<cqoptions::WaitNoRetrieve, cqoptions::Off>>, Counter<cntroptions::Options<cntroptions::WaitNoRetrieve, cntroptions::Off>>, Counter<cntroptions::Options<cntroptions::WaitNoRetrieve, cntroptions::Off>>, AddressVector, Endpoint<I>), libfabric::error::Error> {

    let tx_cntr = CounterBuilder::new(domain).build().unwrap();
    let rx_cntr = CounterBuilder::new(domain).build().unwrap(); // Default : FI_CNTR_EVENTS_COMP, FI_WAIT_UNSPEC

    let tx_cq =  CompletionQueueBuilder::new(domain)
        .format(libfabric::enums::CqFormat::CONTEXT)
        .size(info.get_tx_attr().get_size())
        .build()
        .unwrap();

    let rx_cq =  CompletionQueueBuilder::new(&domain)
        .format(libfabric::enums::CqFormat::CONTEXT)
        .size(info.get_rx_attr().get_size())
        .build()
        .unwrap();

    let av = 
        match info.get_domain_attr().get_av_type() {
            libfabric::enums::AddressVectorType::Unspec => {
                AddressVectorBuilder::new(domain)
                    .build()
                    .unwrap()
            }
            _ =>  {
                AddressVectorBuilder::new(domain)
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

    ep.bind_cq().transmit(true).cq(&tx_cq).unwrap();
    ep.bind_cq().recv(true).cq(&rx_cq).unwrap();

    ep.bind_eq(eq).unwrap();

    ep.enable().unwrap();

    Ok((tx_cq, rx_cq, tx_cntr, rx_cntr, av, ep))
}

pub(crate) fn progress<CQ: CqConfig>(cq: &libfabric::cq::CompletionQueue<CQ>, _total: u64, cq_cntr: &mut u64) {

    let ret = cq.read(1);
    match ret {
        Ok(completion) => {
            match completion {
                Completion::Context(entries) => {
                    if entries.len() == 1 {
                        *cq_cntr += 1;
                    }
                }
                Completion::Message(entries) => {
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
                Completion::Unspec(num_read) => {
                    if num_read == 1 {
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

pub(crate) fn wait_on_context_comp<CQ: CqConfig>(ctx: &libfabric::Context, cq: &CompletionQueue<CQ>, cq_cntr: &mut u64) {

    loop {

        let ret = cq.read(1);
        // println!("Checking");
        match ret {
            Ok(completion) => {
                match completion {
                    Completion::Context(entry) => {
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

pub(crate) fn wait_on_event_join<EQ: EqConfig,CQ: CqConfig>(eq: &EventQueue<EQ>, tx_cq_cntr: &mut u64, rx_cq_cntr: &mut u64, tx_cq: &CompletionQueue<CQ>, rx_cq: &CompletionQueue<CQ>, ctx: &libfabric::Context) {

    // let mut eq_entry: EventQueueEntry<libfabric::Context> = libfabric::eq::EventQueueEntry::new();
    loop {

        let ret = eq.read();
        
        // debug_println!("Waiting for event");
        match ret {
            Ok(event) => {
                debug_println!("Found event");
                match event {
                    libfabric::eq::Event::JOIN_COMPLETE(entry) => {
                        debug_println!("Found join event");
                        if entry.is_context_equal(ctx) {
                            break;
                        }
                    }
                    _ => {}
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

pub(crate) fn reg_mr<I: Caps>(info: &InfoEntry<I>, domain: &libfabric::domain::Domain, ep: &libfabric::ep::Endpoint<I>, buf: &mut [u8], key: u64) -> Result<(libfabric::mr::MemoryRegion, libfabric::mr::MemoryRegionDesc, MrKey), Error> {

    if !need_mr_reg(info) {
        println!("MR not needed");
    }
    let iov = libfabric::iovec::IoVec::from_slice(buf);
    // let mut mr_attr = libfabric::mr::MemoryRegionAttr::new().iov(std::slice::from_ref(&iov)).requested_key(key).iface(libfabric::enums::HmemIface::SYSTEM);

        // MemoryRegionBuilder::new(domain)
        //     .iov(std::slice::from_ref(&iov))
        //     .requested_key(key)
        //     .iface(libfabric::enums::HmemIface::SYSTEM)

    let mr = info_to_mr_attr(info, domain)
        .iov(std::slice::from_ref(&iov))
        .requested_key(key)
        .iface(libfabric::enums::HmemIface::SYSTEM)
        .build()?;

    let desc = mr.description();

    if info.get_domain_attr().get_mr_mode().is_endpoint() {
        println!("MR ENDPOINT");
        mr.bind_ep(ep)?;
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

pub fn info_to_mr_attr<'a, I: Caps>(info: &'a InfoEntry<I>, domain: &'a libfabric::domain::Domain) -> libfabric::mr::MemoryRegionBuilder<'a>{

    let mut mr_region =  MemoryRegionBuilder::new(domain);

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
