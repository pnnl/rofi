use libfabric::{av::AddressVector, cntr::Counter, cq::CompletionQueue, ep::{ActiveEndpoint, Endpoint}, eq::{EventQueue, EventQueueEntry}, error::Error};

pub(crate) fn init_ep_resources(info: &libfabric::InfoEntry, domain: &libfabric::domain::Domain, eq: &libfabric::eq::EventQueue) -> Result<(CompletionQueue, CompletionQueue, Counter, Counter, AddressVector, Endpoint), libfabric::error::Error> {

    let tx_cntr = domain.cntr_open(libfabric::cntr::CounterAttr::new()).unwrap(); // Default : FI_CNTR_EVENTS_COMP, FI_WAIT_UNSPEC
    let rx_cntr = domain.cntr_open(libfabric::cntr::CounterAttr::new()).unwrap(); // Default : FI_CNTR_EVENTS_COMP, FI_WAIT_UNSPEC


    let mut txcq_attr =  libfabric::cq::CompletionQueueAttr::new();
        txcq_attr
        .format(libfabric::enums::CqFormat::CONTEXT)
        .size(info.get_tx_attr().get_size());

    let mut rxcq_attr =  libfabric::cq::CompletionQueueAttr::new();
        rxcq_attr
        .format(libfabric::enums::CqFormat::CONTEXT)
        .size(info.get_rx_attr().get_size());

    let tx_cq = domain.cq_open(txcq_attr).unwrap();
    let rx_cq = domain.cq_open(rxcq_attr).unwrap();

    let av_attr = match info.get_domain_attr().get_av_type() {
        libfabric::enums::AddressVectorType::UNSPEC => libfabric::av::AddressVectorAttr::new(),
        _ => libfabric::av::AddressVectorAttr::new().type_(info.get_domain_attr().get_av_type()),
        }.count(10);

    let av = domain.av_open(av_attr).unwrap();
    let ep = domain.ep(&info).unwrap();



    ep.bind_av(&av).unwrap();
    ep.bind_cntr().write().remote_write().cntr(&tx_cntr).unwrap();
    ep.bind_cntr().read().remote_read().cntr(&rx_cntr).unwrap();

    ep.bind_cq().transmit(true).cq(&tx_cq).unwrap();
    ep.bind_cq().recv(true).cq(&rx_cq).unwrap();

    ep.bind_eq(eq).unwrap();

    ep.enable().unwrap();

    Ok((tx_cq, rx_cq, tx_cntr, rx_cntr, av, ep))
}

pub(crate) fn progress(cq: &libfabric::cq::CompletionQueue, _total: u64, cq_cntr: &mut u64) {

    let mut cq_err_entry = libfabric::cq::CqErrEntry::new();
    let ret = cq.read(std::slice::from_mut(&mut cq_err_entry), 1);
    match ret {
        Ok(size) => {if size == 1 {*cq_cntr += 1;}},
        Err(ref err) => {
            if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                ret.unwrap();
            }
        }
    }
}

pub(crate) fn wait_on_cntr(pending_cntr: &mut u64, cntr: &mut Counter) -> Result<(), libfabric::error::Error>{

    let mut cnt = *pending_cntr;
    let mut prev_cnt;

    loop {
        prev_cnt = cnt;
        cntr.wait(prev_cnt as u64, -1)?;
        cnt = *pending_cntr;

        if prev_cnt >= cnt {
            break;
        }
    }

    Ok(())
}

pub(crate) fn wait_on_context_comp(ctx: &libfabric::Context, cq: &CompletionQueue, cq_cntr: &mut u64) {

    let mut cq_err_entry = libfabric::cq::CqErrEntry::new();
    loop {

        let ret = cq.read(std::slice::from_mut(&mut cq_err_entry), 1);
        
        match ret {
            Ok(_) => {
                *cq_cntr += 1; 
            
                if cq_err_entry.is_op_context_equal(ctx) {
                    break;
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

pub(crate) fn wait_on_event(event: &libfabric::enums::Event, eq: &EventQueue, tx_cq_cntr: &mut u64, rx_cq_cntr: &mut u64, tx_cq: &CompletionQueue, rx_cq: &CompletionQueue, ctx: &libfabric::Context) {

    let mut eq_entry: EventQueueEntry<libfabric::Context> = libfabric::eq::EventQueueEntry::new();
    loop {

        let ret = eq.read(std::slice::from_mut(&mut eq_entry));
        
        match ret {
            Ok((_, _ev)) => {
                println!("Found something");
                if matches!(event, _ev) {
                    println!("Equal");
                    if eq_entry.is_context_equal(ctx) {
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

pub(crate) fn reg_mr(info: &libfabric::InfoEntry, domain: &libfabric::domain::Domain, ep: &libfabric::ep::Endpoint, buf: &mut [u8], key: u64) -> Result<(libfabric::mr::MemoryRegion, libfabric::mr::MemoryRegionDesc, u64), Error> {

    if !need_mr_reg(info) {
        println!("MR not needed");
    }
    let iov = libfabric::IoVec::new(buf);
    // let mut mr_attr = libfabric::mr::MemoryRegionAttr::new().iov(std::slice::from_ref(&iov)).requested_key(key).iface(libfabric::enums::HmemIface::SYSTEM);
    
    let mr_attr = info_to_mr_attr(info).iov(std::slice::from_ref(&iov)).requested_key(key).iface(libfabric::enums::HmemIface::SYSTEM);

    let mut mr = domain.mr_regattr(mr_attr, 0)?;
    let desc = mr.description();

    if info.get_domain_attr().get_mr_mode().is_endpoint() {
        println!("MR ENDPOINT");
        mr.bind_ep(ep)?;
    }

    let key = mr.get_key();

    Ok((mr, desc, key))
}

fn need_mr_reg(info: &libfabric::InfoEntry) -> bool {

    if info.get_caps().is_rma() || info.get_caps().is_atomic() {
        true
    }
    else {
        info.get_domain_attr().get_mr_mode().is_local()
    }
}

fn rma_write_target_allowed(caps: &libfabric::InfoCaps) -> bool {
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

fn rma_read_target_allowed(caps: &libfabric::InfoCaps) -> bool {
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

fn check_mr_local_flag(info: &libfabric::InfoEntry) -> bool {
    info.get_mode().is_local_mr() || info.get_domain_attr().get_mr_mode().is_local()
}

pub fn info_to_mr_attr(info: &libfabric::InfoEntry) -> libfabric::mr::MemoryRegionAttr {

    let mut mr_attr = libfabric::mr::MemoryRegionAttr::new();

    if check_mr_local_flag(info) {
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
            if rma_read_target_allowed(info.get_caps()) {
                mr_attr = mr_attr.access_remote_read();
            }
            if rma_write_target_allowed(info.get_caps()) {
                mr_attr = mr_attr.access_remote_write();
            }
        }
    }

    mr_attr
}

macro_rules!  post{
    ($post_fn:ident, $prog_fn:expr, $cq:expr, $seq:expr, $cq_cntr:expr, $op_str:literal, $ep:expr, $( $x:expr),* ) => {
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
            $prog_fn($cq, $seq, $cq_cntr);
        }
        $seq+=1;
    };
}

pub(crate) use post;
