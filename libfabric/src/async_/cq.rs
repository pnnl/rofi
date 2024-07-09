use crate::cq::{CompletionEntry, SingleCompletion};
use crate::fid::{AsRawTypedFid, CqRawFid};
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::fd::BorrowedFd;
use std::pin::Pin;
use std::{rc::Rc, future::Future, task::ready};
use crate::cq::CompletionQueueImplT; 
#[cfg(feature="use-async-std")]
use async_io::{Async as Async, Readable};
#[cfg(feature="use-tokio")]
use tokio::io::unix::AsyncFd as Async;
use crate::cq::WaitObjectRetrievable;     
use crate::cq::WaitableCompletionQueueImplT;    
use crate::{cq::{CompletionQueueImpl, CompletionQueueAttr, Completion, CtxEntry, DataEntry, TaggedEntry, MsgEntry, CompletionError, CompletionQueueBase}, error::Error, fid::{AsFid, RawFid, AsRawFid}, cqoptions::{CqConfig, self, Options}, FdRetrievable, MappedAddress, Waitable, WaitRetrievable, enums::WaitObjType};

use super::AsyncFid;
use super::{AsyncCtx, domain::{AsyncDomainImpl, Domain}};
macro_rules! alloc_cq_entry {
    ($format: expr, $count: expr) => {
        match $format {
            Completion::Ctx(_) => {
                let entries: Vec<CompletionEntry<CtxEntry>> = Vec::with_capacity($count);
                Completion::Ctx(entries)
            }
            Completion::Data(_) => {
                let entries: Vec<CompletionEntry<DataEntry>> = Vec::with_capacity($count);
                Completion::Data(entries)
            }
            Completion::Tagged(_) => {
                let entries: Vec<CompletionEntry<TaggedEntry>> = Vec::with_capacity($count);
                Completion::Tagged(entries)
            }
            Completion::Msg(_) => {
                let entries: Vec<CompletionEntry<MsgEntry>> = Vec::with_capacity($count);
                Completion::Msg(entries)
            }
            Completion::Unspec(_) => {
                let entries: Vec<CompletionEntry<CtxEntry>> = Vec::with_capacity($count);

                Completion::Unspec(entries)
            }
        }
    };
}

pub type CompletionQueue<T>  = CompletionQueueBase<T>;

pub trait AsyncCompletionQueueImplT: CompletionQueueImplT {
    fn read_in_async<'a>(&'a self, buf: &'a mut Completion, count: usize) -> CqAsyncRead;
    // fn async_transfer_wait(&self, async_ctx: &mut AsyncCtx) -> impl Future<Output = Result<SingleCompletion, crate::error::Error>>;
    fn wait_for_ctx_async(&self, async_ctx: &mut AsyncCtx) -> AsyncTransferCq;
    fn read_async(&self, count: usize) -> CqAsyncReadOwned;
}

impl CompletionQueue<AsyncCompletionQueueImpl> {
    pub(crate) fn new<T0>(domain: &Domain, attr: CompletionQueueAttr, context: Option<&mut T0>, default_buff_size: usize) -> Result<Self, crate::error::Error> {
        Ok(
            Self {
                inner: Rc::new(AsyncCompletionQueueImpl::new(&domain.inner, attr, context, default_buff_size)?),
            }
        )
    }
}

impl CompletionQueueImplT for AsyncCompletionQueueImpl {
    fn read(&self, count: usize) -> Result<Completion, crate::error::Error> {
        let mut borrowed_entries = self.base.get_ref().entry_buff.borrow_mut();
        self.read_in(count, &mut borrowed_entries)?;
        Ok(borrowed_entries.clone())
    }

    fn readfrom(&self, count: usize) -> Result<(Completion, Option<MappedAddress>), crate::error::Error> {
        let mut borrowed_entries = self.base.get_ref().entry_buff.borrow_mut();
        let address = self.readfrom_in(count, &mut borrowed_entries)?;
        Ok((borrowed_entries.clone(), address))
    }

    fn readerr(&self, flags: u64) -> Result<CompletionError, crate::error::Error> {
        let mut entry = self.base.get_ref().error_buff.borrow_mut();
        self.readerr_in(&mut entry, flags)?;
        Ok(entry.clone())
    }
}

impl<'a> WaitObjectRetrievable<'a> for AsyncCompletionQueueImpl {
    fn wait_object(&self) -> Result<WaitObjType<'a>, crate::error::Error> {
        
        if let Some(wait) = self.base.get_ref().wait_obj {
            if wait == libfabric_sys::fi_wait_obj_FI_WAIT_FD {
                let mut fd: i32 = 0;
                let err = unsafe { libfabric_sys::inlined_fi_control(self.as_raw_fid(), libfabric_sys::FI_GETWAIT as i32, (&mut fd as *mut i32).cast()) };
                if err < 0 {
                    Err(crate::error::Error::from_err_code((-err).try_into().unwrap()) )
                }
                else {
                    Ok(WaitObjType::Fd(unsafe{ BorrowedFd::borrow_raw(fd) }))
                }
            }
            else {
                panic!("Unexpected value for wait object in AsyncCompletionQueue");
            }
        }
        else {
            panic!("Unexpected value for wait object in AsyncCompletionQueue");
        }
    }
}


impl AsRawTypedFid for AsyncCompletionQueueImpl {
    type Output = CqRawFid;
    
    fn as_raw_typed_fid(&self) -> Self::Output {
        self.base.get_ref().as_raw_typed_fid()
    }
    
}

pub struct AsyncCompletionQueueImpl {
    base: Async<CompletionQueueImpl<true, true, true>>,
    pub(crate) pending_entries: RefCell<HashMap<usize, SingleCompletion>>,
}

impl WaitableCompletionQueueImplT for AsyncCompletionQueueImpl {
    fn sread(&self, count: usize, cond: usize, timeout: i32) -> Result<Completion, crate::error::Error>  {
        self.base.get_ref().sread(count, cond, timeout)
    }

    fn sreadfrom(&self, count: usize, cond: usize, timeout: i32) -> Result<(Completion, Option<MappedAddress>), crate::error::Error>  {
        self.base.get_ref().sreadfrom(count, cond, timeout)
    }
}

impl AsyncCompletionQueueImplT for AsyncCompletionQueueImpl {

    fn read_in_async<'a>(&'a self, buf: &'a mut Completion, count: usize) -> CqAsyncRead  {
        CqAsyncRead{num_entries: count, buf, cq: self, fut: None}
    }

    fn read_async(&self, count: usize) -> CqAsyncReadOwned {
        CqAsyncReadOwned::new(count, self)
    }

    fn wait_for_ctx_async(&self, async_ctx: &mut AsyncCtx) -> AsyncTransferCq {
        AsyncTransferCq::new(self, async_ctx as *mut AsyncCtx as usize)
    }
}

impl AsyncFid for AsyncCompletionQueueImpl {
    fn trywait(&self) -> Result<(), Error> {
        self.base.get_ref()._domain_rc.get_fabric_impl().trywait(self.base.get_ref())
    }
}

impl<T: AsyncCompletionQueueImplT> CompletionQueue<T> {
    pub async fn read_in_async<'a>(&'a self, buf: &'a mut Completion, count: usize) -> Result<(), crate::error::Error>  {
        self.inner.read_in_async(buf, count).await
    }

    pub async fn async_transfer_wait(&self, async_ctx: &mut AsyncCtx) -> Result<SingleCompletion, crate::error::Error>  {
        self.inner.wait_for_ctx_async(async_ctx).await
    }

    // pub async fn read_async(&self, count: usize) -> Result<Completion, crate::error::Error>  {
    //     self.inner.read_async(count).await
    // }
}

impl AsyncCompletionQueueImpl {

    pub(crate) fn new<T0>(domain: &Rc<AsyncDomainImpl>, attr: CompletionQueueAttr, context: Option<&mut T0>, default_buff_size: usize) -> Result<Self, crate::error::Error> {
        Ok(Self {base:Async::new(CompletionQueueImpl::new(domain, attr, context, default_buff_size)?).unwrap(), pending_entries: RefCell::new(HashMap::new())})
    }
}


pub(crate) struct AsyncTransferCq<'a>{
    pub(crate) ctx: usize,
    fut: Pin<Box<CqAsyncReadOwned<'a>>>,
}

impl<'a> AsyncTransferCq<'a> {
    #[allow(dead_code)]
    pub(crate) fn new(cq: &'a AsyncCompletionQueueImpl, ctx: usize) -> Self {
        Self {
            fut: Box::pin(CqAsyncReadOwned::new(1, cq)),
            ctx, 
        }
    }
}


impl<'a> Future for AsyncTransferCq<'a> {
    type Output=Result<SingleCompletion, Error>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut_self = self.get_mut();
        loop {
            if let Some(mut entry) = mut_self.fut.cq.pending_entries.borrow_mut().remove(&mut_self.ctx) {
                match entry {
                    SingleCompletion::Unspec(ref mut e) => {e.c_entry.op_context = unsafe{ ( *(e.c_entry.op_context as *mut AsyncCtx)).user_ctx.unwrap_or(std::ptr::null_mut())} },
                    SingleCompletion::Ctx(ref mut e) => {e.c_entry.op_context = unsafe{ ( *(e.c_entry.op_context as *mut AsyncCtx)).user_ctx.unwrap_or(std::ptr::null_mut())} },
                    SingleCompletion::Msg(ref mut e) => {e.c_entry.op_context = unsafe{ ( *(e.c_entry.op_context as *mut AsyncCtx)).user_ctx.unwrap_or(std::ptr::null_mut())} },
                    SingleCompletion::Data(ref mut e) => {e.c_entry.op_context = unsafe{ ( *(e.c_entry.op_context as *mut AsyncCtx)).user_ctx.unwrap_or(std::ptr::null_mut())} },
                    SingleCompletion::Tagged(ref mut e) => {e.c_entry.op_context = unsafe{ ( *(e.c_entry.op_context as *mut AsyncCtx)).user_ctx.unwrap_or(std::ptr::null_mut())} },
                }
                return std::task::Poll::Ready(Ok(entry));
            }
            #[allow(clippy::let_unit_value)]
            let _guard = ready!(mut_self.fut.as_mut().poll(cx))?;
            let mut found = None;
            match &mut_self.fut.buf {
                Completion::Unspec(entries) => for e in entries.iter() {if e.c_entry.op_context as usize == mut_self.ctx { unsafe{ (*(e.c_entry.op_context as *mut AsyncCtx)).user_ctx.unwrap_or(std::ptr::null_mut())}; found = Some(SingleCompletion::Unspec(e.clone()));} else  {mut_self.fut.cq.pending_entries.borrow_mut().insert(e.c_entry.op_context as usize, SingleCompletion::Unspec(e.clone()));}},
                Completion::Ctx(entries) => for e in entries.iter() { if e.c_entry.op_context as usize == mut_self.ctx { unsafe{ (*(e.c_entry.op_context as *mut AsyncCtx)).user_ctx.unwrap_or(std::ptr::null_mut())}; found = Some(SingleCompletion::Ctx(e.clone()));} else {mut_self.fut.cq.pending_entries.borrow_mut().insert(e.c_entry.op_context as usize, SingleCompletion::Ctx(e.clone()));}},
                Completion::Msg(entries) => for e in entries.iter() { if e.c_entry.op_context as usize == mut_self.ctx { unsafe{ (*(e.c_entry.op_context as *mut AsyncCtx)).user_ctx.unwrap_or(std::ptr::null_mut())}; found = Some(SingleCompletion::Msg(e.clone()));} else {mut_self.fut.cq.pending_entries.borrow_mut().insert(e.c_entry.op_context as usize, SingleCompletion::Msg(e.clone()));}},
                Completion::Data(entries) => for e in entries.iter() { if e.c_entry.op_context as usize == mut_self.ctx { unsafe{ (*(e.c_entry.op_context as *mut AsyncCtx)).user_ctx.unwrap_or(std::ptr::null_mut())}; found = Some(SingleCompletion::Data(e.clone()));} else {mut_self.fut.cq.pending_entries.borrow_mut().insert(e.c_entry.op_context as usize, SingleCompletion::Data(e.clone()));}},
                Completion::Tagged(entries) => for e in entries.iter() { if e.c_entry.op_context as usize == mut_self.ctx { unsafe{ (*(e.c_entry.op_context as *mut AsyncCtx)).user_ctx.unwrap_or(std::ptr::null_mut())}; found = Some(SingleCompletion::Tagged(e.clone()));} else {mut_self.fut.cq.pending_entries.borrow_mut().insert(e.c_entry.op_context as usize, SingleCompletion::Tagged(e.clone()));}},
            }
            match found {
                Some(v) => return std::task::Poll::Ready(Ok(v)),
                None => {  mut_self.fut  = Box::pin(CqAsyncReadOwned::new(1, mut_self.fut.cq));}
            }
        }
        
    }
}

pub struct CqAsyncRead<'a>{
    num_entries: usize,
    buf: &'a mut Completion,
    cq: &'a AsyncCompletionQueueImpl,
    #[cfg(feature = "use-async-std")]
    fut: Option<Pin<Box<Readable<'a, CompletionQueueImpl<true, true, true>>>>>,
    #[cfg(feature = "use-tokio")]
    fut: Option<Pin<Box<dyn Future<Output = Result<tokio::io::unix::AsyncFdReadyGuard<'a, CompletionQueueImpl<true, true, true>>, std::io::Error>> + 'a>>>,
}

impl<'a> Future for CqAsyncRead<'a>{
    type Output=Result<(), Error>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut_self = self.get_mut();
        loop {
            let (err, _guard) = if mut_self.cq.trywait().is_err() {
                (mut_self.cq.read_in(1, mut_self.buf), None)
            }
            else {
                if mut_self.fut.is_none() {
                    mut_self.fut = Some(Box::pin(mut_self.cq.base.readable()))
                }
                // Tokio returns something we need, async_std returns ()
                #[allow(clippy::let_unit_value)]
                let _guard = ready!(mut_self.fut.as_mut().unwrap().as_mut().poll(cx)).unwrap();
                
                // We only need to reset the option to none
                #[allow(clippy::let_underscore_future)]
                let _ = mut_self.fut.take().unwrap();
                (mut_self.cq.read_in(1, mut_self.buf), Some(_guard))
            };
                
            match err {
                Err(error) => {
                    if !matches!(error.kind, crate::error::ErrorKind::TryAgain) {
                        return std::task::Poll::Ready(Err(error));
                    }
                    else {
                        #[cfg(feature = "use-tokio")]
                        if let Some(mut guard) = _guard {if mut_self.cq.pending_entries.borrow().is_empty(){guard.clear_ready()}}
                        continue;
                    }
                },
                Ok(len) => {      
                    match &mut mut_self.buf {
                        Completion::Unspec(data) => unsafe{data.set_len(len)},
                        Completion::Ctx(data) => unsafe{data.set_len(len)},
                        Completion::Msg(data) => unsafe{data.set_len(len)},
                        Completion::Data(data) => unsafe{data.set_len(len)},
                        Completion::Tagged(data) => unsafe{data.set_len(len)},
                    }
                    return std::task::Poll::Ready(Ok(()));
                },
            }
        }
    }
}

impl<'a>  CqAsyncReadOwned<'a> {
    pub(crate) fn new( num_entries: usize, cq: &'a AsyncCompletionQueueImpl) -> Self {

        Self {
            buf:  alloc_cq_entry!(*cq.base.get_ref().entry_buff.borrow(), 1),
            num_entries,
            cq,
            fut: None,
        }
    }
}

pub struct CqAsyncReadOwned<'a>{
    num_entries: usize,
    buf: Completion,
    cq: &'a AsyncCompletionQueueImpl,
    #[cfg(feature = "use-async-std")]
    fut: Option<Pin<Box<Readable<'a, CompletionQueueImpl<true, true, true>>>>>,
    #[cfg(feature = "use-tokio")]
    fut: Option<Pin<Box<dyn Future<Output = Result<tokio::io::unix::AsyncFdReadyGuard<'a, CompletionQueueImpl<true, true, true>>, std::io::Error>> + 'a>>>,
}

impl<'a> Future for CqAsyncReadOwned<'a>{
    type Output=Result<(), Error>;

    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut_self = self.get_mut();
        loop {
            // println!("About to block");
            let (err, _guard) = if mut_self.cq.trywait().is_err() {
                // println!("Cannot block");
                (mut_self.cq.read_in(1, &mut mut_self.buf), None)
            }
            else {
                if mut_self.fut.is_none() {
                    mut_self.fut = Some(Box::pin(mut_self.cq.base.readable()))
                }
                #[allow(clippy::let_unit_value)]
                let _guard = ready!(mut_self.fut.as_mut().unwrap().as_mut().poll(cx)).unwrap();
                
                #[allow(clippy::let_underscore_future)]
                let _ = mut_self.fut.take().unwrap();
                // println!("Did not block");
                (mut_self.cq.read_in(1, &mut mut_self.buf), Some(_guard))
            };
                
            match err {
                Err(error) => {
                    if !matches!(error.kind, crate::error::ErrorKind::TryAgain) {
                        // println!("Erroring");
                        return std::task::Poll::Ready(Err(error));
                    }
                    else {
                        // println!("Will continue");
                        #[cfg(feature = "use-tokio")]
                        if let Some(mut guard) = _guard {if mut_self.cq.pending_entries.borrow().is_empty(){guard.clear_ready()}}
                        continue;
                    }
                },
                Ok(len) => {      
                    // println!("Actually read something {}", len);          
                    match &mut mut_self.buf {
                        Completion::Unspec(data) => unsafe{data.set_len(len)},
                        Completion::Ctx(data) => unsafe{data.set_len(len)},
                        Completion::Msg(data) => unsafe{data.set_len(len)},
                        Completion::Data(data) => unsafe{data.set_len(len)},
                        Completion::Tagged(data) => unsafe{data.set_len(len)},
                    }
                    return std::task::Poll::Ready(Ok(()));
                },
            }
        }
    }
}



impl AsFid for AsyncCompletionQueueImpl {
    fn as_fid(&self) -> crate::fid::BorrowedFid<'_> {
        self.base.get_ref().as_fid()
    }
}
impl AsFid for &AsyncCompletionQueueImpl {
    fn as_fid(&self) -> crate::fid::BorrowedFid<'_> {
        self.base.get_ref().as_fid()
    }
}
impl AsFid for Rc<AsyncCompletionQueueImpl> {
    fn as_fid(&self) -> crate::fid::BorrowedFid<'_> {
        self.base.get_ref().as_fid()
    }
}


impl AsRawFid for AsyncCompletionQueueImpl {
    fn as_raw_fid(&self) -> RawFid {
        self.base.get_ref().as_raw_fid()
    }
}

impl crate::BindImpl for AsyncCompletionQueueImpl {}
// impl<T: CqConfig + 'static> crate::Bind for CompletionQueue<T> {
//     fn inner(&self) -> Rc<dyn crate::BindImpl> {
//         self.inner.clone()
//     }
// }


pub struct CompletionQueueBuilder<'a, T> {
    cq_attr: CompletionQueueAttr,
    domain: &'a Domain,
    ctx: Option<&'a mut T>,
    options: cqoptions::Options<cqoptions::WaitRetrieve, cqoptions::On>,
    default_buff_size: usize,
}

    
impl<'a> CompletionQueueBuilder<'a, ()> {
    
    /// Initiates the creation of a new [CompletionQueue] on `domain`.
    /// 
    /// The initial configuration is what would be set if no `fi_cq_attr` or `context` was provided to 
    /// the `fi_cq_open` call. 
    pub fn new(domain: &'a Domain) -> CompletionQueueBuilder<()> {
        Self  {
            cq_attr: CompletionQueueAttr::new(),
            domain,
            ctx: None,
            options: Options::new().wait_fd(),
            default_buff_size: 10,
        }
    }
}

impl<'a, T> CompletionQueueBuilder<'a, T> {

    /// Specifies the minimum size of a completion queue.
    /// 
    /// Corresponds to setting the field `fi_cq_attr::size` to `size`.
    pub fn size(mut self, size: usize) -> Self {
        self.cq_attr.size(size);
        self
    }


    pub fn signaling_vector(mut self, signaling_vector: i32) -> Self { // [TODO]
        self.cq_attr.signaling_vector(signaling_vector);
        self
    }

    /// Specificies the completion `format`
    /// 
    /// Corresponds to setting the field `fi_cq_attr::format`.
    pub fn format(mut self, format: crate::enums::CqFormat) -> Self {
        self.cq_attr.format(format);
        self
    }
    
    pub fn default_buff_size(mut self, default_buff_size: usize) -> Self {
        self.default_buff_size = default_buff_size;
        self
    }

    /// Sets the context to be passed to the `CompletionQueue`.
    /// 
    /// Corresponds to passing a non-NULL `context` value to `fi_cq_open`.
    pub fn context(self, ctx: &'a mut T) -> CompletionQueueBuilder<'a, T> {
        CompletionQueueBuilder {
            ctx: Some(ctx),
            cq_attr: self.cq_attr,
            domain: self.domain,
            options: self.options,
            default_buff_size: self.default_buff_size,
        }
    }

    /// Constructs a new [CompletionQueue] with the configurations requested so far.
    /// 
    /// Corresponds to creating a `fi_cq_attr`, setting its fields to the requested ones,
    /// and passing it to the `fi_cq_open` call with an optional `context`.
    pub fn build(mut self) ->  Result<CompletionQueue<AsyncCompletionQueueImpl>, crate::error::Error> {
        self.cq_attr.wait_obj(crate::enums::WaitObj::Fd);
        CompletionQueue::<AsyncCompletionQueueImpl>::new(self.domain, self.cq_attr, self.ctx, self.default_buff_size)   
    }
}

#[cfg(test)]
mod tests {

    use crate::async_::{cq::*, domain::DomainBuilder};
    use crate::info::Info;

    #[test]
    fn cq_open_close_simultaneous() {
        let info = Info::new().request().unwrap();
        let entries = info.get();
        
        let fab = crate::fabric::FabricBuilder::new(&entries[0]).build().unwrap();
        let count = 10;
        let domain = DomainBuilder::new(&fab, &entries[0]).build().unwrap();
        // let mut cqs = Vec::new();
        for _ in 0..count {
            let _cq = CompletionQueueBuilder::new(&domain).build().unwrap();
        }
    }

    #[test]
    fn cq_open_close_sizes() {
        let info = Info::new().request().unwrap();
        let entries = info.get();
        
        let fab = crate::fabric::FabricBuilder::new(&entries[0]).build().unwrap();
        let domain = DomainBuilder::new(&fab, &entries[0]).build().unwrap();
        for i in -1..17 {
            let size = if i == -1 { 0 } else { 1 << i };
            let _cq = CompletionQueueBuilder::new(&domain).size(size)
                .build()
                .unwrap();
        }
    }
}

#[cfg(test)]
mod libfabric_lifetime_tests {
    use crate::async_::{cq::*, domain::DomainBuilder};
    use crate::info::Info;

    #[test]
    fn cq_drops_before_domain() {
        let info = Info::new().request().unwrap();
        let entries = info.get();
        
        let fab = crate::fabric::FabricBuilder::new(&entries[0]).build().unwrap();
        let count = 10;
        let domain = DomainBuilder::new(&fab, &entries[0]).build().unwrap();
        let mut cqs = Vec::new();
        for _ in 0..count {
            let cq = CompletionQueueBuilder::new(&domain)
                .build()
                .unwrap();
            println!("Count = {}", std::rc::Rc::strong_count(&domain.inner));
            cqs.push(cq);
        }
        drop(domain);
        println!("Count = {} After dropping domain\n", std::rc::Rc::strong_count(&cqs[0].inner.base.get_ref()._domain_rc));

    }
}