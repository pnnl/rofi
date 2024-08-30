
use comm::{collective::CollectiveEp, rma::{ReadEp, WriteEp}};
use cq::ReadCq;
use libfabric::cntr::WaitCntr;
use ep::{ActiveEndpoint, BaseEndpoint};
use libfabric::*;
use pmi::pmi::Pmi;
use std::{collections::HashMap, rc::Rc, sync::{atomic::{AtomicBool, AtomicUsize, Ordering}}};
use parking_lot::RwLock;

use crate::{AllocInfo, AllocInfoManager, BarrierImpl, RemoteAllocInfo};

// #[derive(Debug)]
// enum BarrierImpl {
//     Uninit,
//     Collective(libfabric::comm::collective::MulticastGroupCollective),
//     Manual(usize, AtomicUsize),
// }

type WaitableEq = libfabric::eq_caps_type!(EqCaps::WAIT);
type WaitableCq = libfabric::cq_caps_type!(CqCaps::WAIT);
type WaitableCntr = libfabric::cntr_caps_type!(CntrCaps::WAIT);
type RmaAtomicCollEp = libfabric::info_caps_type!(FabInfoCaps::ATOMIC, FabInfoCaps::RMA, FabInfoCaps::COLL);

pub struct Ofi {
    pub(crate) num_pes: usize,
    pub(crate) my_pe: usize,
    mapped_addresses: Vec<libfabric::MappedAddress>,
    barrier_impl: BarrierImpl,
    ep: libfabric::ep::Endpoint<RmaAtomicCollEp>,
    cq: libfabric::cq::CompletionQueue<WaitableCq>,
    put_cntr: libfabric::cntr::Counter<WaitableCntr>,
    get_cntr: libfabric::cntr::Counter<WaitableCntr>,
    av: libfabric::av::AddressVector,
    eq: libfabric::eq::EventQueue<WaitableEq>,
    domain: libfabric::domain::Domain,
    _fabric: libfabric::fabric::Fabric,
    info_entry: libfabric::info::InfoEntry<RmaAtomicCollEp>,
    alloc_manager: AllocInfoManager,
    _my_pmi: pmi::pmi1::Pmi1,
    put_cnt: AtomicUsize,
    get_cnt: AtomicUsize,
}

impl Ofi {
    pub fn new(provider: Option<&str>, domain: Option<&str>) -> Result<Self, libfabric::error::Error> {
        let my_pmi = pmi::pmi1::Pmi1::new().unwrap();

        let info_caps = libfabric::infocapsoptions::InfoCaps::new().rma().atomic().collective();
        // let mut domain_conf = libfabric::domain::DomainAttr::new();
        // domain_conf.resource_mgmt= libfabric::enums::ResourceMgmt::Enabled;
        // domain_conf.threading = libfabric::enums::Threading::Domain;
        // domain_conf.mr_mode = libfabric::enums::MrMode::new().allocated().prov_key().virt_addr();
        // domain_conf.data_progress = libfabric::enums::Progress::Manual;
        
        // let mut endpoint_conf = libfabric::ep::EndpointAttr::new();
        //     endpoint_conf
        //     .ep_type(libfabric::enums::EndpointType::Rdm);
        
        // let info_hints = libfabric::info::InfoHints::new()
        //     .caps(info_caps)
        //     .domain_attr(domain_conf)
        //     .mode(libfabric::enums::Mode::new().context())
        //     .ep_attr(endpoint_conf);

        let info = libfabric::info::Info::new(&info::Version { major: 1, minor: 19 })
            .enter_hints()
                .caps(info_caps)
                .enter_domain_attr()
                    .resource_mgmt(libfabric::enums::ResourceMgmt::Enabled)
                    .threading(libfabric::enums::Threading::Domain)
                    .mr_mode(libfabric::enums::MrMode::new().allocated().prov_key().virt_addr())
                    .data_progress(libfabric::enums::Progress::Manual)
                .leave_domain_attr()
                .enter_ep_attr()
                    .type_(libfabric::enums::EndpointType::Rdm)
                .leave_ep_attr()
                .mode(libfabric::enums::Mode::new().context())
            .leave_hints()
            .get()?;

        let info_entry = info.into_iter()
            .find(|e| 
                if let Some(prov) = provider {
                    if let Some(dom) = domain {
                        e.fabric_attr().prov_name().split(';').any(|s| s == prov) && e.domain_attr().name().split(';').any(|s| s == dom)
                    }
                    else {
                        e.fabric_attr().prov_name().split(';').any(|s| s == prov)
                    }
                }
                else {
                    if let Some(dom) = domain {
                        e.domain_attr().name().split(';').any(|s| s == dom)
                    }
                    else {
                        eprintln!("Warning: No provider/domain requested");
                        true
                    }
                }
            ).expect(&format!("Error! No provider with name {:?} / domain {:?} was found", provider, domain));

        let fabric = libfabric::fabric::FabricBuilder::new().build(&info_entry)?;
        let eq = libfabric::eq::EventQueueBuilder::new(&fabric)
            .build()?;

        let domain = libfabric::domain::DomainBuilder::new(&fabric, &info_entry).build()?;
        let mut coll_attr = libfabric::comm::collective::CollectiveAttr::<()>::new();
        domain.query_collective::<()>(libfabric::enums::CollectiveOp::AllGather, &mut coll_attr)?;

    
        let cq = libfabric::cq::CompletionQueueBuilder::new()
            .format(libfabric::enums::CqFormat::Context)
            .size(info_entry.rx_attr().size())
            .build(&domain)?;

        let av = libfabric::av::AddressVectorBuilder::new()
            .build(&domain)?;

        let put_cntr = libfabric::cntr::CounterBuilder::new().build(&domain)?;
        let get_cntr = libfabric::cntr::CounterBuilder::new().build(&domain)?; //

        let ep = libfabric::ep::EndpointBuilder::new(&info_entry).build(&domain)?;
        ep.bind_av(&av)?;
        ep.bind_cntr()
            .write()
            .remote_write()
            .cntr(&put_cntr)?;
        
        ep.bind_cntr()
            .read()
            .remote_read()
            .cntr(&get_cntr)?;
        
        ep.bind_shared_cq(&cq, true)?;

        ep.bind_eq(&eq)?;

        ep.enable()?;

        let address = ep.getname()?;
        let address_bytes = address.as_bytes();

        my_pmi.put("epname", address_bytes).unwrap();
        my_pmi.exchange().unwrap();

        let unmapped_addresses: Vec<_> = my_pmi
            .ranks()
            .iter()
            .map(|r| {
                let addr = my_pmi.get("epname", &address_bytes.len(), &r).unwrap();
                unsafe{ep::Address::from_bytes(&addr)}
            })
            .collect();

        let mapped_addresses = av.insert(unmapped_addresses.as_slice().into(), libfabric::enums::AVOptions::new())?;
        let mapped_addresses: Vec<MappedAddress> = mapped_addresses.into_iter().map(|a| a.unwrap()).collect();
        let alloc_manager = AllocInfoManager::new();

        let mut ofi = Self {
            num_pes: my_pmi.ranks().len(),
            my_pe: my_pmi.rank(),
            _my_pmi: my_pmi,
            info_entry, 
            _fabric: fabric,
            domain,
            av,
            eq,
            put_cntr, 
            get_cntr,
            cq,
            ep,
            mapped_addresses,
            alloc_manager,
            barrier_impl : BarrierImpl::Uninit,
            put_cnt: AtomicUsize::new(0),
            get_cnt: AtomicUsize::new(0),
        };

        ofi.init_barrier()?;

        Ok(ofi)
    }

    fn create_mc_group(&self, pes: &[usize]) -> Result<libfabric::comm::collective::MulticastGroupCollective, libfabric::error::Error> {

        println!("Creating MC group");
        let mut av_set = libfabric::av::AddressVectorSetBuilder::new_from_range(&self.av, &self.mapped_addresses[pes[0]], &self.mapped_addresses[pes[0]], 1)
            .count(pes.len())
            .build()?;

        for pe in pes.iter().skip(1) {
            av_set.insert(&self.mapped_addresses[*pe])?;
        }

        let mut ctx = self.info_entry.allocate_context();
        let mc = libfabric::comm::collective::MulticastGroupCollective::new(&av_set);
        mc.join_collective_with_context(&self.ep, libfabric::enums::JoinOptions::new(), &mut ctx).unwrap();
        self.wait_for_join_event(&ctx)?;
        println!("Done Creating MC group");

        Ok(mc)
    }

    fn wait_for_join_event(&self, ctx: &Context) -> Result<(), libfabric::error::Error> {
        loop {
            let eq_res = self.eq.read();

            match eq_res {
                Ok(event) => {
                    if let libfabric::eq::Event::JoinComplete(entry) = event {
                        if entry.is_context_equal(ctx) {
                            return Ok(());
                        }
                    }
                },
                Err(err) => {
                    if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                        return Err(err);
                    }
                }
            }

            self.progress()?;
        }
    }

    pub(crate) fn progress(&self) -> Result<(), libfabric::error::Error> {

        let cq_res = self.cq.read(0);

        match cq_res {
            Ok(_) => {Ok(())},
            Err(err) => {
                if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                    Err(err)
                }
                else {
                    Ok(())
                }
            }
        }
    }

    fn wait_for_completion(&self, ctx: &Context) -> Result<(), libfabric::error::Error> {
        
        loop {
            let cq_res = self.cq.read(1);
            match cq_res {
                Ok(completion) => {
                    match completion {
                        libfabric::cq::Completion::Ctx(entries) | libfabric::cq::Completion::Unspec(entries) => {
                            if entries[0].is_op_context_equal(ctx) {
                                return Ok(());
                            }
                        },
                        libfabric::cq::Completion::Msg(entries) => {
                            if entries[0].is_op_context_equal(ctx) {
                                return Ok(());
                            }
                        },
                        libfabric::cq::Completion::Data(entries) => {
                            if entries[0].is_op_context_equal(ctx) {
                                return Ok(());
                            }
                        },
                        libfabric::cq::Completion::Tagged(entries) => {
                            if entries[0].is_op_context_equal(ctx) {
                                return Ok(());
                            }
                        },
                    }
                },
                Err(err) => {
                    if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                        return Err(err);
                    }
                }
            }
        }
    }

    pub(crate) fn wait_all_put(&self) -> Result<(), libfabric::error::Error> {

        let mut  cnt = self.put_cnt.load(Ordering::SeqCst);

        loop {
            let prev_cnt = cnt;
            self.put_cntr.wait(prev_cnt as u64, -1)?;
            cnt = self.put_cnt.load(Ordering::SeqCst);

            if prev_cnt >= cnt {
                break;
            }
        }

        Ok(())
    }

    pub fn wait_all_get(&self) -> Result<(), libfabric::error::Error> {

        let mut  cnt = self.get_cnt.load(Ordering::SeqCst);

        loop {
            let prev_cnt = cnt;
            self.get_cntr.wait(prev_cnt as u64, -1)?;
            cnt = self.get_cnt.load(Ordering::SeqCst);

            if prev_cnt >= cnt {
                break;
            }
        }

        Ok(())
    }

    fn wait_for_tx_cntr(&self, target: usize) -> Result<(), libfabric::error::Error> {

        self.put_cntr.wait(target as u64, -1)
    }

    fn wait_for_rx_cntr(&self, target: usize) -> Result<(), libfabric::error::Error> {

        self.get_cntr.wait(target as u64, -1)
    }

    fn exchange_mr_info(&self, addr: usize, len: usize, key: &libfabric::mr::MemoryRegionKey, pes: &[usize]) -> Result<Vec<libfabric::iovec::RmaIoVec>, libfabric::error::Error> {
        println!("Exchaning mr info");
        let mc = self.create_mc_group(pes)?;
        let key = match key {
            libfabric::mr::MemoryRegionKey::Key(key) => *key,
            libfabric::mr::MemoryRegionKey::RawKey(_) => panic!("Raw keys are not handled currently"),
        };

        println!("PE {} sending : {}", self.my_pe, addr); 
        let mut my_rma_iov = libfabric::iovec::RmaIoVec::new()
            .address(addr as u64)
            .len(len)
            .key(key);


        let my_iov_bytes =  unsafe{std::slice::from_raw_parts_mut((&mut my_rma_iov) as *mut libfabric::iovec::RmaIoVec as *mut u8, std::mem::size_of_val(&my_rma_iov))};
        let mut all_rma_iovs = vec![libfabric::iovec::RmaIoVec::new(); pes.len()];
        
        let mut ctx = self.info_entry.allocate_context();
        let all_iov_bytes = unsafe{std::slice::from_raw_parts_mut(all_rma_iovs.as_mut_ptr() as *mut u8, std::mem::size_of_val(&all_rma_iovs))};

        self.ep.allgather_with_context(my_iov_bytes, &mut libfabric::mr::default_desc(),  all_iov_bytes, &mut libfabric::mr::default_desc(), &mc, libfabric::enums::TferOptions::new(), &mut ctx)?;

        self.wait_for_completion(&ctx)?;
        println!("Recevied the following:");
        for rma_iov in all_rma_iovs.iter() {
            println!("{}", rma_iov.get_address());
        }
        println!("Done Exchaning mr info");

        Ok(all_rma_iovs)
    }


    fn post_put(&self, mut fun: impl FnMut() -> Result<(), libfabric::error::Error> ) -> Result<usize, libfabric::error::Error> {
        loop {
            match fun() {
                Ok(_) => break,
                Err(error) =>  {
                    if matches!(error.kind, libfabric::error::ErrorKind::TryAgain) {
                        self.progress()?;
                    }
                    else {
                        return Err(error);
                    }
                }
            }
        }

        Ok(self.put_cnt.fetch_add(1, Ordering::SeqCst) + 1)
    }

    fn post_get(&self, mut fun: impl FnMut() -> Result<(), libfabric::error::Error> ) -> Result<usize, libfabric::error::Error> {
        loop {
            match fun() {
                Ok(_) => break,
                Err(error) =>  {
                    if matches!(error.kind, libfabric::error::ErrorKind::TryAgain) {
                        self.progress()?;
                    }
                    else {
                        return Err(error);
                    }
                }
            }
        }

        Ok(self.get_cnt.fetch_add(1, Ordering::SeqCst) + 1)
    }


    pub fn init_barrier(&mut self) -> Result<(), libfabric::error::Error> {
        let mut coll_attr = libfabric::comm::collective::CollectiveAttr::<()>::new();

        if self.domain.query_collective::<()>(libfabric::enums::CollectiveOp::Barrier, &mut coll_attr).is_err() {
            println!("Using manual barrier");
            let all_pes: Vec<_> = (0..self.num_pes).collect();
            let barrier_size = all_pes.len() * std::mem::size_of::<usize>();
            let barrier_addr = self.sub_alloc(&all_pes, barrier_size)?;
            
            self.barrier_impl = BarrierImpl::Manual(barrier_addr, AtomicUsize::new(0));
            Ok(())
        }
        else {
            println!("Using libfab barrier");
            let all_pes: Vec<_> = (0..self.num_pes).collect();
            self.barrier_impl = BarrierImpl::Collective(
                self.create_mc_group(&all_pes)?
            );
            Ok(())
        }
    }

    pub fn sub_alloc(&self, pes: &[usize], size: usize) -> Result<usize, libfabric::error::Error> {
        // Align to page boundaries
        let aligned_size = 
        if (self.alloc_manager.page_size() - 1) & size != 0 { 
            (size + self.alloc_manager.page_size()) & !(self.alloc_manager.page_size()-1) 
        } 
        else {
            size
        }; 
        
        // Map memory of aligned size
        let mut mem = memmap::MmapOptions::new()
            .len(aligned_size)
            .map_anon()
            .expect("Error in allocating aligned memory");
    
        // Initialize mapped memory to zeros
        mem.iter_mut()
            .map(|x| *x = 0)
            .count();

        let mem_addr = mem.as_ptr() as usize;
        let mr = libfabric::mr::MemoryRegionBuilder::new(&mem, libfabric::enums::HmemIface::System)
            .requested_key(self.alloc_manager.next_key() as u64)
            .build(&self.domain)?;

        let mr = match mr {
            mr::MaybeDisabledMemoryRegion::Enabled(mr) => mr,
            mr::MaybeDisabledMemoryRegion::Disabled(mr) => {mr.bind_ep(&self.ep)?; mr.enable()?},
        };

        let rma_iovs =  self.exchange_mr_info(mem.as_ptr() as usize, mem.len(), &mr.key()?, &pes)?;

        let remote_alloc_infos = pes
            .iter()
            .zip(rma_iovs)
            .map(|(pe, rma_iov)| {
                let mapped_key = unsafe{libfabric::mr::MemoryRegionKey::from_u64(rma_iov.get_key())}
                    .into_mapped(&self.domain).unwrap();
                (*pe, RemoteAllocInfo::from_rma_iov(rma_iov, mapped_key))
            })
            .collect();

        
        self.alloc_manager.insert(AllocInfo::new(mem, mr, remote_alloc_infos)?);

        Ok(mem_addr)
    }

    pub fn sub_barrier(&self, pes: &[usize]) -> Result<(), libfabric::error::Error> {
        println!("Running barrier");
        match &self.barrier_impl {
            BarrierImpl::Uninit => {
                panic!("Barrier is not initialized");
            },
            BarrierImpl::Collective(mc) => {
                let mut ctx = self.info_entry.allocate_context();
                self.ep.barrier_with_context(mc, &mut ctx)?;
                self.wait_for_completion(&ctx)?;
                println!("Done with barrier");
                Ok(())
            },
            BarrierImpl::Manual(barrier_addr, barrier_id) => {
                let n = 2;
                let num_pes = pes.len();
                let num_rounds = ((num_pes as f64).log2() / (n as f64).log2()).ceil();
                let my_barrier = barrier_id.fetch_add(1, Ordering::SeqCst);

                for round in 0..num_rounds as usize {
                    for i in 1..=n {
                        let send_pe = euclid_rem(self.my_pe  as i64 + i  as i64 * (n as i64 + 1 ).pow(round as u32), num_pes as i64 );
                        
                        let dst = barrier_addr + 8 * self.my_pe;
                        unsafe { self.put(dst, std::slice::from_ref(&my_barrier), send_pe, false)? };
                    }
                
                    for i in 1..=n {
                        let recv_pe = euclid_rem(self.my_pe as i64 - i as i64 * (n  as i64 + 1).pow(round as u32), num_pes as i64);
                        let barrier_vec = unsafe {std::slice::from_raw_parts(barrier_addr as *const usize,  num_pes) };
                        
                        while my_barrier > barrier_vec[recv_pe] {
                            self.progress()?;
                            std::thread::yield_now();
                        }
                    } 
                }

                Ok(())
            }
        }
    }

    pub unsafe fn put<T>(&self, pe:usize, src_addr: &[T], dst_addr:usize, sync: bool) -> Result<(), libfabric::error::Error>{
        println!("Putting to PE {}, addr: {}", pe, dst_addr);
        let (offset, mut desc,  remote_alloc_info) = {
            let table = self.alloc_manager.mr_info_table.read();
            let alloc_info = table
                .iter()
                .find(|e| e.contains(&dst_addr))
                .expect("Invalid address");
            
            (alloc_info.start(), alloc_info.mr_desc(), alloc_info.remote_info(&pe)
                .expect(&format!("PE {} is not part of the sub allocation group", pe)))
        };

        let mut remote_dst_addr = dst_addr - offset + remote_alloc_info.start();
        let remote_key = remote_alloc_info.key();
        let cntr_order = if std::mem::size_of_val(src_addr) < self.info_entry.tx_attr().inject_size() {
            self.post_put( || {unsafe{self.ep.inject_write_to(src_addr, &self.mapped_addresses[pe], remote_dst_addr as u64, remote_key)}})?
        }
        else {
            let mut curr_idx = 0;

            let mut cntr_order = 0;
            while curr_idx < src_addr.len() {
                let msg_len = std::cmp::min(src_addr.len() - curr_idx, self.info_entry.ep_attr().max_msg_size()); 

                let order = self.post_put(|| {unsafe{self.ep.write_to(&src_addr[curr_idx..curr_idx+msg_len], &mut desc, &self.mapped_addresses[pe], remote_dst_addr as u64, remote_key)}})?;
              
                remote_dst_addr += msg_len;
                curr_idx += msg_len;
                cntr_order = order;
            }

            cntr_order
        };

        if sync {
            self.wait_for_tx_cntr(cntr_order)?;
        }        
        println!("Done putting");
        Ok(())
    } 

    pub unsafe fn get<T>(&self, pe:usize, src_addr:usize, dst_addr: &mut [T], sync: bool) -> Result<(), libfabric::error::Error> {
        println!("Getting from PE {}, addr: {}", pe, src_addr);
        let (offset, mut desc,  remote_alloc_info) = {
            let table = self.alloc_manager.mr_info_table.read();
            let alloc_info = table
                .iter()
                .find(|e| e.contains(&src_addr))
                .expect("Invalid address");
            
            (alloc_info.start(), alloc_info.mr_desc(), alloc_info.remote_info(&pe)
                .expect(&format!("PE {} is not part of the sub allocation group", pe)))
        };

        let mut remote_src_addr = src_addr - offset + remote_alloc_info.start();
        let remote_key = remote_alloc_info.key();

        let mut curr_idx = 0;

        let mut cntr_order = 0;
        while curr_idx < dst_addr.len() {
            let msg_len = std::cmp::min(dst_addr.len() - curr_idx,  self.info_entry.ep_attr().max_msg_size());
            let order = self.post_get(|| {unsafe{self.ep.read_from(&mut dst_addr[curr_idx..curr_idx+msg_len], &mut desc, &self.mapped_addresses[pe], remote_src_addr as u64, remote_key)}})?;
            remote_src_addr += msg_len;
            curr_idx += msg_len;
            cntr_order = order;
        }

        if sync {
            self.wait_for_rx_cntr(cntr_order)?;
        }

        println!("Done getting from PE {}, addr: {}", pe, src_addr);
        Ok(())
    }

    pub(crate) fn local_addr(&self, remote_pe: &usize, remote_addr: &usize) -> usize {
        self.alloc_manager
            .local_addr(remote_pe, remote_addr)
            .expect(&format!("Local address not found from remote PE {}, remote addr: {}", remote_pe, remote_addr))
    }

    pub(crate) fn remote_addr(&self, pe: &usize, local_addr: &usize) -> usize {
        self.alloc_manager
            .remote_addr(pe, local_addr)
            .expect(&format!("Remote address not found for PE {}", pe))
    }

    pub(crate) fn release(&self, addr: &usize) {
        self.alloc_manager.remove(addr);
    }



}

impl Drop for Ofi {
    fn drop(&mut self) {
       self.wait_all_get().unwrap();
       self.wait_all_put().unwrap();
    }
}


fn euclid_rem(a: i64, b: i64) -> usize {
    let r = a % b;

    if r>= 0 {r as usize} else {(r + b.abs()) as usize}
}#[cfg(test)]
mod tests {
    // use crate::rofi::RofiBuilder;

    use super::Ofi;

    #[test]
    fn init() {
        let mut rofi = Ofi::new(Some("verbs"), None).unwrap();
        // rofi.init_barrier().unwrap();
    }
    
    #[test]
    fn alloc() {
        let mut rofi = Ofi::new(Some("verbs"), None).unwrap();
        // rofi.init_barrier().unwrap();
        let pes: Vec<_> = (0..rofi.num_pes).collect();
        let _mem = rofi.sub_alloc( &pes, 256);
        // async_std::task::block_on(rofi.alloc(256).await)
    }
    
    #[test]
    fn sub_alloc() {
        let mut exclude_id = 1;

        const N: usize = 256;
        let mut rofi = Ofi::new(Some("verbs"), None).unwrap();
        // rofi.init_barrier().unwrap();
        let size = rofi.num_pes;
        assert!(size > 2);
        
        if rofi.my_pe != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter(|x| x != &exclude_id).collect();
            let pes_len = pes.len();
            println!("PES: {:?}", pes);
            let _mem = rofi.sub_alloc( &pes, N * pes_len);
        }
        exclude_id += 1;

        if rofi.my_pe != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter(|x| x != &exclude_id).collect();
            let pes_len = pes.len();
            println!("PES: {:?}", pes);
            let _mem = rofi.sub_alloc( &pes, N * pes_len);
        }
    }

    #[test]
    fn sub_put() {
        let exclude_id = 1;

        const N: usize = 256;
        let mut rofi = Ofi::new(Some("verbs"), None).unwrap();
        // rofi.init_barrier().unwrap();
        let size = rofi.num_pes;
        let my_id = rofi.my_pe;
        assert!(size > 2);
        
        if rofi.my_pe != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter(|x| x != &exclude_id).collect();
            let me = pes.iter().position(|x| x == &my_id).unwrap();

            let pes_len = pes.len();
            let send_id = pes[(me + 1) % pes_len];
            let other = if me as i64 - 1 < 0  {pes.len() as i64 - 1} else {me as i64 - 1} as usize;
            let mem = rofi.sub_alloc(&pes, N * pes_len).unwrap();
            let mem_slice = unsafe{std::slice::from_raw_parts_mut(mem as *mut u8, N * pes_len)};
            for i in 0..N {
               mem_slice[me* N + i] = (i % N) as u8;
               mem_slice[other* N + i] = 5;
            }
            let dst = mem + me  *  N;
            unsafe {rofi.put(send_id, &mem_slice[me*N..me*N+N], dst, false).unwrap()};
            rofi.wait_all_put().unwrap();
            rofi.sub_barrier(&pes).unwrap();
            // while mem_slice[other*N + N -1] == 5 {rofi.progress().unwrap();}
            assert_eq!( mem_slice[me * N..me * N + N], mem_slice[other * N.. other*N + N]);
        }
    }

    #[test]
    fn sub_get() {
        let exclude_id = 1;

        const N: usize = 256;
        let mut rofi = Ofi::new(Some("verbs"), None).unwrap();
        // rofi.init_barrier().unwrap();
        let size = rofi.num_pes;
        let my_id = rofi.my_pe;
        assert!(size > 2);
        
        if rofi.my_pe != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter(|x| x != &exclude_id).collect();
            let me = pes.iter().position(|x| x == &my_id).unwrap();

            let pes_len = pes.len();
            let recv_id = pes[(me + 1) % pes_len];
            let other = (me + 1) % pes_len;
            let mem = rofi.sub_alloc(&pes, N * pes_len).unwrap();
            let mem_slice = unsafe{std::slice::from_raw_parts_mut(mem as *mut u8, N * pes_len)};
            for i in 0..N {
                mem_slice[me* N + i] = (i % N) as u8;
                mem_slice[other* N + i] = 5;
            }
            let src = mem + other  *  N;
            unsafe {rofi.get(recv_id, src, &mut mem_slice[other*N..other*N+N], false).unwrap()};
            rofi.wait_all_get().unwrap();
            // while mem_slice[other*N + N -1] == 5 {rofi.progress().unwrap();}
            assert_eq!(&mem_slice[me * N..me * N + N],&mem_slice[other * N.. other*N + N]);
            rofi.sub_barrier(&pes).unwrap();
        }
    }

    #[test]
    fn sub_barrier() {
        let exclude_id = 1;
        let mut rofi = Ofi::new(Some("verbs"), None).unwrap();
        // rofi.init_barrier().unwrap();
        let size = rofi.num_pes;
        let my_id = rofi.my_pe;
        assert!(size > 2);

        if rofi.my_pe != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter(|x| x != &exclude_id).collect();
            rofi.sub_barrier(&pes).unwrap();
        }
    }

    #[test]
    fn put_inject() {
        const N : usize = 1 << 7;
        let mut rofi = Ofi::new(Some("verbs"), None).unwrap();
        // rofi.init_barrier().unwrap();
        let size = rofi.num_pes;
        let my_id = rofi.my_pe;
        assert!(size >= 2);
        let pes: Vec<_> = (0..rofi.num_pes).collect();
        let my_id = rofi.my_pe;
        let send_id = (my_id + 1) % size ;
        let recv_id =  if my_id as i64 - 1 < 0 {size as i64 -1 } else { my_id as i64 -1} as usize ;
        let pes_len = pes.len();
        let mem = rofi.sub_alloc( &pes, size * N).unwrap();
        let mem_slice = unsafe{std::slice::from_raw_parts_mut(mem as *mut u8, N * pes_len)};
        for i in 0..N {
            mem_slice[my_id* N + i] = (i % N) as u8;
        }
        
        rofi.sub_barrier(&pes).unwrap();
        let ptr =  my_id * N + mem;
        unsafe { rofi.put(send_id, &mem_slice[my_id * N..my_id* N + N ], ptr, false  ) }.unwrap();
        rofi.wait_all_put().unwrap(); 
        rofi.sub_barrier(&pes).unwrap();
        // while mem_slice[recv_id*N + N-1] != 127 {rofi.progress().unwrap();}
        assert_eq!(&mem_slice[my_id * N..my_id * N + N],&mem_slice[recv_id * N.. recv_id*N + N]);
    }


    #[test]
    fn put() {
        const N : usize = 1 << 8;
        let mut rofi = Ofi::new(Some("verbs"), None).unwrap();
        // rofi.init_barrier().unwrap();
        let size = rofi.num_pes;
        let my_id = rofi.my_pe;
        assert!(size >= 2);
        let pes: Vec<_> = (0..rofi.num_pes).collect();

        let send_id = (my_id + 1) % size ;
        let recv_id =  if my_id as i64 - 1 < 0 {size as i64 -1 } else { my_id as i64 -1} as usize ;
        
        let mem = rofi.sub_alloc(&pes, N * size).unwrap();
        let mem_slice = unsafe{std::slice::from_raw_parts_mut(mem as *mut u8, N * size)};

        for i in 0..N {
            mem_slice[my_id* N + i] = (i % N) as u8;
            mem_slice[recv_id* N + i] = 5;
        }

        rofi.sub_barrier(&pes).unwrap();

        let ptr =  my_id * N + mem;
        unsafe { rofi.put(send_id, &mem_slice[my_id * N..my_id* N + N ], ptr, false) }.unwrap();
        rofi.wait_all_put().unwrap();
        rofi.sub_barrier(&pes).unwrap();

        // rofi.barrier();
        // while mem_slice[recv_id*N + N - 1] == 5 {rofi.progress().unwrap();}
        assert_eq!(&mem_slice[my_id * N..my_id * N + N],&mem_slice[recv_id * N.. recv_id*N + N], "PE {}", my_id);
    }

    #[test]
    fn put_sync() {
        const N : usize = 1 << 8;
        let mut rofi = Ofi::new(Some("verbs"), None).unwrap();
        // rofi.init_barrier().unwrap();
        let size = rofi.num_pes;
        let my_id = rofi.my_pe;
        assert!(size >= 2);

        let send_id = (my_id + 1) % size ;
        let recv_id =  if my_id as i64 - 1 < 0 {size as i64 -1 } else { my_id as i64 -1} as usize ;
        let pes: Vec<_> = (0..rofi.num_pes).collect();

        let mem = rofi.sub_alloc(&pes, N * size).unwrap();
        let mem_slice = unsafe{std::slice::from_raw_parts_mut(mem as *mut u8, N * size)};

        for i in 0..N {
            mem_slice[my_id* N + i] = (i % N) as u8;
            mem_slice[recv_id* N + i] = 5;
        }

        rofi.sub_barrier(&pes).unwrap();

        let ptr =  my_id * N + mem;
        unsafe { rofi.put(send_id, &mem_slice[my_id * N..my_id* N + N ], ptr, true) }.unwrap();

        rofi.sub_barrier(&pes).unwrap();
        assert_eq!(&mem_slice[my_id * N..my_id * N + N],&mem_slice[recv_id * N.. recv_id*N + N], "PE {}", my_id);
    }

    #[test]
    fn get_sync() {
        const N : usize = 1 << 7;
        let mut rofi = Ofi::new(Some("verbs"), None).unwrap();
        // rofi.init_barrier().unwrap();
        let size = rofi.num_pes;
        let my_id = rofi.my_pe;
        assert!(size >= 2);
        let pes: Vec<_> = (0..rofi.num_pes).collect();

        let recv_id = (my_id + 1) % size ;

        let mem = rofi.sub_alloc(&pes, N * size).unwrap();
        let mem_slice = unsafe{std::slice::from_raw_parts_mut(mem as *mut u8, N * size)};

        for i in 0..N {
            mem_slice[my_id* N + i] = (i % N) as u8;
            mem_slice[recv_id* N + i] = 255;
        }

        rofi.sub_barrier(&pes).unwrap();

        let ptr =  recv_id*N + mem;
        unsafe { rofi.get(recv_id, ptr, &mut mem_slice[recv_id * N..recv_id* N + N ], true   ) }.unwrap();

        
        assert_eq!(&mem_slice[my_id * N..my_id * N + N],&mem_slice[recv_id * N.. recv_id*N + N]);
        rofi.sub_barrier(&pes).unwrap();
    }

    
    #[test]
    fn get() {
        const N : usize = 1 << 7;
        let mut rofi = Ofi::new(Some("verbs"), None).unwrap();
        // rofi.init_barrier().unwrap();
        let size = rofi.num_pes;
        let my_id = rofi.my_pe;
        assert!(size >= 2);
        let pes: Vec<_> = (0..rofi.num_pes).collect();

        let recv_id = (my_id + 1) % size ;

        let mem = rofi.sub_alloc(&pes, N * size).unwrap();
        let mem_slice = unsafe{std::slice::from_raw_parts_mut(mem as *mut u8, N * size)};

        for i in 0..N {
            mem_slice[my_id * N + i] = (i % N) as u8;
            mem_slice[recv_id * N + i] = 255;
        }
        rofi.sub_barrier(&pes).unwrap();
        
        let ptr =  recv_id*N + mem_slice.as_ptr() as usize;
        unsafe { rofi.get( recv_id, ptr, &mut mem_slice[recv_id * N..recv_id* N + N ], false ) }.unwrap();
        rofi.wait_all_get().unwrap();
        // while mem_slice[recv_id*N + N -1] == 255 {rofi.progress().unwrap();}
        assert_eq!(&mem_slice[my_id * N..my_id * N + N],&mem_slice[recv_id * N.. recv_id*N + N]);
        rofi.sub_barrier(&pes).unwrap();
    }


}