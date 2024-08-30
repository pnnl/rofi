use libfabric::{mr, CntrCaps};
use libfabric::async_::comm::collective::AsyncCollectiveEp;
use libfabric::async_::comm::rma::{AsyncWriteEp, AsyncReadEp};
use libfabric::cntr::WaitCntr;
use libfabric::error::ErrorKind::ErrorInQueue;
use libfabric::comm::collective::MulticastGroupCollective;
use libfabric::comm::rma::WriteEp;
use libfabric::domain::{Domain, DomainBuilder};
use pmi::pmi::Pmi;
use std::sync::atomic::{AtomicUsize, Ordering};
// use crate::mr::{MappedMemoryRegion, MemoryRegionManager, RmaInfo};
use debug_print::debug_println;
use libfabric::cq::ReadCq;
use libfabric::enums::{MrMode, AVOptions, JoinOptions, TferOptions};
use libfabric::ep::{self, ActiveEndpoint, BaseEndpoint};
use libfabric::fabric::{Fabric, FabricBuilder};
use libfabric::info;
// use libfabric::{av::{AddressVector, AddressVectorSetBuilder}, cntr::Counter, cq::{CompletionQueue, Completion, ReadCq}, domain::{Domain, DomainBuilder}, enums::{MrMode, AVOptions, JoinOptions, TferOptions}, ep::{Endpoint, EndpointAttr, self}, eq::{EventQueue, EventQueueBuilder, EventQueueImplT}, error::Error, fabric::{Fabric, FabricBuilder}, mr::{MemoryRegionDesc, MemoryRegionKey}, infocapsoptions::{InfoCaps, Caps, CollCap}, info::{InfoHints, Info, InfoEntry}, cntroptions::CntrConfig, MappedAddress, Waitable, async_::{cq::AsyncReadCq, eq::AsyncReadEq}};
// use libfabric::ep::Address;
use libfabric::{Context, FabInfoCaps, MappedAddress};
use async_std::task::block_on;
use crate::{AllocInfo, AllocInfoManager, BarrierImpl, RemoteAllocInfo};


pub type RmaAtomicCollEp = libfabric::info_caps_type!(FabInfoCaps::RMA, FabInfoCaps::COLL, FabInfoCaps::ATOMIC);
pub type EqOptDefault =  libfabric::async_eq_caps_type!();
pub type CqOptDefault =  libfabric::async_cq_caps_type!();
pub type CntrOptDefault = libfabric::cntr_caps_type!(CntrCaps::WAIT);

macro_rules!  post_async{
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

#[allow(dead_code)]
pub struct Ofi {
    pub(crate) num_pes: usize,
    pub(crate) my_pe: usize,
    mapped_addresses: Vec<libfabric::MappedAddress>,
    barrier_impl: BarrierImpl,
    ep: libfabric::async_::ep::Endpoint<RmaAtomicCollEp>,
    cq: libfabric::async_::cq::CompletionQueue<CqOptDefault>,
    put_cntr: libfabric::cntr::Counter<CntrOptDefault>,
    get_cntr: libfabric::cntr::Counter<CntrOptDefault>,
    av: libfabric::av::AddressVector,
    eq: libfabric::async_::eq::EventQueue<EqOptDefault>,
    domain: libfabric::domain::Domain,
    fabric: libfabric::fabric::Fabric,
    info_entry: libfabric::info::InfoEntry<RmaAtomicCollEp>,
    alloc_manager: AllocInfoManager,
    my_pmi: pmi::pmi1::Pmi1,
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
        let eq = libfabric::async_::eq::EventQueueBuilder::new(&fabric)
            .build()?;

        let domain = libfabric::domain::DomainBuilder::new(&fabric, &info_entry).build()?;
        let mut coll_attr = libfabric::comm::collective::CollectiveAttr::<()>::new();
        domain.query_collective::<()>(libfabric::enums::CollectiveOp::AllGather, &mut coll_attr)?;

    
        let cq = libfabric::async_::cq::CompletionQueueBuilder::new()
            .format(libfabric::enums::CqFormat::Context)
            .size(info_entry.rx_attr().size())
            .build(&domain)?;

        let av = libfabric::av::AddressVectorBuilder::new()
            .build(&domain)?;

        let put_cntr = libfabric::cntr::CounterBuilder::new().build(&domain)?;
        let get_cntr = libfabric::cntr::CounterBuilder::new().build(&domain)?; //

        let ep = libfabric::async_::ep::EndpointBuilder::new(&info_entry).build(&domain)?;
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
            my_pmi,
            info_entry, 
            fabric,
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

    pub fn init_barrier(&mut self) -> Result<(), libfabric::error::Error> {
        let mut coll_attr = libfabric::comm::collective::CollectiveAttr::<()>::new();

        if self.domain.query_collective::<()>(libfabric::enums::CollectiveOp::Barrier, &mut coll_attr).is_err() {
            println!("Using manual barrier");
            let all_pes: Vec<_> = (0..self.num_pes).collect();
            let barrier_size = all_pes.len() * std::mem::size_of::<usize>();
            let barrier_addr =  block_on(async {self.sub_alloc(&all_pes, barrier_size).await})?;
            
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

    pub async fn alloc(&mut self, size: usize) ->  Result<usize, libfabric::error::Error> {
        let pes: Vec<_> = (0..self.num_pes).collect();
        self.sub_alloc(&pes, size).await
    }
    

    pub async fn sub_alloc(&self, pes: &[usize], size: usize) -> Result<usize, libfabric::error::Error> {
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

        let rma_iovs =  self.exchange_mr_info(mem.as_ptr() as usize, mem.len(), &mr.key()?).await;

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

    /// Block the calling PE until all outstanding remote operations have completed.
    ///
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::RofiBuilder;
    ///
    /// let mut rofi = RofiBuilder::new().build();
    /// let mem = rofi.alloc(256);
    /// let dst = [0_u8; 256];
    /// unsafe { rofi.get(mem[0..256].as_ptr() as usize, &mut dst, 1).unwrap() };
    /// rofi.wait(); // make sure we got the data
    /// ```
    pub fn wait(&mut self)  {
        self.wait_all_put().unwrap();
        self.wait_all_get().unwrap();
    }

    pub fn local_addr(&self, remote_pe: &usize, remote_addr: &usize) -> usize {
        self.alloc_manager
            .local_addr(remote_pe, remote_addr)
            .expect(&format!("Local address not found from remote PE {}, remote addr: {}", remote_pe, remote_addr))
    }

    pub fn remote_addr(&self, pe: &usize, local_addr: &usize) -> usize {
        self.alloc_manager
            .remote_addr(pe, local_addr)
            .expect(&format!("Remote address not found for PE {}", pe))
    }


    // /// Flush all completion queue events from previous communication calls, ensuring progress.
    // /// 
    // /// # Examples
    // ///
    // /// ```
    // /// use rofi_rust::RofiBuilder;
    // ///
    // /// let mut rofi = RofiBuilder::new().build();
    // /// rofi.flush();
    // /// ```
    // pub fn flush(&mut self) {
    //     transport::progress(&self.tx.cq, self.tx.cq_seq, &mut self.tx.cq_cntr);
    //     transport::progress(&self.rx.cq, self.rx.cq_seq, &mut self.rx.cq_cntr);
    // }


    /// Block the calling PE until all processes in the job have entered the call as well.
    /// 
    /// # Collective Operation
    /// Requires all PEs in the job to enter the call 
    /// 
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::RofiBuilder;
    ///
    /// let mut rofi = RofiBuilder::new().build();
    /// rofi.barrier();
    /// ```
    pub async fn barrier(&mut self) {
        debug_println!("P[{}] Calling Barrier:", self.my_pe);
        let pes: Vec<_> = (0..self.num_pes).collect();
        self.sub_barrier(&pes).await;
        debug_println!("P[{}] End calling Barrier", self.my_pe);
    }

    /// Block the calling PE until all processes in the subset `pes` have entered the call as well.
    /// 
    /// # Collective Operation
    /// Requires all PEs in the subset to enter the call 
    /// 
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::RofiBuilder;
    ///
    /// let mut rofi = RofiBuilder::new().build();
    /// let pes: Vec<usize> = (0..3).collect();
    /// rofi.sub_barrier(&pes);
    /// ```
    pub async fn sub_barrier(&mut self, pes: &[usize]) {
        match &self.barrier_impl {
            BarrierImpl::Manual(barrier_addr, barrier_id) => {
                let n = 2_usize;
                let num_pes = pes.len();
                let num_rounds = ((num_pes as f64).log2() / (n as f64).log2()).ceil();
                let my_barrier = barrier_id.fetch_add(1, Ordering::SeqCst);
            

                for round in 0..num_rounds as usize {
                    
                    {
                        let id = self.my_pe as i64;
                        let mut futs = Vec::new();
                        for i in 1..=n {
                            let send_pe = euclid_rem(id + i as i64 * (n as i64 + 1).pow(round as u32), num_pes as i64 );
                            let send_pe = pes[send_pe];
                            let dst = barrier_addr + 8 * self.my_pe;
                            let src = std::slice::from_ref(&my_barrier);
                            // futs.insert(0, unsafe { self.put(dst,  src, send_pe)})
                            futs.push(unsafe { self.put(dst,  src, send_pe)})
                        }
                        debug_println!("Waiting for all puts");
                        futures::future::join_all(futs).await;
                    }


                    for i in 1..=n {
                        let recv_pe = euclid_rem(self.my_pe as i64 - i as i64 * (n  as i64 + 1).pow(round as u32), num_pes as i64);
                        let barrier_vec = unsafe {std::slice::from_raw_parts(barrier_addr as *const usize,  num_pes) };
                        
                        while my_barrier > barrier_vec[recv_pe] {
                            self.progress().unwrap();
                            std::thread::yield_now();
                        }
                    }  
                }
            }
            BarrierImpl::Uninit => panic!("Barrier is not initialized"),
            BarrierImpl::Collective(mc) => {
                // let mut ctx = self.info_entry.allocate_context();
                self.ep.barrier_async(mc).await.unwrap();
            },
        }
    }


    // pub unsafe fn put<T>(&self, pe:usize, src_addr: &[T], dst_addr:usize, sync: bool) -> Result<(), libfabric::error::Error>{
    //     println!("Putting to PE {}, addr: {}", pe, dst_addr);
    //     let (offset, mut desc,  remote_alloc_info) = {
    //         let table = self.alloc_manager.mr_info_table.read();
    //         let alloc_info = table
    //             .iter()
    //             .find(|e| e.contains(&dst_addr))
    //             .expect("Invalid address");
            
    //         (alloc_info.start(), alloc_info.mr_desc(), alloc_info.remote_info(&pe)
    //             .expect(&format!("PE {} is not part of the sub allocation group", pe)))
    //     };

    //     let mut remote_dst_addr = dst_addr - offset + remote_alloc_info.start();
    //     let remote_key = remote_alloc_info.key();
    //     let cntr_order = if std::mem::size_of_val(src_addr) < self.info_entry.tx_attr().inject_size() {
    //         self.post_put( || {unsafe{self.ep.inject_write_to(src_addr, &self.mapped_addresses[pe], remote_dst_addr as u64, remote_key)}})?
    //     }
    //     else {
    //         let mut curr_idx = 0;

    //         let mut cntr_order = 0;
    //         while curr_idx < src_addr.len() {
    //             let msg_len = std::cmp::min(src_addr.len() - curr_idx, self.info_entry.ep_attr().max_msg_size()); 

    //             let order = self.post_put(|| {unsafe{self.ep.write_to(&src_addr[curr_idx..curr_idx+msg_len], &mut desc, &self.mapped_addresses[pe], remote_dst_addr as u64, remote_key)}})?;
              
    //             remote_dst_addr += msg_len;
    //             curr_idx += msg_len;
    //             cntr_order = order;
    //         }

    //         cntr_order
    //     };

    //     if sync {
    //         self.wait_for_tx_cntr(cntr_order)?;
    //     }        
    //     println!("Done putting");
    //     Ok(())
    // } 

    pub async unsafe fn put<T>(&self, pe:usize, src_addr: &[T], dst_addr:usize) -> Result<(), libfabric::error::Error>{
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
                post_async!(write_to_async, self.ep, &src_addr[curr_idx..curr_idx+msg_len], &mut desc, &self.mapped_addresses[pe], remote_dst_addr as u64, remote_key);
                let order = self.put_cnt.fetch_add(1, Ordering::SeqCst) + 1;
              
                remote_dst_addr += msg_len;
                curr_idx += msg_len;
                cntr_order = order;
            }

            cntr_order
        };

        // if sync {
        self.wait_for_tx_cntr(cntr_order)?;
        // }        
        println!("Done putting");
        Ok(())
    } 


    fn wait_for_tx_cntr(&self, target: usize) -> Result<(), libfabric::error::Error> {

        self.put_cntr.wait(target as u64, -1)
    }

    fn wait_for_rx_cntr(&self, target: usize) -> Result<(), libfabric::error::Error> {

        self.get_cntr.wait(target as u64, -1)
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
    
    pub async unsafe fn get<T>(&self, pe:usize, src_addr:usize, dst_addr: &mut [T]) -> Result<(), libfabric::error::Error> {
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
            post_async!(read_from_async, self.ep, &mut dst_addr[curr_idx..curr_idx+msg_len], &mut desc, &self.mapped_addresses[pe], remote_src_addr as u64, remote_key);
            let order = self.get_cnt.fetch_add(1, Ordering::SeqCst) + 1;
            remote_src_addr += msg_len;
            curr_idx += msg_len;
            cntr_order = order;
        }

        // if sync {
        self.wait_for_rx_cntr(cntr_order)?;
        // }

        println!("Done getting from PE {}, addr: {}", pe, src_addr);
        Ok(())
    }

    // fn check_event(&mut self, event: &libfabric::enums::Event, ctx: &libfabric::Context) -> bool {

    //     let mut eq_entry: libfabric::eq::EventQueueEntry<libfabric::Context> = libfabric::eq::EventQueueEntry::new();

    //     let ret = self.eq.read();
    //     // std::slice::from_mut(&mut eq_entry));
        
    //     match ret {
    //         Ok((_, _ev)) => {
    //             if matches!(event, _ev) && eq_entry.is_context_equal(ctx) {
    //                     return true;
    //             }
    //         },
    //         Err(ref err) => {
    //             if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
    //                 ret.unwrap();
    //             }
    //         }
    //     }

    //     self.flush();
    //     // progress(self.tx.cq, 0, self.tx.cq_cntr);
    //     // progress(self.rx.cq, 0, rx_cq_cntr);

    //     false
    // }

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

    async fn sub_exchange_mr_info(&self, addr: usize, len: usize, key: &libfabric::mr::MemoryRegionKey, pes: &[usize]) -> Result<Vec<libfabric::iovec::RmaIoVec>, libfabric::error::Error> {
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
        
        let all_iov_bytes = unsafe{std::slice::from_raw_parts_mut(all_rma_iovs.as_mut_ptr() as *mut u8, std::mem::size_of_val(&all_rma_iovs))};

        self.ep.allgather_async(my_iov_bytes, &mut libfabric::mr::default_desc(),  all_iov_bytes, &mut libfabric::mr::default_desc(), &mc, libfabric::enums::TferOptions::new()).await?;

        println!("Recevied the following:");
        for rma_iov in all_rma_iovs.iter() {
            println!("{}", rma_iov.get_address());
        }
        println!("Done Exchaning mr info");

        Ok(all_rma_iovs)
    }

    async fn exchange_mr_info(&self, addr: usize, len: usize, key: &libfabric::mr::MemoryRegionKey) -> Vec<libfabric::iovec::RmaIoVec> {
        let pes: Vec<_> = (0..self.num_pes).collect();
        self.sub_exchange_mr_info(addr, len, key, &pes).await.unwrap()
    }
    
} 

fn euclid_rem(a: i64, b: i64) -> usize {
    let r = a % b;

    if r>= 0 {r as usize} else {(r + b.abs()) as usize}
}

#[cfg(test)]
mod tests {
    use super::Ofi;
    use async_std::task::block_on;

    macro_rules! define_test {
        ($func_name:ident, $async_fname:ident, $body: block) => {
            
            // #[cfg(feature= "use-async-std")]
            #[test]
            fn $func_name() {
                block_on(async {$async_fname().await});
            } 
            
            #[cfg(feature= "use-tokio")]
            #[tokio::test]
            #[ignore]
            async fn $func_name() {
                $async_fname().await;
            }
    
            async fn $async_fname() $body
        };
    }


    // #[tokio::test]
    // async fn init_async() {
    define_test!(init, init_async,  {
        let _rofi = Ofi::new(Some("verbs"), None).unwrap();
    });
    
    define_test!(alloc, alloc_async,  {
        let mut rofi =  Ofi::new(Some("verbs"), None).unwrap();
        rofi.alloc(256).await;
    });
    
    define_test!(sub_alloc, sub_alloc_async, {
        let exclude_id = 1;
        const N: usize = 256;
        let mut rofi =  Ofi::new(Some("verbs"), None).unwrap();
        let size = rofi.num_pes;
        assert!(size > 2);
        
        if rofi.my_pe != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter(|x|  x != &exclude_id ).collect();
            let pes_len = pes.len();
            let _mem = rofi.sub_alloc(&pes, N * pes_len).await;
        }
    });

    define_test!(sub_put, sub_put_async, {
        let exclude_id = 1;

        const N: usize = 256;
        let mut rofi =  Ofi::new(Some("verbs"), None).unwrap();
        let my_id = rofi.my_pe;
        let size = rofi.num_pes;
        assert!(size > 2);
        
        if rofi.my_pe != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter(|x| x != &exclude_id).collect();
            let me = pes.iter().position(|x| x == &my_id).unwrap();

            let pes_len = pes.len();
            let send_id = pes[(me + 1) % pes_len];
            let other = if me as i64 - 1 < 0  {pes.len() as i64 - 1} else {me as i64 - 1} as usize;
            let mem = rofi.sub_alloc(&pes, N * pes_len).await.unwrap();
            let mem_slice = unsafe{std::slice::from_raw_parts_mut(mem as *mut u8, N * pes_len)};

            for i in 0..N {
                mem_slice[me* N + i] = (i % N) as u8;
                mem_slice[other* N + i] = 5;
            }
            rofi.sub_barrier(&pes).await;
            let dst = mem + me  *  N;
            unsafe {rofi.put(send_id, &mem_slice[me*N..me*N+N], dst).await.unwrap()};
            rofi.sub_barrier(&pes).await; // Make sure everyone has finished putting
            assert_eq!(&mem_slice[me * N..me * N + N],&mem_slice[other * N.. other*N + N]);
        }
    });

    define_test!(sub_get, sub_get_async, {
        let exclude_id = 1;

        const N: usize = 256;
        let mut rofi =  Ofi::new(Some("verbs"), None).unwrap();
        let my_id = rofi.my_pe;
        let size = rofi.num_pes;
        assert!(size > 2);
        
        if rofi.my_pe != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter(|x| x != &exclude_id).collect();
            let me = pes.iter().position(|x| x == &my_id).unwrap();

            let pes_len = pes.len();
            let recv_id = pes[(me + 1) % pes_len];
            let other = (me + 1) % pes_len;
            let mem = rofi.sub_alloc(&pes, N * pes_len).await.unwrap();
            let mem_slice = unsafe{std::slice::from_raw_parts_mut(mem as *mut u8, N * pes_len)};

            for i in 0..N {
                mem_slice[me* N + i] = (i % N) as u8;
                mem_slice[other* N + i] = 5;
            }
            let src = mem + other  *  N;
            rofi.sub_barrier(&pes).await;
            unsafe {rofi.get(recv_id, src, &mut mem_slice[other*N..other*N+N],).await.unwrap()};
            
            assert_eq!(&mem_slice[me * N..me * N + N],&mem_slice[other * N.. other*N + N]);
            rofi.sub_barrier(&pes).await; // Make sure everyone has finished getting
        }
    });

    define_test!(sub_barrier, sub_barrier_async, {
        let exclude_id = 1;
        let mut rofi =  Ofi::new(Some("verbs"), None).unwrap();
        let size = rofi.num_pes;
        assert!(size > 2);

        if rofi.my_pe != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter(|x| x != &exclude_id).collect();
            rofi.sub_barrier(&pes).await;
        }
    });

    define_test!(put_inject, put_inject_async, {
        const N : usize = 1 << 7;
        let mut rofi =  Ofi::new(Some("verbs"), None).unwrap();
        let size = rofi.num_pes;
        assert!(size >= 2);

        let my_id = rofi.my_pe;
        let send_id = (my_id + 1) % size ;
        let recv_id =  if my_id as i64 - 1 < 0 {size as i64 -1 } else { my_id as i64 -1} as usize ;

        let mem = rofi.alloc( size * N).await.unwrap();
        let mem_slice = unsafe{std::slice::from_raw_parts_mut(mem as *mut u8, N * size)};

        for i in 0..N {
            mem_slice[my_id* N + i] = (i % N) as u8;
        }
        
        rofi.barrier().await;
        let ptr =  my_id * N + mem_slice.as_ptr() as usize;
        unsafe { rofi.put(send_id, &mem_slice[my_id * N..my_id* N + N ], ptr) }.await.unwrap();
        
        rofi.barrier().await;

        assert_eq!(&mem_slice[my_id * N..my_id * N + N],&mem_slice[recv_id * N.. recv_id*N + N]);
    });


    define_test!(put, put_async, {
        const N : usize = 1 << 20;
        let mut rofi =  Ofi::new(Some("verbs"), None).unwrap();
        let size = rofi.num_pes;
        assert!(size >= 2);

        let my_id = rofi.my_pe;
        let send_id = (my_id + 1) % size ;
        let recv_id =  if my_id as i64 - 1 < 0 {size as i64 -1 } else { my_id as i64 -1} as usize ;

        let mem = rofi.alloc(N * size).await.unwrap();
        let mem_slice = unsafe{std::slice::from_raw_parts_mut(mem as *mut u8, N * size)};


        for i in 0..N {
            mem_slice[my_id* N + i] = (i % N) as u8;
            mem_slice[recv_id* N + i] = 5;
        }

        rofi.barrier().await;

        let ptr =  my_id * N + mem_slice.as_ptr() as usize;
        {

            let target_mem = &mem_slice[my_id * N..my_id* N + N ];
            let put0 = unsafe { rofi.put(send_id, target_mem, ptr) };
            let put1 = unsafe { rofi.put(send_id, target_mem, ptr) };
            let put2 = unsafe { rofi.put(send_id, target_mem, ptr) };
            let (r0, r1, r2) = futures::join!(put1, put2, put0);
            r0.unwrap();
            r1.unwrap();
            r2.unwrap();
        }

        rofi.barrier().await; // Make sure everyone has finished putting


        assert_eq!(&mem_slice[my_id * N..my_id * N + N],&mem_slice[recv_id * N.. recv_id*N + N]);
    });
  
    define_test!(get, get_async, {
        const N : usize = 1 << 7;
            let mut rofi =  Ofi::new(Some("verbs"), None).unwrap();
            let size = rofi.num_pes;
            assert!(size >= 2);

            let my_id = rofi.my_pe;
            let recv_id = (my_id + 1) % size ;

            let mem = rofi.alloc(N* rofi.num_pes).await.unwrap();
            let mem_slice = unsafe{std::slice::from_raw_parts_mut(mem as *mut u8, N * rofi.num_pes)};
            for i in 0..N {
                mem_slice[my_id * N + i] = (i % N) as u8;
                mem_slice[recv_id * N + i] = 255;
            }
            rofi.barrier().await;
            
            let ptr =  recv_id*N + mem_slice.as_ptr() as usize;
            unsafe { rofi.get(recv_id, ptr, &mut mem_slice[recv_id * N..recv_id* N + N ]) }.await.unwrap();
            
            rofi.barrier().await; // Make sure all PEs have finished reading from remote
            assert_eq!(&mem_slice[my_id * N..my_id * N + N],&mem_slice[recv_id * N.. recv_id*N + N]);
    });


}