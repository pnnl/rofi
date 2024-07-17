
use futures::future;
// mod async_rofi;
use libfabric::error::ErrorKind::ErrorInQueue;
use std::cell::RefCell;
use std::rc::Rc;
// use crate::mr::{MappedMemoryRegion, MemoryRegionManager, RmaInfo};
use debug_print::debug_println;
use libfabric::async_::av::AddressVector;
use libfabric::async_::cq::AsyncCompletionQueueImplT;
use libfabric::async_::domain::{Domain, DomainBuilder};
use libfabric::async_::ep::Endpoint;
use libfabric::async_::eq::{AsyncEventQueueImplT, EventQueue, EventQueueBuilder};
use libfabric::av::AddressVectorSetBuilder;
use libfabric::cq::Completion;
use libfabric::enums::{MrMode, AVOptions, JoinOptions, TferOptions};
use libfabric::ep::{EndpointAttr, self};
use libfabric::error::Error;
use libfabric::fabric::{Fabric, FabricBuilder};
use libfabric::info::{InfoEntry, Info, InfoHints};
use libfabric::mr::{MemoryRegionKey, MemoryRegionDesc};
use libfabric::{async_::cq::CompletionQueue, cntr::Counter, cntroptions::CntrConfig, cq::CompletionQueueImplT};
use crate::context::ContextBank;
// use libfabric::{av::{AddressVector, AddressVectorSetBuilder}, cntr::Counter, cq::{CompletionQueue, Completion, CompletionQueueImplT}, domain::{Domain, DomainBuilder}, enums::{MrMode, AVOptions, JoinOptions, TferOptions}, ep::{Endpoint, EndpointAttr, self}, eq::{EventQueue, EventQueueBuilder, EventQueueImplT}, error::Error, fabric::{Fabric, FabricBuilder}, mr::{MemoryRegionDesc, MemoryRegionKey}, infocapsoptions::{InfoCaps, Caps, CollCap}, info::{InfoHints, Info, InfoEntry}, cntroptions::CntrConfig, MappedAddress, Waitable, async_::{cq::AsyncCompletionQueueImplT, eq::AsyncEventQueueImplT}};
// use libfabric::ep::Address;
use libfabric::infocapsoptions::{RmaDefaultCap, Caps, InfoCaps, CollCap};
use libfabric::{RMA, COLL, ATOMIC, MappedAddress, Waitable};

use super::mr::{MemoryRegionManager, MappedMemoryRegion, RmaInfo};
use super::transport;
// Encapsulates data for the Tx/Rx operations
struct XxData<CQ: CompletionQueueImplT , CNTR: CntrConfig> {
    cq: CompletionQueue<CQ>,
    cntr: Counter<CNTR>,
    cq_cntr: u64,
    cq_seq: u64,
}


struct CommWorld {
    nnodes: usize,
    my_id : usize,
    addresses: Vec<MappedAddress>
}

pub enum RmaOp {
    RmaWrite,
    RmaWriteData,
    RmaRead,
}
pub type EpRmaAtomicCol = libfabric::caps_type!(RMA, COLL, ATOMIC);
pub type EqOptDefault =  libfabric::async_::eq::AsyncEventQueueImpl<false>;
pub type CqOptDefault =  libfabric::async_::cq::AsyncCompletionQueueImpl;
pub type CntrOptDefault = libfabric::cntroptions::Options<libfabric::cntroptions::WaitNoRetrieve, libfabric::cntroptions::Off>; // [TODO]

#[allow(dead_code)]
pub struct Rofi<I: Caps, EQ: AsyncEventQueueImplT, CQ: AsyncCompletionQueueImplT , CNTR: CntrConfig> {                       // Note that the order in which libfabric structs are defined matters 
                                        // e.g. fabric has to be dropped after domain, so we define it after
    world: CommWorld,
    pmi: Box<dyn crate::pmi::PmiTrait>,
    pub(crate) mr_manager: Rc<RefCell<MemoryRegionManager>>,
    barrier_mr: Rc<MappedMemoryRegion>,
    ep: Endpoint<I>,
    eq: EventQueue<EQ>,
    tx: XxData<CQ, CNTR>,
    rx: XxData<CQ, CNTR>,
    av: AddressVector,
    domain: Domain,
    fabric: Fabric,
    pub(crate) info: InfoEntry<I>,
    all_info: Info<I>,
    mr_next_key: u64,
    ctx_bank: RefCell<ContextBank>,
    barrier_id: usize,
    // transport_mtx: std::sync::Mutex<()>,
}


pub struct RofiBuilder {
    pmi: Box<dyn crate::pmi::PmiTrait>,
}

impl RofiBuilder {
    /// Request to build rofi using PMI1 if available
    /// 
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::RofiBuilder;
    ///
    /// let rofi = RofiBuilder::with_pmi1();
    /// ```
    #[cfg(feature = "with-pmi1")]    
    pub fn with_pmi1() -> Self {
        Self {
            pmi: Box::new(crate::pmi::Pmi1::init().unwrap()),
        }
    }
    
    /// Request to build rofi using PMI2 if available
    /// 
    ///  # Examples
    /// 
    /// ```
    /// use rofi_rust::RofiBuilder;
    /// 
    /// let rofi = RofiBuilder::with_pmi2();
    /// ```
    #[cfg(feature = "with-pmi2")]    
    pub fn with_pmi2() -> Self {
        Self {
            pmi: Box::new(pmi::Pmi2::init().unwrap()),
        }
    }

    /// Request to build rofi without any prefered PMI implementation.
    /// 
    /// Depending on the features enabled (i.e., "with-pmi1", "with-pmi2") rofi will
    /// try to build with PMI2 first and will fallback to PMI1 if PMI2 is not enabled
    /// 
    /// 
    ///
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::RofiBuilder;
    ///
    /// let rofi = RofiBuilder::new();
    /// ```
    #[cfg(any(feature = "with-pmi1", feature = "with-pmi2"))]
    pub fn new() -> Self {
        #[cfg(not(feature = "with-pmi2"))]
        return Self {
            pmi: Box::new(crate::pmi::Pmi1::init().unwrap()),
        };

        #[cfg(feature = "with-pmi2")]
        Self {
            pmi: Box::new(pmi::Pmi2::init().unwrap()),
        }
    }


    /// Instatiate a Rofi object
    /// 
    /// # Collective Operation
    /// Requires all PEs in the job to enter the call
    ///
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::RofiBuilder;
    ///
    /// let rofi_builder = RofiBuilder::with_pmi1.build()
    /// ```
    pub async fn build(mut self) -> Result<Rofi<EpRmaAtomicCol, EqOptDefault, CqOptDefault, CntrOptDefault>, libfabric::error::Error> {

        let caps = InfoCaps::new().rma().atomic().collective();
        let hints = InfoHints::new()
            .caps(caps)
            .domain_attr(
                libfabric::domain::DomainAttr::new()
                    .resource_mgmt(libfabric::enums::ResourceMgmt::Enabled)
                    .threading(libfabric::enums::Threading::Domain)
                    .mr_mode(MrMode::new().allocated().prov_key().virt_addr()).clone()
                    // .data_progress(libfabric::enums::Progress::Auto).clone()
                    .control_progress(libfabric::enums::Progress::Manual).clone()
                )
            .mode(libfabric::enums::Mode::new().context())
            .ep_attr(EndpointAttr::new().ep_type(libfabric::enums::EndpointType::Rdm).clone());
        

    
        let all_info = Info::new().hints(&hints).request()?;
        let entries = all_info.get();
        let info = entries[0].clone();
        let fabric = FabricBuilder::new(&info).build()?;
        // println!("Control Progress {} ",info.get_domain_attr().c_attr.control_progress);
        // println!("Data Progress {}" ,info.get_domain_attr().c_attr.data_progress);
            
        let eq = EventQueueBuilder::new(&fabric).build()?;
        let domain = DomainBuilder::new(&fabric, &info).build()?;
        // libfabric::comm::collective::CollectiveAttr::new();
        let mut attr = libfabric::comm::collective::CollectiveAttr::<()>::new();
        domain.query_collective::<()>(libfabric::enums::CollectiveOp::AllGather, &mut attr)?;
        let (tx_cq, rx_cq, tx_cntr, rx_cntr, av, ep) = transport::init_ep_resources(&info, &domain, &eq).unwrap();

        // let mut addresses: Vec<Address> =  vec![u64::MAX; pmi.get_size()];
        // let mut addr  = vec![0_u8; 16];

        ep.getname()?;
        let address = ep.getname().unwrap();
        let address_bytes = address.as_bytes();

        self.pmi.put("epname", address_bytes).unwrap();
        self.pmi.exchange().unwrap();
        let mut all_addresses = Vec::new();
        for i in 0..self.pmi.get_size() {
            let res = self.pmi.get(i, "epname", address_bytes.len()).unwrap();
            let address = unsafe{ep::Address::from_bytes(&res)};
            all_addresses.push(address);
        }

        let (_, addresses) = av.insert_async(&all_addresses, AVOptions::new()).await.unwrap();

        let mr_manager = Rc::new(RefCell::new(MemoryRegionManager::new()));
        let barrier_size = self.pmi.get_size() * std::mem::size_of::<usize>();
        let barrier_mr = mr_manager.borrow_mut().alloc(&info, &domain, &ep, barrier_size);
        let mut rofi = Rofi {
            world: CommWorld{ nnodes: self.pmi.get_size(), my_id: self.pmi.get_rank(), addresses},
            pmi: self.pmi,
            all_info,
            info,
            fabric,
            domain,
            eq,
            ep,
            tx: XxData{ cq: tx_cq, cntr: tx_cntr, cq_cntr: 0, cq_seq: 0},
            rx: XxData{ cq: rx_cq, cntr: rx_cntr, cq_cntr: 0, cq_seq: 0}, // Rx buffer starts just after the Tx buffer
            av,
            mr_next_key: 0,
            ctx_bank: RefCell::new(ContextBank::new()),
            barrier_mr, 
            barrier_id : 0,
            mr_manager,
            // transport_mtx: std::sync::Mutex::new(()),
        };

        let key = rofi.barrier_mr.get_key();
        let mr = rofi.barrier_mr.get_mem().borrow().as_ptr() as u64;
        println!("About to exchange mr info");
        let remote_iovs = match key {
            MemoryRegionKey::Key(key) => {
            rofi.exchange_mr_info(mr, *key).await
                }
                _ => todo!()
        };
        let remote_infos: Vec<RmaInfo> = remote_iovs.iter().map(|iov| {RmaInfo::new(iov.get_address(), iov.get_len(), &Rc::new(unsafe{MemoryRegionKey::from_u64(iov.get_key())}.into_mapped(&rofi.domain).unwrap()) )}).collect();
        rofi.barrier_mr.set_rma_infos(remote_infos);

        Ok(rofi)
        // Rofi::init(self.pmi, hints, eq, tx_cq, rx_cq, tx_cntr, rx_cntr)
    }
}

// impl  Rofi<(),(),(),()> {

//     pub(crate) fn init<I, EQ, CQ, CNTR>(mut pmi: Box<dyn crate::pmi::PmiTrait>, all_info: Info<I>, info: InfoEntry<I>,
//     eq: EventQueue<EQ>, tx_cq: CompletionQueue<CQ>, rx_cq: CompletionQueue<CQ>, tx_cntr: Counter<CNTR>, rx_cntr: Counter<CNTR>) -> Result<Rofi<T, EQ, CQ, CNTR>, libfabric::error::Error> 
//                 where I : Caps + CollCap + RmaDefaultCap, EQ: EqConfig + Waitable, CQ: CqConfig + Waitable, CNTR: CntrConfig + Waitable {

        


//         let key = rofi.barrier_mr.get_key();
//         let mr = rofi.barrier_mr.get_mem().borrow().as_ptr() as u64;
        
//         let remote_iovs = rofi.exchange_mr_info(mr, key);
//         rofi.barrier_mr.set_iovs(remote_iovs);

//         Ok(rofi)
//     }
// }
impl<I, EQ, CQ, CNTR> Rofi<I, EQ, CQ, CNTR>  
    where I : Caps + CollCap + RmaDefaultCap, EQ: AsyncEventQueueImplT, CQ:  AsyncCompletionQueueImplT, CNTR: CntrConfig + Waitable {

    /// Returns the number of processes that take part into this job
    /// 
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::RofiBuilder;
    ///
    /// let rofi = RofiBuilder::new().build();
    /// let num_pes = rofi.get_size();
    /// ```
    pub fn get_size(&self) -> usize {
        self.world.nnodes
    }

    /// Returns the id of the current processes in this job
    /// 
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::RofiBuilder;
    ///
    /// let rofi = RofiBuilder::new().build();
    /// let my_pe = rofi.get_id();
    /// ```
    pub fn get_id(&self) -> usize {
        self.world.my_id
    }

    /// Allocates a memory region of size `size` and registers it to be accessible remotely
    /// from other compute nodes. 
    /// 
    /// # Collective Operation
    /// Requires all PEs in the job to enter the call
    /// 
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::RofiBuilder;
    ///
    /// let rofi = RofiBuilder::new().build();
    /// let mem = rofi.alloc(256);
    /// ```
    pub async fn alloc(&mut self, size: usize) ->  Rc<MappedMemoryRegion> {

        let mem = self.mr_manager.borrow_mut().alloc(&self.info, &self.domain, &self.ep, size);
        let addr = mem.get_mem().borrow().as_ptr() as u64;
        
        let remote_iovs = 
            match mem.get_key() {
                MemoryRegionKey::Key(key) => {
                    self.exchange_mr_info(addr, *key).await
                }
                _ => todo!()
            };

        let remote_infos: Vec<RmaInfo> = remote_iovs.iter().map(|iov| {RmaInfo::new(iov.get_address(), iov.get_len(), &Rc::new(unsafe{MemoryRegionKey::from_u64(iov.get_key())}.into_mapped(&self.domain).unwrap()) )}).collect();

        mem.set_rma_infos(remote_infos);
        
        mem.clone()
    }
    
    /// Allocates a memory region of size `size` and registers it to be accessible remotely
    /// from other compute nodes in the subset. The calling PE *must* be in the subset
    /// 
    /// # Collective Operations
    /// Requires all PEs in the subset to enter the call  
    ///
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::RofiBuilder;
    ///
    /// let mut rofi = RofiBuilder::new().build();
    /// let pes: Vec<usize> = (0..3).collect();
    /// let result = rofi.sub_alloc(256, &pes);
    /// ```
    pub async fn sub_alloc(&mut self, size: usize, pes: &[usize]) ->  Rc<MappedMemoryRegion> {
        
        let mem = self.mr_manager.borrow_mut().alloc(&self.info, &self.domain, &self.ep, size);
        let mem_key = 
            match mem.get_key() {
                MemoryRegionKey::Key(key) => {
                    key
                }
                _ => todo!()
            };
        let addr = mem.get_mem().borrow().as_ptr() as u64;
        let remote_iovs = self.sub_exchange_mr_info(addr, *mem_key, pes).await;
        let rma_infos : Vec<RmaInfo> = remote_iovs.iter().map(|iov| {RmaInfo::new(iov.get_address(), iov.get_len(), &Rc::new(unsafe{MemoryRegionKey::from_u64(iov.get_key())}.into_mapped(&self.domain).unwrap())) }).collect();
        mem.set_sub_rma_infos(rma_infos, pes);
    
        mem.clone()
    }

    /// Initiate a transfer of data in `src` to the memory address `dst` at PE `id`and return immediately
    /// 
    /// # Safety
    /// This function is unsafe as the destination memory address might be mutated by other PEs at the same time
    /// 
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::RofiBuilder;
    ///
    /// let mut rofi = RofiBuilder::new().build();
    /// let mem = rofi.alloc(256);
    /// let src = [0_u8; 256];
    /// 
    /// unsafe { rofi.iput(&mem[128..].as_ptr() as usize, &src, 1).unwrap() };
    /// ```
    pub async unsafe fn put(&self, dst: usize, src: &[u8], id: usize) -> Result<(), std::io::Error> {

        self.put_(dst, src, id, true).await
    }

    /// Initiate a transfer from data in memory address `src` at PE `id` to buffer slice `dst` and return immediately.
    /// 
    /// # Safety
    /// This function is unsafe as the src memory address might be mutated by other PEs at the same time 
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
    /// ```
    pub async unsafe fn get(&mut self, src: usize, dst: &mut[u8], id: usize) -> Result<(), std::io::Error> {

        self.get_(src, dst, id, false).await
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
        self.wait_get_all().unwrap();
        self.wait_put_all().unwrap();
    }

    /// Compute the virtual address corresponding to `local_addr` on PE `pe`.
    /// 
    /// When allocating a symmetric memory region, ROFI does not require that the virutal
    /// addresses be aligned. In a sense, the virtual addresses are not symmetric, only the
    /// offsets are. This function maps a certain address `local_addr` on the current node to the
    /// corresponding virtual address on the remote PE `pe`.
    /// 
    /// 
    ///
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::Rofi;
    ///
    /// use rofi_rust::RofiBuilder;
    ///
    /// let mut rofi = RofiBuilder::new().build();
    /// let mem = rofi.alloc(256);
    /// let remote_addr = rofi.get_remote_address(&mem[128..].as_ptr() as usize, 2);
    /// ```
    pub fn get_remote_address(&self, local_addr: usize, pe: usize) -> usize {
        let remote_offset =  self.mr_manager.borrow().mr_get(local_addr).expect("Local address not found").get_remote_start(pe);
        let local_offset =  self.mr_manager.borrow().mr_get(local_addr).expect("Local address not found").get_start();

        (local_addr - local_offset) + remote_offset
    }

    /// Compute the local virtual address corresponding to `remote_addr` on PE `pe`.
    ///
    /// When allocating a symmetric memory region, ROFI does not require that the virutal
    /// addresses be aligned. In a sense, the virtual addresses are not symmetric, only the
    /// offsets are. This function maps a certain address `remote_addr` on the remote PE `pe` to the
    /// corresponding virtual address on the calling PE.
    /// 
    ///
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::Rofi;
    ///
    /// use rofi_rust::RofiBuilder;
    ///
    /// let mut rofi = RofiBuilder::new().build();
    /// let remote_addr  = 0xFFF; // Must be part of a memory region in PE 2
    /// let local_addr = rofi.get_local_from_remote_address(remote_addr, 2);
    /// ```
    pub fn get_local_from_remote_address(&self, remote_addr: usize, pe: usize) -> usize {
        let local_offset =  self.mr_manager.borrow().mr_get_from_remote(remote_addr, pe).expect("Remote address not found").get_start();
        let remote_offset =  self.mr_manager.borrow().mr_get_from_remote(remote_addr, pe).expect("Remote address not found").get_remote_start(pe);

        (remote_addr - remote_offset) + local_offset
    }

    /// Flush all completion queue events from previous communication calls, ensuring progress.
    /// 
    /// # Examples
    ///
    /// ```
    /// use rofi_rust::RofiBuilder;
    ///
    /// let mut rofi = RofiBuilder::new().build();
    /// rofi.flush();
    /// ```
    pub fn flush(&mut self) {
        transport::progress(&self.tx.cq, self.tx.cq_seq, &mut self.tx.cq_cntr);
        transport::progress(&self.rx.cq, self.rx.cq_seq, &mut self.rx.cq_cntr);
    }


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
        debug_println!("P[{}] Calling Barrier:", self.world.my_id);
        let n = 2;
        let num_pes = self.world.nnodes ;
        let num_rounds = ((num_pes as f64).log2() / (n as f64).log2()).ceil();
        self.barrier_id += 1;
        let barrier_ptr = self.barrier_mr.get_mem().borrow().as_ptr() as usize;
        let src = unsafe{ std::slice::from_raw_parts(&self.barrier_id as *const usize as *const u8, std::mem::size_of::<usize>())};
        debug_println!("\tBarrierID: {}\n\tNum rounds: {}", self.barrier_id, num_rounds);
        
        for round in 0..num_rounds as usize {
            for i in 1..=n {
                let send_pe = euclid_rem(self.world.my_id  as i64 + i  as i64 * (n as i64 + 1 ).pow(round as u32), self.world.nnodes as i64 );
                
                let dst = barrier_ptr + 8 * self.world.my_id;
                debug_println!("\tP[{}] Round {} Sending BarrierID to: {}", self.world.my_id, round, send_pe);
                
                unsafe { self.put(dst, src, send_pe).await.unwrap() };
            }
            
            for i in 1..=n {
                let recv_pe = euclid_rem(self.world.my_id as i64 - i as i64 * (n  as i64 + 1).pow(round as u32), self.world.nnodes as i64);
                let barrier_vec = unsafe {std::slice::from_raw_parts(barrier_ptr as *const usize,  self.world.nnodes) };
                
                debug_println!("\tP[{}] Round {} Receiving BarrierID from: {}, Current Value: {}", self.world.my_id, round, recv_pe, barrier_vec[recv_pe]);
                while self.barrier_id > barrier_vec[recv_pe] {
                    transport::progress(&self.rx.cq, self.rx.cq_seq, &mut self.rx.cq_cntr);
                    transport::progress(&self.tx.cq, self.tx.cq_seq, &mut self.tx.cq_cntr);
                    std::thread::yield_now();
                }
            } 
        }
        
        debug_println!("P[{}] End calling Barrier", self.world.my_id);
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
        
        let n = 2_usize;
        let num_pes = pes.len();
        let num_rounds = ((num_pes as f64).log2() / (n as f64).log2()).ceil();
    
        self.barrier_id += 1;
        let barrier_ptr = self.barrier_mr.get_mem().borrow().as_ptr() as usize;

        let src = unsafe{ std::slice::from_raw_parts(&self.barrier_id as *const usize as *const u8, std::mem::size_of::<usize>())};
        

        for round in 0..num_rounds as usize {
            
            {
                let id = self.world.my_id as i64;
                let mut futs = Vec::new();
                for i in 1..=n {
                    let send_pe = euclid_rem(id + i as i64 * (n as i64 + 1).pow(round as u32), num_pes as i64 );
                    let send_pe = pes[send_pe];
                    let dst = barrier_ptr + 8 * id as usize ;
                    
                    // futs.insert(0, unsafe { self.put(dst,  src, send_pe)})
                    futs.push(unsafe { self.put(dst,  src, send_pe)})
                }
                debug_println!("Waiting for all puts");
                futures::future::join_all(futs).await;
            }


            for i in 1..=n {
                let recv_pe = euclid_rem(self.world.my_id as i64 - i as i64 * (n as i64 + 1).pow(round as u32), num_pes as i64 );
                let recv_pe = pes[recv_pe];
                let barrier_vec = unsafe {std::slice::from_raw_parts(barrier_ptr as *const usize,  8 * self.world.nnodes) };
                
                while self.barrier_id > barrier_vec[recv_pe] {
                    transport::progress(&self.rx.cq, self.rx.cq_seq, &mut self.rx.cq_cntr);
                    transport::progress(&self.tx.cq, self.tx.cq_seq, &mut self.tx.cq_cntr);

                    std::thread::yield_now();
                }
            } 
        }
    }

    fn mr_get(&self, addr: usize) -> Option<Rc<MappedMemoryRegion>>{
        
        self.mr_manager.borrow().mr_get(addr)
    }

    fn mr_get_from_remote(&self, addr: usize, remote_id: usize) -> Option<Rc<MappedMemoryRegion>> {

        self.mr_manager.borrow().mr_get_from_remote(addr, remote_id)
    }

    async unsafe fn put_(&self, mut dst: usize, src: &[u8], id: usize, block: bool) -> Result<(), std::io::Error> {

        let mem = self.mr_get(dst).unwrap();
        // let mem_key = 
        //     match mem.get_remote_key(id) {
        //         MemoryRegionKey::Key(key) => {
        //             key
        //         }
        //         _ => todo!()
        //     };

        let mapped_key =  mem.get_remote_key(id);

        dst = dst - mem.get_start() +  mem.get_remote_start(id);
        let rma_iov = if self.info.get_domain_attr().get_mr_mode().is_basic() || 
        self.info.get_domain_attr().get_mr_mode().is_virt_addr() {
            libfabric::iovec::RmaIoVec::new().address( dst as u64)
        }
        else {
            libfabric::iovec::RmaIoVec::new()
        };
        
        let mut rma_info = RmaInfo::new(rma_iov.get_address(), rma_iov.get_len(), mapped_key);
        
        if std::mem::size_of_val(src) < self.info.get_tx_attr().get_inject_size() {
            debug_println!("P[{}] Injecting put to P[{}]:\n\tSource ptr: {}\n\tDestination ptr (real): {}\n", self.world.my_id, id, src.as_ptr() as usize, dst);
            unsafe { self.post_rma_inject(&RmaOp::RmaWrite, &rma_info, src, id) };
        }
        else {
            debug_println!("P[{}] Putting to P[{}]:\n\tSource ptr: {}\n\tDestination ptr (real): {}\n", self.world.my_id, id, src.as_ptr() as usize, dst);       
            
            let mut curr_idx = 0;
            
            while curr_idx < src.len() {
                let msg_len = std::cmp::min(src.len() - curr_idx, self.info.get_ep_attr().get_max_msg_size()); 
                self.post_rma(&RmaOp::RmaWrite, &rma_info, &src[curr_idx..curr_idx+msg_len], &mut mem.get_mr_desc(), id).await;
                dst += msg_len;
                curr_idx += msg_len;
                rma_info.mem_address = dst as u64;
            }
        }

        Ok(())
    }

    async unsafe fn get_(&mut self, mut src: usize, dst: &mut[u8], id: usize, block: bool) -> Result<(), std::io::Error> {
        
        let mem = self.mr_get(dst.as_ptr() as usize).unwrap();
        // let mem_key = 
        //     match mem.get_remote_key(id) {
        //         MemoryRegionKey::Key(key) => {
        //             key
        //         }
        //         _ => todo!()
        //     };
        src = src - mem.get_start() +  mem.get_remote_start(id);
        let mapped_key =  mem.get_remote_key(id);


        let rma_iov = if self.info.get_domain_attr().get_mr_mode().is_basic() || 
            self.info.get_domain_attr().get_mr_mode().is_virt_addr() {

            libfabric::iovec::RmaIoVec::new().address( src as u64)
        }
        else {
            libfabric::iovec::RmaIoVec::new()

        };
        debug_println!("P[{}] Getting from P[{}]:\n\tSource ptr (real): {}\n\tDestination ptr: {}\n", self.world.my_id, id, src, dst.as_ptr() as usize);       
        let mut rma_info = RmaInfo::new(rma_iov.get_address(), rma_iov.get_len(), mapped_key);

        let mut curr_idx = 0;

        while curr_idx < dst.len() {
            let msg_len = std::cmp::min(dst.len() - curr_idx,  self.info.get_ep_attr().get_max_msg_size());
            println!("P[{}] Reading", self.world.my_id);
            self.post_rma_mut(&RmaOp::RmaRead, &rma_info, &mut dst[curr_idx..curr_idx+msg_len], &mut mem.get_mr_desc(), id).await;
            println!("P[{}] Done reading", self.world.my_id);
            src += msg_len;
            curr_idx += msg_len;
            rma_info.mem_address = src as u64;
        }

        debug_println!("P[{}] Done with get", self.world.my_id);
        Ok(())
    }

    fn check_context_comp(&mut self, ctx: &libfabric::Context) -> bool {

        let ret = self.tx.cq.read(1);
        
        match ret {
            Ok(completion) => {
                match completion {
                    Completion::Ctx(context_entry) => {
                        if context_entry.len() == 1{

                            self.tx.cq_cntr += 1; 

                            if context_entry[0].is_op_context_equal(ctx) { //[TODO! CRITICAL]
                                return true;
                            }
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

        false
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

    fn wait_get_all(&mut self) -> Result<(), Error> {

        transport::wait_on_cntr(&mut self.rx.cq_seq, &self.rx.cntr)
    }

    fn check_get_all(&self) -> bool {

        transport::check_cntr(&self.rx.cq_seq, &self.rx.cntr)
    }

    fn wait_put_all(&mut self) -> Result<(), Error> {

        transport::wait_on_cntr(&mut self.tx.cq_seq, &self.tx.cntr)
    }

    fn check_put_all(&self) -> bool {
        transport::check_cntr(&self.tx.cq_seq, &self.tx.cntr)
    }

    async fn sub_exchange_mr_info(&mut self, addr: u64, key: u64, pes: &[usize]) -> Vec<libfabric::iovec::RmaIoVec> {

        debug_println!("P[{}] Exchaning mr info with subgroup", self.world.my_id);
        let mut av_set = AddressVectorSetBuilder::new(&self.av)
            .count(pes.len())
            .start_addr(&self.world.addresses[pes[0]])
            .end_addr(&self.world.addresses[pes[0]])
            .stride(1)
            .build()
            .unwrap();

        for pe in pes.iter().skip(1) {
            av_set.insert(&self.world.addresses[*pe]).unwrap();
        }

        let address = av_set.get_addr().unwrap();
        // debug_println!("\tP[{}] AV set address: {}", self.world.my_id, address);
        let mut bank = self.ctx_bank.borrow_mut();
        let ctx = bank.create();
        let mut mut_ctx = ctx.borrow_mut();
        
        // debug_println!("\tP[{}] Creating collective join ctx: {}", self.world.my_id, &mut *mut_ctx as *mut libfabric::Context as usize);
        debug_println!("\tP[{}] Creating collective join", self.world.my_id);
        // let (_, mc) = self.ep.join_collective_async(&address, &av_set, JoinOptions::new()).await.unwrap();
        let mc = self.ep.join_collective_with_context(&address, &av_set, JoinOptions::new(), &mut *mut_ctx).unwrap();
        // debug_println!("\tP[{}] Waiting collective join {}", self.world.my_id, &*mut_ctx as *const libfabric::Context as usize);
        transport::wait_on_event_join(&self.eq, &mut self.tx.cq_cntr, &mut self.rx.cq_cntr, &self.tx.cq, &self.rx.cq, &mut_ctx);
        
        // let address = mc.get_addr();
        // debug_println!("\tP[{}] Done creating collective. MC address: {}", self.world.my_id, address);
        
        let mut rma_iov = libfabric::iovec::RmaIoVec::new().address(addr).key(key);
        debug_println!("\tP[{}] Allgather the following address: {} {}", self.world.my_id, addr, key);
        
        let mut rma_iovs = (0..pes.len()).map(|_| libfabric::iovec::RmaIoVec::new()).collect::<Vec<_>>();
        
        mc.allgather_async(std::slice::from_mut(&mut rma_iov), &mut libfabric::mr::default_desc(), &mut rma_iovs, &mut libfabric::mr::default_desc(), TferOptions::new()).await.unwrap();
        
        // transport::wait_on_context_comp(&mut_ctx, &self.tx.cq, &mut self.tx.cq_cntr);
        
        debug_println!("\tP[{}] Got the following addresses ({}) from all gather:", self.world.my_id,  rma_iovs.len());
        
        #[allow(unused_variables)]
        for iov in rma_iovs.iter() {
            debug_println!("\t\tP[{}] {} {}", self.world.my_id, iov.get_address(), iov.get_key());
        }
        debug_println!("P[{}] Done exchaning mr info with subgroup", self.world.my_id);

        rma_iovs
    }

    async fn exchange_mr_info(&mut self, addr: u64, key: u64) -> Vec<libfabric::iovec::RmaIoVec> {

        let pes: Vec<_> = (0..self.world.nnodes).collect();
        self.sub_exchange_mr_info(addr, key, &pes).await
    }

    unsafe fn post_rma_inject(&self, rma_op: &RmaOp, remote: &RmaInfo, buf: &[u8], id: usize) { // Unsafe because we write to remote 
        
        match rma_op {
            
            RmaOp::RmaWrite => {
                let addr = remote.mem_address();
                let key = remote.key();
                let remote_address = &self.world.addresses[id];
                let ep = &self.ep;
                let mut cq_cntr = 0; 
                let mut _cq_seq = 0;
                unsafe{ crate::transport::post!(inject_write, transport::progress, &self.tx.cq, _cq_seq, &mut cq_cntr, "fi_write", ep, buf, remote_address, addr, &key); }
            }
    
            RmaOp::RmaWriteData => {
                todo!();
                // let addr = remote.mem_address() as u64;
                // let key = remote.key();
                // let buf = &buf[..size];
                // let remote_cq_data = self..remote_cq_data;
                // unsafe{ tranport::ft_post!(inject_writedata, tranport::progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "fi_writedata", ep, buf, remote_cq_data, self.world.addresses[id], addr, key); }
            }
            RmaOp::RmaRead => {
                panic!("post_rma_inject does not support read");
            }
        }
    }

    async unsafe  fn  post_rma_mut(&mut self, rma_op: &RmaOp, remote: &RmaInfo, buf: &mut [u8], mr_desc: &mut MemoryRegionDesc, id: usize) {

        let remote_address = &self.world.addresses[id];

        match rma_op {
            
            RmaOp::RmaWrite => {
                let addr = remote.mem_address();
                let key = remote.key();
                unsafe{ transport::post!(write_async, &self.ep, buf, mr_desc, remote_address, addr, &key); }
            }
    
            RmaOp::RmaWriteData => {
                todo!();

                // let addr = remote.mem_address() as u64;
                // let remote_address = self.world.addresses[id];
                // let key = remote.key();
                // let remote_cq_data = gl_ctx.remote_cq_data;
                // unsafe{ tranport::ft_post!(writedata, tranport::progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "fi_write", ep, buf, data_desc, remote_cq_data, fi_addr, addr, key); }
            }
            
            RmaOp::RmaRead => {
                let addr = remote.mem_address();
                let key = remote.key();
                unsafe{ transport::post!(read_async, &self.ep, buf, mr_desc, remote_address, addr, &key); }
            }
        }
    }

    // unsafe fn  post_rma(&mut self, rma_op: &RmaOp, remote: &RmaInfo, buf: &[u8], mr_desc: &mut MemoryRegionDesc,  id: usize) {
    //     let remote_address = &self.world.addresses[id];
    //     let mut bank = self.ctx_bank.borrow_mut();
    //     let ctx = bank.create();

    //     match rma_op {
            
    //         RmaOp::RmaWrite => {
    //             let addr = remote.mem_address();
    //             let key = remote.key();
    //             unsafe{ transport::post!(write_with_context, transport::progress, &self.tx.cq, self.tx.cq_seq, &mut self.tx.cq_cntr, "fi_write", &self.ep, buf, mr_desc, remote_address, addr, &key, &mut *ctx.borrow_mut()); }
    //         }
    
    //         RmaOp::RmaWriteData => {
    //             todo!();

    //             // let addr = remote.get_address() as u64;
    //             // let remote_address = self.world.addresses[id];
    //             // let key = remote.get_key();
    //             // let remote_cq_data = gl_ctx.remote_cq_data;
    //             // unsafe{ tranport::ft_post!(writedata, tranport::progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "fi_write", ep, buf, data_desc, remote_cq_data, fi_addr, addr, key); }
    //         }
    //         _ => panic!("Cannot use post_rma to read into local buffer. Use post_rma_mut instead")
    //     }
    // }
    
    async unsafe fn  post_rma(&self, rma_op: &RmaOp, remote: &RmaInfo, buf: &[u8], mr_desc: &mut MemoryRegionDesc,  id: usize) {
        let remote_address = &self.world.addresses[id];

        match rma_op {
            
            RmaOp::RmaWrite => {
                let addr = remote.mem_address();
                let key = remote.key();
                // let _ = self.ep.write_async(buf, mr_desc, remote_address, addr, &key).await.unwrap();
                unsafe{ transport::post!(write_async, &self.ep, buf, mr_desc, remote_address, addr, &key); }
            }
    
            RmaOp::RmaWriteData => {
                todo!();

                // let addr = remote.get_address() as u64;
                // let remote_address = self.world.addresses[id];
                // let key = remote.get_key();
                // let remote_cq_data = gl_ctx.remote_cq_data;
                // unsafe{ tranport::ft_post!(writedata, tranport::progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "fi_write", ep, buf, data_desc, remote_cq_data, fi_addr, addr, key); }
            }
            _ => panic!("Cannot use post_rma to read into local buffer. Use post_rma_mut instead")
        }
    }
} 

fn euclid_rem(a: i64, b: i64) -> usize {
    let r = a % b;

    if r>= 0 {r as usize} else {(r + b.abs()) as usize}
}

#[cfg(test)]
mod tests {
    macro_rules! define_test {
        ($func_name:ident, $async_fname:ident, $body: block) => {
            
            // #[cfg(feature= "use-async-std")]
            #[test]
            fn $func_name() {
                async_std::task::block_on(async {$async_fname().await});
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

    use super::RofiBuilder;
    // use tokio;

    // #[tokio::test]
    // async fn init_async() {
    define_test!(init, init_async,  {
        let _rofi = RofiBuilder::new().build().await.unwrap();
        // let _rofi = RofiBuilder::new().build().await.unwrap();
    });
    
    define_test!(alloc, alloc_async,  {
        let mut rofi = RofiBuilder::new().build().await.unwrap();
        rofi.alloc(256).await;
    });
    
    define_test!(sub_alloc, sub_alloc_async, {
        let exclude_id = 1;
        const N: usize = 256;
        let mut rofi = RofiBuilder::new().build().await.unwrap();
        let size = rofi.get_size();
        assert!(size > 2);
        
        if rofi.get_id() != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter(|x|  x != &exclude_id ).collect();
            let pes_len = pes.len();
            let _mem = rofi.sub_alloc(N * pes_len, &pes).await;
        }
    });

    define_test!(sub_put, sub_put_async, {
        let exclude_id = 1;

        const N: usize = 256;
        let mut rofi = RofiBuilder::new().build().await.unwrap();
        let my_id = rofi.get_id();
        let size = rofi.get_size();
        assert!(size > 2);
        
        if rofi.get_id() != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter(|x| x != &exclude_id).collect();
            let me = pes.iter().position(|x| x == &my_id).unwrap();

            let pes_len = pes.len();
            let send_id = pes[(me + 1) % pes_len];
            let other = if me as i64 - 1 < 0  {pes.len() as i64 - 1} else {me as i64 - 1} as usize;
            let mem = rofi.sub_alloc(N * pes_len, &pes).await;
            for i in 0..N {
                mem.get_mem().borrow_mut()[me* N + i] = (i % N) as u8;
                mem.get_mem().borrow_mut()[other* N + i] = 5;
            }
            rofi.sub_barrier(&pes).await;
            let dst = mem.get_start() + me  *  N;
            unsafe {rofi.put(dst, &mem.get_mem().borrow()[me*N..me*N+N], send_id).await.unwrap()};
            rofi.sub_barrier(&pes).await; // Make sure everyone has finished putting
            assert_eq!(&mem.get_mem().borrow()[me * N..me * N + N],&mem.get_mem().borrow()[other * N.. other*N + N]);
        }
    });

    define_test!(sub_get, sub_get_async, {
        let exclude_id = 1;

        const N: usize = 256;
        let mut rofi = RofiBuilder::new().build().await.unwrap();
        let my_id = rofi.get_id();
        let size = rofi.get_size();
        assert!(size > 2);
        
        if rofi.get_id() != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter(|x| x != &exclude_id).collect();
            let me = pes.iter().position(|x| x == &my_id).unwrap();

            let pes_len = pes.len();
            let recv_id = pes[(me + 1) % pes_len];
            let other = (me + 1) % pes_len;
            let mem = rofi.sub_alloc(N * pes_len, &pes).await;
            for i in 0..N {
                mem.get_mem().borrow_mut()[me* N + i] = (i % N) as u8;
                mem.get_mem().borrow_mut()[other* N + i] = 5;
            }
            let src = mem.get_start() + other  *  N;
            rofi.sub_barrier(&pes).await;
            unsafe {rofi.get(src, &mut mem.get_mem().borrow_mut()[other*N..other*N+N], recv_id).await.unwrap()};
            
            assert_eq!(&mem.get_mem().borrow()[me * N..me * N + N],&mem.get_mem().borrow()[other * N.. other*N + N]);
            rofi.sub_barrier(&pes).await; // Make sure everyone has finished getting
        }
    });

    define_test!(sub_barrier, sub_barrier_async, {
        let exclude_id = 1;
        let mut rofi = RofiBuilder::new().build().await.unwrap();
        let size = rofi.get_size();
        assert!(size > 2);

        if rofi.get_id() != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter(|x| x != &exclude_id).collect();
            rofi.sub_barrier(&pes).await;
        }
    });

    define_test!(put_inject, put_inject_async, {
        const N : usize = 1 << 7;
        let mut rofi = RofiBuilder::new().build().await.unwrap();
        let size = rofi.get_size();
        assert!(size >= 2);

        let my_id = rofi.get_id();
        let send_id = (my_id + 1) % size ;
        let recv_id =  if my_id as i64 - 1 < 0 {size as i64 -1 } else { my_id as i64 -1} as usize ;

        let mem = rofi.alloc( size * N).await;

        for i in 0..N {
            mem.get_mem().borrow_mut()[my_id* N + i] = (i % N) as u8;
        }
        
        rofi.barrier().await;
        let ptr =  my_id * N + mem.get_mem().borrow().as_ptr() as usize;
        unsafe { rofi.put(ptr, &mem.get_mem().borrow()[my_id * N..my_id* N + N ], send_id ) }.await.unwrap();
        
        rofi.barrier().await;

        assert_eq!(&mem.get_mem().borrow()[my_id * N..my_id * N + N],&mem.get_mem().borrow()[recv_id * N.. recv_id*N + N]);
    });


    define_test!(put, put_async, {
        const N : usize = 1 << 20;
        let mut rofi = RofiBuilder::new().build().await.unwrap();
        let size = rofi.get_size();
        assert!(size >= 2);

        let my_id = rofi.get_id();
        let send_id = (my_id + 1) % size ;
        let recv_id =  if my_id as i64 - 1 < 0 {size as i64 -1 } else { my_id as i64 -1} as usize ;

        let mem = rofi.alloc(N * size).await;

        for i in 0..N {
            mem.get_mem().borrow_mut()[my_id* N + i] = (i % N) as u8;
            mem.get_mem().borrow_mut()[recv_id* N + i] = 5;
        }

        rofi.barrier().await;

        let ptr =  my_id * N + mem.get_mem().borrow().as_ptr() as usize;
        {

            let target_mem = &mem.get_mem().borrow_mut()[my_id * N..my_id* N + N ];
            let put0 = unsafe { rofi.put(ptr, target_mem, send_id ) };
            let put1 = unsafe { rofi.put(ptr, target_mem, send_id ) };
            let put2 = unsafe { rofi.put(ptr, target_mem, send_id ) };
            let (r0, r1, r2) = futures::join!(put1, put2, put0);
            r0.unwrap();
            r1.unwrap();
            r2.unwrap();
        }

        rofi.barrier().await; // Make sure everyone has finished putting


        assert_eq!(&mem.get_mem().borrow()[my_id * N..my_id * N + N],&mem.get_mem().borrow()[recv_id * N.. recv_id*N + N]);
    });
  
    define_test!(get, get_async, {
        const N : usize = 1 << 7;
        async_std::task::block_on(async {
            let mut rofi = RofiBuilder::new().build().await.unwrap();
            let size = rofi.get_size();
            assert!(size >= 2);

            let my_id = rofi.get_id();
            let recv_id = (my_id + 1) % size ;

            let mem = rofi.alloc(N* rofi.get_size()).await;

            for i in 0..N {
                mem.get_mem().borrow_mut()[my_id * N + i] = (i % N) as u8;
                mem.get_mem().borrow_mut()[recv_id * N + i] = 255;
            }
            rofi.barrier().await;
            
            let ptr =  recv_id*N + mem.get_mem().borrow().as_ptr() as usize;
            unsafe { rofi.get(ptr, &mut mem.get_mem().borrow_mut()[recv_id * N..recv_id* N + N ], recv_id ) }.await.unwrap();
            
            rofi.barrier().await; // Make sure all PEs have finished reading from remote
            assert_eq!(&mem.get_mem().borrow()[my_id * N..my_id * N + N],&mem.get_mem().borrow()[recv_id * N.. recv_id*N + N]);
        });
    });


}