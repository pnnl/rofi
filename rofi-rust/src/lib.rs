mod pmi;
mod mr;
mod context;
mod transport;
mod async_rofi;
use std::cell::RefCell;
use std::rc::Rc;
use mr::{MappedMemoryRegion, MemoryRegionManager};
use debug_print::debug_println;
use context::ContextBank;
use libfabric::{av::AddressVector, cntr::Counter, cq::CompletionQueue, domain::Domain, enums::MrMode, ep::{Endpoint, EndpointAttr}, eq::{EventQueue, EventQueueAttr}, error::Error, fabric::Fabric, mr::MemoryRegionDesc, Address, CollectiveAttr, Info, InfoEntry, RmaIoVec, FID};

// Encapsulates data for the Tx/Rx operations
struct XxData {
    cq: CompletionQueue,
    cntr: Counter,
    cq_cntr: u64,
    cq_seq: u64,
}

struct CommWorld {
    nnodes: usize,
    my_id : usize,
    addresses: Vec<Address>
}

pub enum RmaOp {
    RmaWrite,
    RmaWriteData,
    RmaRead,
}

#[allow(dead_code)]
pub struct Rofi {                       // Note that the order in which libfabric structs are defined matters 
                                        // e.g. fabric has to be dropped after domain, so we define it after
    world: CommWorld,
    pmi: Box<dyn crate::pmi::PmiTrait>,
    pub(crate) mr_manager: Rc<RefCell<MemoryRegionManager>>,
    barrier_mr: Rc<MappedMemoryRegion>,
    ep: Endpoint,
    eq: EventQueue,
    tx: XxData,
    rx: XxData,
    av: AddressVector,
    domain: Domain,
    fabric: Fabric,
    pub(crate) info: InfoEntry,
    all_info: Info,
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
            pmi: Box::new(pmi::Pmi1::init().unwrap()),
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
            pmi: Box::new(pmi::Pmi1::init().unwrap()),
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
    pub fn build(self) -> Result<Rofi, libfabric::error::Error> {
        
        Rofi::init(self.pmi)
    }
}

impl Rofi {

    pub(crate) fn init(mut pmi: Box<dyn crate::pmi::PmiTrait>) -> Result<Rofi, libfabric::error::Error> {

        let caps = libfabric::InfoCaps::new().rma().atomic().collective();
        let hints = libfabric::InfoHints::new()
            .caps(caps)
            .domain_attr(
                libfabric::domain::DomainAttr::new()
                    .resource_mgmt(libfabric::enums::ResourceMgmt::ENABLED)
                    .threading(libfabric::enums::Threading::DOMAIN)
                    .mr_mode(MrMode::new().allocated().prov_key().virt_addr())
                    .data_progress(libfabric::enums::Progress::MANUAL)
                )
            .mode(libfabric::enums::Mode::new().context())
            .ep_attr(EndpointAttr::new().ep_type(libfabric::enums::EndpointType::RDM));
    
        let all_info = libfabric::Info::new().hints(&hints).request().unwrap();
        let entries = all_info.get();
        let info = entries[0].clone();
        let fabric = Fabric::new(info.get_fabric_attr().clone()).unwrap();
        let eq: EventQueue = fabric.eq_open(EventQueueAttr::new()).unwrap();
        let domain = fabric.domain(&info)?;
        domain.query_collective(libfabric::enums::CollectiveOp::ALLGATHER, CollectiveAttr::new(), 0).unwrap();
        let (tx_cq, rx_cq, tx_cntr, rx_cntr, av, ep) = transport::init_ep_resources(&info, &domain, &eq).unwrap();

        let mut addresses: Vec<Address> =  vec![u64::MAX; pmi.get_size()];
        let mut addr  = vec![0_u8; 16];

        let len = ep.getname(&mut addr)?;

        pmi.put("epname", &addr[0..len]).unwrap();
        pmi.exchange().unwrap();
        let mut all_addresses: Vec<u8> = vec![0 ; len * pmi.get_size()];
        for i in 0..pmi.get_size() {
            let res = pmi.get(i, "epname", len).unwrap();
            all_addresses[i* len..(i+1)*len].copy_from_slice(&res);
        }

        av.insert(&all_addresses, &mut addresses, 0).unwrap();

        let mr_manager = Rc::new(RefCell::new(MemoryRegionManager::new()));
        let barrier_size = pmi.get_size() * std::mem::size_of::<usize>();
        let barrier_mr = mr_manager.borrow_mut().alloc(&info, &domain, &ep, barrier_size);
        
        let mut rofi = Rofi {
            world: CommWorld{ nnodes: pmi.get_size(), my_id: pmi.get_rank(), addresses},
            pmi,
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
        
        let remote_iovs = rofi.exchange_mr_info(mr, key);
        rofi.barrier_mr.set_iovs(remote_iovs);



        Ok(rofi)
    }

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
    pub fn alloc(&mut self, size: usize) ->  Rc<mr::MappedMemoryRegion> {

        let mem = self.mr_manager.borrow_mut().alloc(&self.info, &self.domain, &self.ep, size);
        let remote_iovs = self.exchange_mr_info(mem.get_mem().borrow().as_ptr() as u64, mem.get_key());
        mem.set_iovs(remote_iovs);
        
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
    pub fn sub_alloc(&mut self, size: usize, pes: &[usize]) ->  Rc<mr::MappedMemoryRegion> {
        
        let mem = self.mr_manager.borrow_mut().alloc(&self.info, &self.domain, &self.ep, size);
        let remote_iovs = self.sub_exchange_mr_info(mem.get_mem().borrow().as_ptr() as u64, mem.get_key(), pes);
        mem.set_sub_iovs(remote_iovs, pes);
    
        mem.clone()
    }

    /// Initiate a transfer of data in `src` to the memory address `dst` at PE `id`and wait for its completion
    /// Buffer `src` can be reused immediately once the call returns.
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
    pub unsafe fn iput(&mut self, dst: usize, src: &[u8], id: usize) -> Result<(), std::io::Error> {

        self.put_(dst, src, id, true)
    }

    /// Initiate a transfer of data in `src` to the memory address `dst` at PE `id`and return immediately.
    /// Call returns before the operation has been completed so users are expected to check for its completion
    /// before modifying the data in `src`.
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
    /// unsafe { rofi.put(mem[0..256].as_ptr() as usize, &src, 1).unwrap() };
    /// ```
    pub unsafe fn put(&mut self, dst: usize, src: &[u8], id: usize) -> Result<(), std::io::Error> {

        self.put_(dst, src, id, false)
    }

    /// Initiate a transfer from data in memory address `src` at PE `id` to buffer slice `dst` and wait for its completion.
    /// Buffer `dst` can be used immediately once the call returns.
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
    /// unsafe { rofi.iget(mem[0..256].as_ptr() as usize, &mut dst, 1).unwrap() };
    /// ```
    pub unsafe fn iget(&mut self, src: usize, dst: &mut[u8], id: usize) -> Result<(), std::io::Error> {

        self.get_(src, dst, id, true)
    }

    /// Initiate a transfer from data in memory address `src` at PE `id` to buffer slice `dst` and return immediately.
    /// Call returns before the operation has been completed so users are expected to check for its completion
    /// before using the data in `dst`.
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
    pub unsafe fn get(&mut self, src: usize, dst: &mut[u8], id: usize) -> Result<(), std::io::Error> {

        self.get_(src, dst, id, false)
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
    pub fn barrier(&mut self) {
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
                
                unsafe { self.iput(dst, src, send_pe).unwrap() };
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
    pub fn sub_barrier(&mut self, pes: &[usize]) {
        
        let n = 2_usize;
        let num_pes = pes.len();
        let num_rounds = ((num_pes as f64).log2() / (n as f64).log2()).ceil();
    
        self.barrier_id += 1;
        let barrier_ptr = self.barrier_mr.get_mem().borrow().as_ptr() as usize;

        let src = unsafe{ std::slice::from_raw_parts(&self.barrier_id as *const usize as *const u8, std::mem::size_of::<usize>())};
        

        for round in 0..num_rounds as usize {
            for i in 1..=n {
                let send_pe = euclid_rem(self.world.my_id as i64 + i as i64 * (n as i64 + 1).pow(round as u32), num_pes as i64 );
                let send_pe = pes[send_pe];
                let dst = barrier_ptr + 8 * self.world.my_id ;

                unsafe { self.iput(dst,  src, send_pe).unwrap() };
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

    fn mr_get(&self, addr: usize) -> Option<Rc<mr::MappedMemoryRegion>>{
        
        self.mr_manager.borrow().mr_get(addr)
    }

    fn mr_get_from_remote(&self, addr: usize, remote_id: usize) -> Option<Rc<mr::MappedMemoryRegion>> {

        self.mr_manager.borrow().mr_get_from_remote(addr, remote_id)
    }

    unsafe fn put_(&mut self, mut dst: usize, src: &[u8], id: usize, block: bool) -> Result<(), std::io::Error> {

        let mem = self.mr_get(dst).unwrap();

        dst = dst - mem.get_start() +  mem.get_remote_start(id);
        let mut rma_iov = if self.info.get_domain_attr().get_mr_mode().is_basic() || 
        self.info.get_domain_attr().get_mr_mode().is_virt_addr() {
            libfabric::RmaIoVec::new().address( dst as u64)
        }
        else {
            libfabric::RmaIoVec::new()
        }.key(mem.get_remote_key(id));

        
        if std::mem::size_of_val(src) < self.info.get_tx_attr().get_inject_size() {
            debug_println!("P[{}] Injecting put to P[{}]:\n\tSource ptr: {}\n\tDestination ptr (real): {}\n\tKey: {}", self.world.my_id, id, src.as_ptr() as usize, dst, mem.get_remote_key(id));
            unsafe { self.post_rma_inject(&RmaOp::RmaWrite, &rma_iov, src, id) };

        }
        else {
            debug_println!("P[{}] Putting to P[{}]:\n\tSource ptr: {}\n\tDestination ptr (real): {}\n\tKey: {}", self.world.my_id, id, src.as_ptr() as usize, dst, mem.get_remote_key(id));       
            
            let mut curr_idx = 0;
            
            while curr_idx < src.len() {
                let msg_len = std::cmp::min(src.len() - curr_idx, self.info.get_ep_attr().get_max_msg_size()); 
                self.post_rma(&RmaOp::RmaWrite, &rma_iov, &src[curr_idx..curr_idx+msg_len], &mut mem.get_mr_desc(), id);
                dst += msg_len;
                curr_idx += msg_len;
                rma_iov = rma_iov.address(dst as u64);
            }
        }

        if block {
            self.wait_put_all().unwrap();
        }

        Ok(())
    }

    unsafe fn get_(&mut self, mut src: usize, dst: &mut[u8], id: usize, block: bool) -> Result<(), std::io::Error> {
        
        let mem = self.mr_get(dst.as_ptr() as usize).unwrap();

        src = src - mem.get_start() +  mem.get_remote_start(id);

        let mut rma_iov = if self.info.get_domain_attr().get_mr_mode().is_basic() || 
            self.info.get_domain_attr().get_mr_mode().is_virt_addr() {

            libfabric::RmaIoVec::new().address( src as u64)
        }
        else {
            libfabric::RmaIoVec::new()

        }.key(mem.get_remote_key(id));
        debug_println!("P[{}] Getting from P[{}]:\n\tSource ptr (real): {}\n\tDestination ptr: {}\n\tKey: {}", self.world.my_id, id, src, dst.as_ptr() as usize, mem.get_remote_key(id));       


        let mut curr_idx = 0;

        while curr_idx < dst.len() {
            let msg_len = std::cmp::min(dst.len() - curr_idx,  self.info.get_ep_attr().get_max_msg_size());
            self.post_rma_mut(&RmaOp::RmaRead, &rma_iov, &mut dst[curr_idx..curr_idx+msg_len], &mut mem.get_mr_desc(), id);
            src += msg_len;
            curr_idx += msg_len;
            rma_iov = rma_iov.address(src as u64);
        }


        if block {
            debug_println!("P[{}] Waiting for get completion", self.world.my_id);
            self.wait_get_all().unwrap();
        }
        debug_println!("P[{}] Done with get", self.world.my_id);
        Ok(())
    }

    fn check_context_comp(&mut self, ctx: &libfabric::Context) -> bool {

        let mut cq_err_entry = libfabric::cq::CqErrEntry::new();
    
        let ret = self.tx.cq.read(std::slice::from_mut(&mut cq_err_entry), 1);
        
        match ret {
            Ok(_) => {
                self.tx.cq_cntr += 1; 
            
                if cq_err_entry.is_op_context_equal(ctx) {
                    return true;
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

    fn check_event(&mut self, event: &libfabric::enums::Event, ctx: &libfabric::Context) -> bool {

        let mut eq_entry: libfabric::eq::EventQueueEntry<libfabric::Context> = libfabric::eq::EventQueueEntry::new();

        let ret = self.eq.read(std::slice::from_mut(&mut eq_entry));
        
        match ret {
            Ok((_, _ev)) => {
                if matches!(event, _ev) && eq_entry.is_context_equal(ctx) {
                        return true;
                }
            },
            Err(ref err) => {
                if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
                    ret.unwrap();
                }
            }
        }

        self.flush();
        // progress(self.tx.cq, 0, self.tx.cq_cntr);
        // progress(self.rx.cq, 0, rx_cq_cntr);

        false
    }

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

    fn sub_exchange_mr_info(&mut self, addr: u64, key: u64, pes: &[usize]) -> Vec<RmaIoVec> {

        debug_println!("P[{}] Exchaning mr info with subgroup", self.world.my_id);
        let mut av_set = self.av.avset(libfabric::av::AddressVectorSetAttr::new()
            .count(pes.len())
            .start_addr(self.world.addresses[pes[0]])
            .end_addr(self.world.addresses[pes[0]])
            .stride(1)
        ).unwrap();

        for pe in pes.iter().skip(1) {
            av_set.insert(self.world.addresses[*pe]).unwrap();
        }

        let address = av_set.get_addr().unwrap();
        debug_println!("\tP[{}] AV set address: {}", self.world.my_id, address);
        let mut bank = self.ctx_bank.borrow_mut();
        let ctx = bank.create();
        
        debug_println!("\tP[{}] Creating collective join", self.world.my_id);
        let mc = self.ep.join_collective_with_context(address, &av_set, 0, &mut ctx.borrow_mut()).unwrap();
        transport::wait_on_event(&libfabric::enums::Event::JOIN_COMPLETE, &self.eq, &mut self.tx.cq_cntr, &mut self.rx.cq_cntr, &self.tx.cq, &self.rx.cq, &ctx.borrow());
        
        let address = mc.get_addr();
        debug_println!("\tP[{}] Done creating collective. MC address: {}", self.world.my_id, address);
        
        let mut rma_iov = libfabric::RmaIoVec::new().address(addr).key(key);
        debug_println!("\tP[{}] Allgather the following address: {} {}", self.world.my_id, addr, key);
        
        let mut rma_iovs = (0..pes.len()).map(|_| libfabric::RmaIoVec::new()).collect::<Vec<_>>();
        
        self.ep.allgather_with_context(std::slice::from_mut(&mut rma_iov), &mut libfabric::default_desc(), &mut rma_iovs, &mut libfabric::default_desc(), address, 0, &mut ctx.borrow_mut()).unwrap();
        
        transport::wait_on_context_comp(&ctx.borrow(), &self.tx.cq, &mut self.tx.cq_cntr);
        
        debug_println!("\tP[{}] Got the following addresses ({}) from all gather:", self.world.my_id,  rma_iovs.len());
        
        #[allow(unused_variables)]
        for iov in rma_iovs.iter() {
            debug_println!("\t\tP[{}] {} {}", self.world.my_id, iov.get_address(), iov.get_key());
        }
        debug_println!("P[{}] Done exchaning mr info with subgroup", self.world.my_id);

        rma_iovs
    }

    fn exchange_mr_info(&mut self, addr: u64, key: u64) -> Vec<RmaIoVec> {

        let pes: Vec<_> = (0..self.world.nnodes).collect();
        self.sub_exchange_mr_info(addr, key, &pes)
    }

    unsafe fn post_rma_inject(&mut self, rma_op: &RmaOp, remote: &libfabric::RmaIoVec, buf: &[u8], id: usize) { // Unsafe because we write to remote 
        
        match rma_op {
            
            RmaOp::RmaWrite => {
                let addr = remote.get_address();
                let key = remote.get_key();
                let remote_address = self.world.addresses[id];
                let ep: &Endpoint = &self.ep;
                unsafe{ transport::post!(inject_write, transport::progress, &self.tx.cq, self.tx.cq_seq, &mut self.tx.cq_cntr, "fi_write", ep, buf, remote_address, addr, key); }
            }
    
            RmaOp::RmaWriteData => {
                todo!();
                // let addr = remote.get_address() as u64;
                // let key = remote.get_key();
                // let buf = &buf[..size];
                // let remote_cq_data = self..remote_cq_data;
                // unsafe{ tranport::ft_post!(inject_writedata, tranport::progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "fi_writedata", ep, buf, remote_cq_data, self.world.addresses[id], addr, key); }
            }
            RmaOp::RmaRead => {
                panic!("post_rma_inject does not support read");
            }
        }
        self.tx.cq_cntr += 1;
    }

    unsafe fn  post_rma_mut(&mut self, rma_op: &RmaOp, remote: &libfabric::RmaIoVec, buf: &mut [u8], mr_desc: &mut MemoryRegionDesc, id: usize) {

        let remote_address = self.world.addresses[id];
        let mut bank = self.ctx_bank.borrow_mut();
        let ctx = bank.create();

        match rma_op {
            
            RmaOp::RmaWrite => {
                let addr = remote.get_address();
                let key = remote.get_key();
                unsafe{ transport::post!(write_with_context, transport::progress, &self.tx.cq, self.tx.cq_seq, &mut self.tx.cq_cntr, "fi_write", &self.ep, buf, mr_desc, remote_address, addr, key, &mut *ctx.borrow_mut()); }
            }
    
            RmaOp::RmaWriteData => {
                todo!();

                // let addr = remote.get_address() as u64;
                // let remote_address = self.world.addresses[id];
                // let key = remote.get_key();
                // let remote_cq_data = gl_ctx.remote_cq_data;
                // unsafe{ tranport::ft_post!(writedata, tranport::progress, tx_cq, gl_ctx.tx_seq, &mut gl_ctx.tx_cq_cntr, "fi_write", ep, buf, data_desc, remote_cq_data, fi_addr, addr, key); }
            }
            
            RmaOp::RmaRead => {
                let addr = remote.get_address();
                let key = remote.get_key();
                unsafe{ transport::post!(read_with_context, transport::progress, &self.rx.cq, self.rx.cq_seq, &mut self.rx.cq_cntr, "fi_write", &self.ep, buf, mr_desc, remote_address, addr, key, &mut *ctx.borrow_mut()); }
            }
        }
    }

    unsafe fn  post_rma(&mut self, rma_op: &RmaOp, remote: &libfabric::RmaIoVec, buf: &[u8], mr_desc: &mut MemoryRegionDesc,  id: usize) {
        let remote_address = self.world.addresses[id];
        let mut bank = self.ctx_bank.borrow_mut();
        let ctx = bank.create();

        match rma_op {
            
            RmaOp::RmaWrite => {
                let addr = remote.get_address();
                let key = remote.get_key();
                unsafe{ transport::post!(write_with_context, transport::progress, &self.tx.cq, self.tx.cq_seq, &mut self.tx.cq_cntr, "fi_write", &self.ep, buf, mr_desc, remote_address, addr, key, &mut *ctx.borrow_mut()); }
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
    use crate::RofiBuilder;

    #[test]
    fn init() {
        let _rofi = RofiBuilder::new().build().unwrap();
    }
    
    #[test]
    fn alloc() {
        let mut rofi = RofiBuilder::new().build().unwrap();
        let _mem = rofi.alloc(256);
        // async_std::task::block_on(rofi.alloc(256).await)
    }
    
    #[test]
    fn sub_alloc() {
        let exclude_id = 1;

        const N: usize = 256;
        let mut rofi = RofiBuilder::new().build().unwrap();
        let size = rofi.get_size();
        assert!(size > 2);
        
        if rofi.get_id() != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter_map(|x| if x != exclude_id {Some(x)} else {None}).collect();
            let pes_len = pes.len();
            let _mem = rofi.sub_alloc(N * pes_len, &pes);
        }
    }

    #[test]
    fn sub_put() {
        let exclude_id = 1;

        const N: usize = 256;
        let mut rofi = RofiBuilder::new().build().unwrap();
        let my_id = rofi.get_id();
        let size = rofi.get_size();
        assert!(size > 2);
        
        if rofi.get_id() != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter_map(|x| if x != exclude_id {Some(x)} else {None}).collect();
            let me = pes.iter().position(|x| x == &my_id).unwrap();

            let pes_len = pes.len();
            let send_id = pes[(me + 1) % pes_len];
            let other = if me as i64 - 1 < 0  {pes.len() as i64 - 1} else {me as i64 - 1} as usize;
            let mem = rofi.sub_alloc(N * pes_len, &pes);
            for i in 0..N {
                mem.get_mem().borrow_mut()[me* N + i] = (i % N) as u8;
                mem.get_mem().borrow_mut()[other* N + i] = 5;
            }
            let dst = mem.get_start() + me  *  N;
            unsafe {rofi.put(dst, &mem.get_mem().borrow()[me*N..me*N+N], send_id).unwrap()};
            while mem.get_mem().borrow()[other*N] == 5 {}
            assert_eq!(&mem.get_mem().borrow()[me * N..me * N + N],&mem.get_mem().borrow()[other * N.. other*N + N]);
        }
    }

    #[test]
    fn sub_get() {
        let exclude_id = 1;

        const N: usize = 256;
        let mut rofi = RofiBuilder::new().build().unwrap();
        let my_id = rofi.get_id();
        let size = rofi.get_size();
        assert!(size > 2);
        
        if rofi.get_id() != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter_map(|x| if x != exclude_id {Some(x)} else {None}).collect();
            let me = pes.iter().position(|x| x == &my_id).unwrap();

            let pes_len = pes.len();
            let recv_id = pes[(me + 1) % pes_len];
            let other = (me + 1) % pes_len;
            let mem = rofi.sub_alloc(N * pes_len, &pes);
            for i in 0..N {
                mem.get_mem().borrow_mut()[me* N + i] = (i % N) as u8;
                mem.get_mem().borrow_mut()[other* N + i] = 5;
            }
            let src = mem.get_start() + other  *  N;
            unsafe {rofi.get(src, &mut mem.get_mem().borrow_mut()[other*N..other*N+N], recv_id).unwrap()};
            while mem.get_mem().borrow()[other*N] == 5 {}
            assert_eq!(&mem.get_mem().borrow()[me * N..me * N + N],&mem.get_mem().borrow()[other * N.. other*N + N]);
        }
    }

    #[test]
    fn sub_barrier() {
        let exclude_id = 1;
        let mut rofi = RofiBuilder::new().build().unwrap();
        let size = rofi.get_size();
        assert!(size > 2);

        if rofi.get_id() != exclude_id {
            let pes: Vec<usize> = (0_usize..size).filter_map(|x| if x != exclude_id {Some(x)} else {None}).collect();
            rofi.sub_barrier(&pes);
        }
    }

    #[test]
    fn put_inject() {
        const N : usize = 1 << 7;
        let mut rofi = RofiBuilder::new().build().unwrap();
        let size = rofi.get_size();
        assert!(size >= 2);

        let my_id = rofi.get_id();
        let send_id = (my_id + 1) % size ;
        let recv_id =  if my_id as i64 - 1 < 0 {size as i64 -1 } else { my_id as i64 -1} as usize ;

        let mem = rofi.alloc( size * N);

        for i in 0..N {
            mem.get_mem().borrow_mut()[my_id* N + i] = (i % N) as u8;
        }
        
        rofi.barrier();
        let ptr =  my_id * N + mem.get_mem().borrow().as_ptr() as usize;
        unsafe { rofi.iput(ptr, &mem.get_mem().borrow()[my_id * N..my_id* N + N ], send_id ) }.unwrap();
        
        rofi.barrier();

        assert_eq!(&mem.get_mem().borrow()[my_id * N..my_id * N + N],&mem.get_mem().borrow()[recv_id * N.. recv_id*N + N]);
    }


    #[test]
    fn put() {
        const N : usize = 1 << 8;
        let mut rofi = RofiBuilder::new().build().unwrap();
        let size = rofi.get_size();
        assert!(size >= 2);

        let my_id = rofi.get_id();
        let send_id = (my_id + 1) % size ;
        let recv_id =  if my_id as i64 - 1 < 0 {size as i64 -1 } else { my_id as i64 -1} as usize ;

        let mem = rofi.alloc(N * size);

        for i in 0..N {
            mem.get_mem().borrow_mut()[my_id* N + i] = (i % N) as u8;
            mem.get_mem().borrow_mut()[recv_id* N + i] = 5;
        }

        rofi.barrier();

        let ptr =  my_id * N + mem.get_mem().borrow().as_ptr() as usize;
        unsafe { rofi.put(ptr, &mem.get_mem().borrow()[my_id * N..my_id* N + N ], send_id ) }.unwrap();
        rofi.wait_put_all().unwrap();
        // rofi.barrier();
        while mem.get_mem().borrow()[recv_id*N] == 5 {}

        assert_eq!(&mem.get_mem().borrow()[my_id * N..my_id * N + N],&mem.get_mem().borrow()[recv_id * N.. recv_id*N + N]);
    }

    #[test]
    fn put_sync() {
        const N : usize = 1 << 8;
        let mut rofi = RofiBuilder::new().build().unwrap();
        let size = rofi.get_size();
        assert!(size >= 2);

        let my_id = rofi.get_id();
        let send_id = (my_id + 1) % size ;
        let recv_id =  if my_id as i64 - 1 < 0 {size as i64 -1 } else { my_id as i64 -1} as usize ;

        let mem = rofi.alloc(N * size);

        for i in 0..N {
            mem.get_mem().borrow_mut()[my_id* N + i] = (i % N) as u8;
            mem.get_mem().borrow_mut()[recv_id* N + i] = 5;
        }

        rofi.barrier();

        let ptr =  my_id * N + mem.get_mem().borrow().as_ptr() as usize;
        unsafe { rofi.iput(ptr, &mem.get_mem().borrow()[my_id * N..my_id* N + N ], send_id ) }.unwrap();

        rofi.barrier();
        assert_eq!(&mem.get_mem().borrow()[my_id * N..my_id * N + N],&mem.get_mem().borrow()[recv_id * N.. recv_id*N + N]);
    }

    #[test]
    fn get_sync() {
        const N : usize = 1 << 7;
        let mut rofi = RofiBuilder::new().build().unwrap();
        let size = rofi.get_size();
        assert!(size >= 2);

        let my_id = rofi.get_id();
        let recv_id = (my_id + 1) % size ;

        let mem = rofi.alloc(N* rofi.get_size());

        for i in 0..N {
            mem.get_mem().borrow_mut()[my_id* N + i] = (i % N) as u8;
            mem.get_mem().borrow_mut()[recv_id* N + i] = 255;
        }

        rofi.barrier();

        let ptr =  recv_id*N + mem.get_mem().borrow().as_ptr() as usize;
        unsafe { rofi.iget(ptr, &mut mem.get_mem().borrow_mut()[recv_id * N..recv_id* N + N ], recv_id ) }.unwrap();

        rofi.barrier();

        assert_eq!(&mem.get_mem().borrow()[my_id * N..my_id * N + N],&mem.get_mem().borrow()[recv_id * N.. recv_id*N + N]);
    }

    
    #[test]
    fn get() {
        const N : usize = 1 << 7;
        let mut rofi = RofiBuilder::new().build().unwrap();
        let size = rofi.get_size();
        assert!(size >= 2);

        let my_id = rofi.get_id();
        let recv_id = (my_id + 1) % size ;

        let mem = rofi.alloc(N* rofi.get_size());

        for i in 0..N {
            mem.get_mem().borrow_mut()[my_id * N + i] = (i % N) as u8;
            mem.get_mem().borrow_mut()[recv_id * N + i] = 255;
        }

        rofi.barrier();
        
        let ptr =  recv_id*N + mem.get_mem().borrow().as_ptr() as usize;
        unsafe { rofi.get(ptr, &mut mem.get_mem().borrow_mut()[recv_id * N..recv_id* N + N ], recv_id ) }.unwrap();
        
        rofi.barrier();
        while mem.get_mem().borrow()[recv_id*N] == 255 {}
        assert_eq!(&mem.get_mem().borrow()[my_id * N..my_id * N + N],&mem.get_mem().borrow()[recv_id * N.. recv_id*N + N]);
    }


}