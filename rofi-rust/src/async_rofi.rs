use libfabric::*;
use crate::RmaOp;
use crate::euclid_rem;
use crate::Rofi;
use std::cell::RefCell;
use std::rc::Rc;
use debug_print::debug_println;
use crate::MappedMemoryRegion;

pub struct RofiAsyncRmaFuture<'a> {
    rofi: RefCell<&'a mut Rofi>,
    rma_op: RmaOp,
}


impl<'a> async_std::future::Future  for RofiAsyncRmaFuture<'a> {
    type Output=();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        
        let done = match self.rma_op {
            RmaOp::RmaWrite => {
                self.rofi.borrow().check_put_all()
            },
            RmaOp::RmaRead => {
                self.rofi.borrow().check_get_all()
            },
            _ => {
                todo!()
            },
        };

        if done {std::task::Poll::Ready(())} else {cx.waker().wake_by_ref(); std::task::Poll::Pending}
    }
}


struct RofiAsyncBarrierRoundFuture<'a> {
    rofi: RefCell<&'a mut Rofi>,
    recv_pes: Vec<usize>,
}

impl<'a> async_std::future::Future  for RofiAsyncBarrierRoundFuture<'a> {
    type Output=();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let my_barrier_id = self.rofi.borrow().barrier_id;
        let barrier_mr = self.rofi.borrow().barrier_mr.clone();
        let barrier_ptr = barrier_mr.get_mem().borrow().as_ptr() as usize;
        let barrier_vec = unsafe {std::slice::from_raw_parts(barrier_ptr as *const usize,  self.rofi.borrow().get_size()) };
        
        let mut done = true;
        
        for recv_pe in self.recv_pes.iter() {
            if my_barrier_id > barrier_vec[*recv_pe] {
                done = false;
            }
        }

        if done {
            std::task::Poll::Ready(())
        } 
        else {
            self.rofi.borrow_mut().flush();
            cx.waker().wake_by_ref(); 
            std::task::Poll::Pending
        }
    }
    
}

pub struct RofiAsyncBarrierFuture<'a> {
    rofi: RefCell<&'a mut Rofi>,
    recv_pes: RefCell<Vec<Option<Vec<usize>>>>,
}

impl<'a> async_std::future::Future  for RofiAsyncBarrierFuture<'a> {
    type Output=();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {

        let my_barrier_id = self.rofi.borrow().barrier_id;
        let barrier_mr = self.rofi.borrow().barrier_mr.clone();
        let barrier_ptr = barrier_mr.get_mem().borrow().as_ptr() as usize;
        let barrier_vec = unsafe {std::slice::from_raw_parts(barrier_ptr as *const usize,  self.rofi.borrow().get_size()) };
        
        let mut done = true;
        
        for round_recv_pes in self.recv_pes.borrow_mut().iter_mut() {
            if let Some(recv_pes) = round_recv_pes {
                let mut curr_done = true;
                for recv_pe in recv_pes {
                    if my_barrier_id > barrier_vec[*recv_pe] {
                        done = false;
                        curr_done = false;
                    }
                }
                if curr_done {
                    round_recv_pes.take();
                }
            }
            
        }

        if done {
            std::task::Poll::Ready(())
        } 
        else {
            self.rofi.borrow_mut().flush();
            cx.waker().wake_by_ref(); 
            std::task::Poll::Pending
        }
    }
}


struct RofiAsyncContextCompFuture<'a> {
    rofi: RefCell<&'a mut Rofi>,
    ctx:  Rc<RefCell<Context>>,
}

impl<'a> async_std::future::Future for RofiAsyncContextCompFuture<'a> {
    type Output=();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut rofi = self.rofi.borrow_mut();

        let done = rofi.check_context_comp(&self.ctx.borrow_mut());

    
        if done {async_std::task::Poll::Ready(())} else {cx.waker().wake_by_ref(); async_std::task::Poll::Pending}
    }
}

struct RofiAsyncEventFuture<'a> {
    rofi: RefCell<&'a mut Rofi>,
    event: libfabric::enums::Event,
    ctx:  Rc<RefCell<Context>>,
}

impl<'a> async_std::future::Future for RofiAsyncEventFuture<'a> {
    type Output=();

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut rofi = self.rofi.borrow_mut();
        let done = rofi.check_event(&self.event, &self.ctx.borrow());
    
        if done {async_std::task::Poll::Ready(())} else {cx.waker().wake_by_ref(); async_std::task::Poll::Pending}
    }
}

impl Rofi {
    
    pub async fn sub_alloc_async(&mut self, size: usize, pes: &[usize]) ->  Rc<MappedMemoryRegion> {
        
        let mem = self.mr_manager.borrow_mut().alloc(&self.info, &self.domain, &self.ep, size);
        let ptr = mem.get_mem().borrow().as_ptr() as u64;
        let remote_iovs = self.sub_exchange_mr_info_async(ptr, mem.get_key(), pes).await;
        mem.set_sub_iovs(remote_iovs, pes);
    
        mem.clone()
    }

    pub async unsafe fn get_async<'a>(&'a mut self, src: usize, dst: &'a mut [u8], id: usize) {

        self.get_(src, dst, id, false).unwrap();
        RofiAsyncRmaFuture{rofi: std::cell::RefCell::new(self), rma_op: RmaOp::RmaRead}.await;
    }

    pub async unsafe fn put_async<'a>(&'a mut self, dst: usize, src: &'a [u8], id: usize) {

        self.put_(dst, src, id, false).unwrap();
        RofiAsyncRmaFuture{rofi: std::cell::RefCell::new(self), rma_op: RmaOp::RmaWrite}.await;
    }

    pub async fn barrier_async(&mut self) {
        debug_println!("P[{}] Calling Barrier:", self.world.my_id);
        let n = 2;
        let num_pes = self.world.nnodes ;
        let num_rounds = ((num_pes as f64).log2() / (n as f64).log2()).ceil();
        self.barrier_id += 1;
        let barrier_ptr = self.barrier_mr.get_mem().borrow().as_ptr() as usize;
        let src = unsafe{ std::slice::from_raw_parts(&self.barrier_id as *const usize as *const u8, std::mem::size_of::<usize>())};
        debug_println!("\tBarrierID: {}\n\tNum rounds: {}", self.barrier_id, num_rounds);
        
        let round_barriers =  (0_usize..num_rounds as usize).map(|round| {
            for i in 1..=n {
                let send_pe = euclid_rem(self.world.my_id  as i64 + i  as i64 * (n as i64 + 1 ).pow(round as u32), self.world.nnodes as i64 );
                
                let dst = barrier_ptr + 8 * self.world.my_id;
                debug_println!("\tP[{}] Round {} Sending BarrierID to: {}", self.world.my_id, round, send_pe);
                
                unsafe { self.iput(dst, src, send_pe).unwrap() };
            }
            
            let recv_pes =  (1..=n).map(|i| euclid_rem(self.world.my_id as i64 - i as i64 * (n  as i64 + 1).pow(round as u32) , self.world.nnodes as i64)).collect();
            Some(recv_pes)
        }).collect();
        
        debug_println!("P[{}] End calling Barrier", self.world.my_id);

        RofiAsyncBarrierFuture{rofi: std::cell::RefCell::new(self) , recv_pes: std::cell::RefCell::new(round_barriers)}.await;
    }

    pub async fn sub_barrier_async(&mut self, pes: &[usize]) {
        
        let n = 2_usize;
        let num_pes = pes.len();
        let num_rounds = ((num_pes as f64).log2() / (n as f64).log2()).ceil();
    
        self.barrier_id += 1;
        let barrier_ptr = self.barrier_mr.get_mem().borrow().as_ptr() as usize;

        let src = unsafe{ std::slice::from_raw_parts(&self.barrier_id as *const usize as *const u8, std::mem::size_of::<usize>())};
        

        let round_barriers =  (0_usize..num_rounds as usize).map(|round| {
            for i in 1..=n {
                let send_pe = euclid_rem(self.world.my_id as i64 + i as i64 * (n as i64 + 1).pow(round as u32), num_pes as i64 );
                let send_pe = pes[send_pe];
                let dst = barrier_ptr + 8 * self.world.my_id ;

                unsafe { self.iput(dst,  src, send_pe).unwrap() };
            }

            let recv_pes =  (1..=n).map(|i| {
                let recv_pe = euclid_rem(self.world.my_id as i64 - i as i64 * (n as i64 + 1).pow(round as u32), num_pes as i64 );
                pes[recv_pe]
            }).collect();

            Some(recv_pes) 
        }).collect();

        RofiAsyncBarrierFuture{rofi: std::cell::RefCell::new(self) , recv_pes: std::cell::RefCell::new(round_barriers)}.await;
    }

    async fn sub_exchange_mr_info_async(&mut self, addr: u64, key: u64, pes: &[usize]) -> Vec<RmaIoVec> {

        // let _guard = self.transport_mtx.lock().unwrap();
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
        let ctx = self.ctx_bank.borrow_mut().create();
        // let ctx = bank.create();
        
        debug_println!("\tP[{}] Creating collective join", self.world.my_id);
        let mc = self.ep.join_collective_with_context(address, &av_set, 0, &mut ctx.borrow_mut()).unwrap();
        RofiAsyncEventFuture{rofi: std::cell::RefCell::new(self), event: libfabric::enums::Event::JOIN_COMPLETE, ctx: ctx.clone()}.await;
        
        let address = mc.get_addr();
        debug_println!("\tP[{}] Done creating collective. MC address: {}", self.world.my_id, address);
        
        let mut rma_iov = libfabric::RmaIoVec::new().address(addr).key(key);
        debug_println!("\tP[{}] Allgather the following address: {} {}", self.world.my_id, addr, key);
        
        let mut rma_iovs = (0..pes.len()).map(|_| libfabric::RmaIoVec::new()).collect::<Vec<_>>();
        
        self.ep.allgather_with_context(std::slice::from_mut(&mut rma_iov), &mut libfabric::default_desc(), &mut rma_iovs, &mut libfabric::default_desc(), address, 0, &mut ctx.borrow_mut()).unwrap();
        RofiAsyncContextCompFuture{rofi: std::cell::RefCell::new(self), ctx: ctx.clone()}.await;
        
        debug_println!("\tP[{}] Got the following addresses ({}) from all gather:", self.world.my_id,  rma_iovs.len());
        #[allow(unused_variables)]
        for iov in rma_iovs.iter() {
            debug_println!("\t\tP[{}] {} {}", self.world.my_id, iov.get_address(), iov.get_key());
        }
        debug_println!("P[{}] Done exchaning mr info with subgroup", self.world.my_id);

        rma_iovs
    }
}


mod tests {
    #[test]
    fn put_async() {
        async_std::task::block_on(async {
            const N : usize = 1 << 8;
            let mut rofi = RofiBuilder::new().build().unwrap();
            let size = rofi.get_size();
            // assert!(size >= 2);

            let my_id = rofi.get_id();
            let send_id = (my_id + 1) % size ;
            let recv_id =  if my_id as i64 - 1 < 0 {size as i64 -1 } else { my_id as i64 -1} as usize ;

            let mem = rofi.alloc(N * size);

            for i in 0..N {
                mem.get_mem().borrow_mut()[my_id* N + i] = (i % N) as u8;
                mem.get_mem().borrow_mut()[recv_id* N + i] = 5;
            }

            rofi.barrier_async().await;

            let ptr =  my_id * N + mem.get_mem().borrow().as_ptr() as usize;
            let buff = &mem.get_mem().borrow()[my_id * N..my_id* N + N ];
            unsafe { rofi.put_async(ptr, buff, send_id ).await };

            rofi.barrier_async().await;
            assert_eq!(&mem.get_mem().borrow()[my_id * N..my_id * N + N],&mem.get_mem().borrow()[recv_id * N.. recv_id*N + N]);
        });
    }


    #[test]
    fn get_async() {
        async_std::task::block_on(async {
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

            rofi.barrier_async().await;
            
            let ptr =  recv_id*N + mem.get_mem().borrow().as_ptr() as usize;
            unsafe { rofi.get_async(ptr,  &mut mem.get_mem().borrow_mut()[recv_id * N..recv_id* N + N ], recv_id).await };
            
            rofi.barrier_async().await;

            assert_eq!(&mem.get_mem().borrow()[my_id * N..my_id * N + N],&mem.get_mem().borrow()[recv_id * N.. recv_id*N + N]);
        });
    }


    #[test]
    fn sub_barrier_async() {
        async_std::task::block_on(async {
            let exclude_id = 1;
            let mut rofi = RofiBuilder::new().build().unwrap();
            let size = rofi.get_size();
            assert!(size > 2);

            if rofi.get_id() != exclude_id {
                let pes: Vec<usize> = (0_usize..size).filter_map(|x| if x != exclude_id {Some(x)} else {None}).collect();
                rofi.sub_barrier_async(&pes).await;
            }
        });
    }

        
    #[test]
    fn sub_alloc_async() {
        async_std::task::block_on(async {
            let exclude_id = 1;

            const N: usize = 256;
            let mut rofi = RofiBuilder::new().build().unwrap();
            let size = rofi.get_size();
            assert!(size > 2);
            
            if rofi.get_id() != exclude_id {
                let pes: Vec<usize> = (0_usize..size).filter_map(|x| if x != exclude_id {Some(x)} else {None}).collect();
                let pes_len = pes.len();
                let _mem = rofi.sub_alloc_async(N * pes_len, &pes).await;
            }
        });
    }


    #[test]
    fn sub_put_async() {
        async_std::task::block_on(async {
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
                let mem = rofi.sub_alloc_async(N * pes_len, &pes).await;
                for i in 0..N {
                    mem.get_mem().borrow_mut()[me* N + i] = (i % N) as u8;
                    mem.get_mem().borrow_mut()[other* N + i] = 5;
                }

                rofi.sub_barrier_async(&pes).await;
                let dst = mem.get_start() + me  *  N;
                unsafe {rofi.put_async(dst, &mem.get_mem().borrow()[me*N..me*N+N], send_id).await};
                rofi.sub_barrier_async(&pes).await;
                // while mem.get_mem().borrow()[other*N] == 5 {}
                assert_eq!(&mem.get_mem().borrow()[me * N..me * N + N],&mem.get_mem().borrow()[other * N.. other*N + N]);
            }
        });
    }


    #[test]
    fn sub_get_async() {
        async_std::task::block_on(async {
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
                let mem = rofi.sub_alloc_async(N * pes_len, &pes).await;
                for i in 0..N {
                    mem.get_mem().borrow_mut()[me* N + i] = (i % N) as u8;
                    mem.get_mem().borrow_mut()[other* N + i] = 5;
                }
                rofi.sub_barrier_async(&pes).await;
                
                let src = mem.get_start() + other  *  N;
                unsafe {rofi.get_async(src, &mut mem.get_mem().borrow_mut()[other*N..other*N+N], recv_id).await};
                // while mem.get_mem().borrow()[other*N] == 5 {}
                rofi.sub_barrier_async(&pes).await;
                assert_eq!(&mem.get_mem().borrow()[me * N..me * N + N],&mem.get_mem().borrow()[other * N.. other*N + N]);
            }
        });
    }
}