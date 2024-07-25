use std::{os::fd::{AsFd, BorrowedFd}, rc::Rc, cell::{RefCell, OnceCell}, marker::PhantomData};


use libfabric_sys::{fi_wait_obj_FI_WAIT_FD, inlined_fi_control, FI_BACKLOG, FI_GETOPSFLAG};

#[allow(unused_imports)]
use crate::fid::AsFid;
use crate::{av::{AddressVector, AddressVectorBase}, cntr::{Counter, ReadCntr}, enums::{HmemP2p, TransferOptions}, eq::{EventQueueBase, ReadEq}, domain::DomainImplT, fabric::FabricImpl, utils::check_error, info::InfoEntry, fid::{self, AsRawFid, AsRawTypedFid, EpRawFid, OwnedEpFid, RawFid, PepRawFid, OwnedPepFid, AsTypedFid}, cq::ReadCq};

#[repr(C)]
pub struct Address {
    address: Vec<u8>,
}

impl Address {

    /// Creates a new Address object from a raw pointer, usually received from the network.
    /// 
    /// # Safety
    /// This function is unsafe since the contents of the pointer are not checked
    pub unsafe fn from_raw_parts(raw: *const u8, len: usize) -> Self {
        let mut address = vec![0u8; len];
        address.copy_from_slice(std::slice::from_raw_parts(raw, len));
        Self{address}
    }

    /// Creates a new Address object from a slice of bytes, usually received from the network.
    /// 
    /// # Safety
    /// This function is unsafe since the contents of the slice are not checked
    pub unsafe fn from_bytes(raw: &[u8]) -> Self {
        Address { address: raw.to_vec() }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.address
    } 
}

pub struct EndpointImplBase<T, EQ: ?Sized, CQ: ?Sized> {
    pub(crate) c_ep: OwnedEpFid,
    pub(crate) tx_cq: OnceCell<Rc<CQ>>,
    pub(crate) rx_cq: OnceCell<Rc<CQ>>,
    pub(crate) eq: OnceCell<Rc<EQ>>,
    _sync_rcs: RefCell<Vec<Rc<dyn AsRawFid>>>,
    _domain_rc:  Rc<dyn DomainImplT>,
    phantom: PhantomData<T>,
}

pub type  Endpoint<T>  = EndpointBase<EndpointImplBase<T, dyn ReadEq, dyn ReadCq>>;
pub struct EndpointBase<EP> {
    pub(crate) inner: Rc<EP>,
}

pub(crate) trait BaseEndpointImpl: AsRawTypedFid<Output = EpRawFid> {
    fn bind<T: crate::Bind + AsRawFid>(&self, res: &T, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_ep_bind(self.as_raw_typed_fid(), res.as_raw_fid(), flags) };
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(())
        }
    }
}


pub trait BaseEndpoint : AsRawFid {

    fn getname(&self) -> Result<Address, crate::error::Error> {
        let mut len = 0;
        let err: i32 = unsafe { libfabric_sys::inlined_fi_getname(self.as_raw_fid(), std::ptr::null_mut(), &mut len) };
        if -err as u32  == libfabric_sys::FI_ETOOSMALL {
            let mut address = vec![0; len];
            let err: i32 = unsafe { libfabric_sys::inlined_fi_getname(self.as_raw_fid(), address.as_mut_ptr().cast(), &mut len) };
            if err < 0
            {
                Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
            }
            else 
            {
                Ok(Address{address})
            }
        }
        else
        {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
    }

    fn buffered_limit(&self) -> Result<usize, crate::error::Error> {
        let mut res = 0_usize;
        let mut len = std::mem::size_of::<usize>();

        let err = unsafe { libfabric_sys::inlined_fi_getopt(self.as_raw_fid(), libfabric_sys::FI_OPT_ENDPOINT as i32, libfabric_sys::FI_OPT_BUFFERED_LIMIT as i32, (&mut res as *mut usize).cast(), &mut len)};
    
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(res)
        }
    }

    fn set_buffered_limit(&self, size: usize) -> Result<(), crate::error::Error> {
        let mut res = size;
        let mut len = std::mem::size_of::<usize>();

        let err = unsafe { libfabric_sys::inlined_fi_getopt(self.as_raw_fid(), libfabric_sys::FI_OPT_ENDPOINT as i32, libfabric_sys::FI_OPT_BUFFERED_LIMIT as i32, (&mut res as *mut usize).cast(), &mut len)};
    
        check_error(err.try_into().unwrap())
    }

    fn buffered_min(&self) -> Result<usize, crate::error::Error> {
        let mut res = 0_usize;
        let mut len = std::mem::size_of::<usize>();

        let err = unsafe { libfabric_sys::inlined_fi_getopt(self.as_raw_fid(), libfabric_sys::FI_OPT_ENDPOINT as i32, libfabric_sys::FI_OPT_BUFFERED_MIN as i32, (&mut res as *mut usize).cast(), &mut len)};
    
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(res)
        }
    }

    fn set_buffered_min(&self, size: usize) -> Result<(), crate::error::Error> {
        let mut res = size;
        let mut len = std::mem::size_of::<usize>();

        let err = unsafe { libfabric_sys::inlined_fi_getopt(self.as_raw_fid(), libfabric_sys::FI_OPT_ENDPOINT as i32, libfabric_sys::FI_OPT_BUFFERED_MIN as i32, (&mut res as *mut usize).cast(), &mut len)};
    
        check_error(err.try_into().unwrap())
    }

    fn cm_data_size(&self) -> Result<usize, crate::error::Error> {
        let mut res = 0_usize;
        let mut len = std::mem::size_of::<usize>();

        let err = unsafe { libfabric_sys::inlined_fi_getopt(self.as_raw_fid(), libfabric_sys::FI_OPT_ENDPOINT as i32, libfabric_sys::FI_OPT_CM_DATA_SIZE as i32, (&mut res as *mut usize).cast(), &mut len)};
    
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(res)
        }
    }

    fn set_cm_data_size(&self, size: usize) -> Result<(), crate::error::Error> {
        let mut res = size;
        let mut len = std::mem::size_of::<usize>();

        let err = unsafe { libfabric_sys::inlined_fi_getopt(self.as_raw_fid(), libfabric_sys::FI_OPT_ENDPOINT as i32, libfabric_sys::FI_OPT_CM_DATA_SIZE as i32, (&mut res as *mut usize).cast(), &mut len)};
    
        check_error(err.try_into().unwrap())
    }

    fn min_multi_recv(&self) -> Result<usize, crate::error::Error> {
        let mut res = 0_usize;
        let mut len = std::mem::size_of::<usize>();

        let err = unsafe { libfabric_sys::inlined_fi_getopt(self.as_raw_fid(), libfabric_sys::FI_OPT_ENDPOINT as i32, libfabric_sys::FI_OPT_MIN_MULTI_RECV as i32, (&mut res as *mut usize).cast(), &mut len)};
    
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(res)
        }
    }

    fn set_min_multi_recv(&self, size: usize) -> Result<(), crate::error::Error> {
        let mut res = size;
        let mut len = std::mem::size_of::<usize>();

        let err = unsafe { libfabric_sys::inlined_fi_getopt(self.as_raw_fid(), libfabric_sys::FI_OPT_ENDPOINT as i32, libfabric_sys::FI_OPT_MIN_MULTI_RECV as i32, (&mut res as *mut usize).cast(), &mut len)};
    
        check_error(err.try_into().unwrap())
    }

    fn hmem_p2p(&self) -> Result<HmemP2p, crate::error::Error> {
        let mut res = 0_u32;
        let mut len = std::mem::size_of::<u32>();

        let err = unsafe { libfabric_sys::inlined_fi_getopt(self.as_raw_fid(), libfabric_sys::FI_OPT_ENDPOINT as i32, libfabric_sys::FI_OPT_FI_HMEM_P2P as i32, (&mut res as *mut u32).cast(), &mut len)};
    
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(HmemP2p::from_value(res))
        }
    }

    fn xpu_trigger(&self) -> Result<libfabric_sys::fi_trigger_xpu, crate::error::Error> {
        let mut res = libfabric_sys::fi_trigger_xpu {
            count: 0,
            iface: 0,
            device: libfabric_sys::fi_trigger_xpu__bindgen_ty_1 {
                reserved: 0,
            },
            var: std::ptr::null_mut(),
        };
        let mut len = std::mem::size_of::<libfabric_sys::fi_trigger_xpu>();

        let err = unsafe { libfabric_sys::inlined_fi_getopt(self.as_raw_fid(), libfabric_sys::FI_OPT_ENDPOINT as i32, libfabric_sys::FI_OPT_XPU_TRIGGER as i32, (&mut res as *mut libfabric_sys::fi_trigger_xpu).cast(), &mut len)};
    
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(res)
        }
    }

    fn set_hmem_p2p(&self, hmem: HmemP2p) -> Result<(), crate::error::Error> {

        let mut len = std::mem::size_of::<u32>();

        let err = unsafe { libfabric_sys::inlined_fi_getopt(self.as_raw_fid(), libfabric_sys::FI_OPT_ENDPOINT as i32, libfabric_sys::FI_OPT_FI_HMEM_P2P as i32, (&mut hmem.get_value() as *mut u32).cast(), &mut len)};
    
        check_error(err.try_into().unwrap())
    }

    fn set_cuda_api_permitted(&self, permitted: bool) -> Result<(), crate::error::Error> {
    
        let mut val = if permitted {1_u32} else {0_u32}; 
        let mut len = std::mem::size_of::<u32>();

        let err = unsafe { libfabric_sys::inlined_fi_getopt(self.as_raw_fid(), libfabric_sys::FI_OPT_ENDPOINT as i32, libfabric_sys::FI_OPT_FI_HMEM_P2P as i32, (&mut val as *mut u32).cast(), &mut len)};
    
        check_error(err.try_into().unwrap())
    }

    fn wait_fd(&self) -> Result<BorrowedFd, crate::error::Error> {
        let mut fd = 0;

        let err = unsafe{ libfabric_sys::inlined_fi_control(self.as_raw_fid(), fi_wait_obj_FI_WAIT_FD as i32, (&mut fd as *mut i32).cast())};
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(unsafe{BorrowedFd::borrow_raw(fd)})
        }
    }
}


impl<T, EQ: ?Sized + ReadEq, CQ: ?Sized+ ReadCq> BaseEndpoint for EndpointImplBase<T, EQ, CQ> {}
impl<T: BaseEndpoint> BaseEndpoint for EndpointBase<T> {}

impl<T, EQ: ?Sized + ReadEq, CQ: ?Sized+ ReadCq> ActiveEndpoint for EndpointImplBase<T, EQ, CQ> {}

impl<T:ActiveEndpoint> ActiveEndpoint for EndpointBase<T> {}

impl<E: AsFd> AsFd for EndpointBase<E> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inner.as_fd()
    }
}

impl<T, EQ: ?Sized + ReadEq, CQ: ?Sized + ReadCq> AsFd for EndpointImplBase<T, EQ, CQ> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.wait_fd().unwrap()
    }
}

//================== Scalable Endpoint (fi_scalable_ep) ==================//
pub(crate) struct ScalableEndpointImpl {
    pub(crate) c_sep: OwnedEpFid,
    _domain_rc:  Rc<dyn DomainImplT>,
}

pub struct ScalableEndpoint<E> {
    inner: Rc<ScalableEndpointImpl>,
    phantom: PhantomData<E>,
}

impl ScalableEndpoint<()> {
    pub fn new<T0, E, EQ: ?Sized + 'static>(domain: &crate::domain::DomainBase<EQ>, info: &InfoEntry<E>, context: Option<&mut T0>) -> Result<ScalableEndpoint<E>, crate::error::Error> {
        Ok(
            ScalableEndpoint::<E> { 
                inner: Rc::new( ScalableEndpointImpl::new(&domain.inner, info, context)?),
                phantom: PhantomData,
            })
    }
}

impl ScalableEndpointImpl {

    pub fn new<T0, E, EQ: ?Sized + 'static>(domain: &Rc<crate::domain::DomainImplBase<EQ>>, info: &InfoEntry<E>, context: Option<&mut T0>) -> Result<ScalableEndpointImpl, crate::error::Error> {
        let mut c_sep: EpRawFid = std::ptr::null_mut();
        let err = 
            if let Some(ctx) = context {
                unsafe { libfabric_sys::inlined_fi_scalable_ep(domain.as_raw_typed_fid(), info.c_info, &mut c_sep, (ctx as *mut T0).cast()) }
            }
            else {
                unsafe { libfabric_sys::inlined_fi_scalable_ep(domain.as_raw_typed_fid(), info.c_info, &mut c_sep, std::ptr::null_mut()) }
            };
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            
            Ok(
                ScalableEndpointImpl { 
                    c_sep: OwnedEpFid::from(c_sep),
                    _domain_rc: domain.clone(), 
                })
        }
    }
    fn bind<T: crate::fid::AsFid + ?Sized>(&self, res: &T, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_scalable_ep_bind(self.as_raw_typed_fid(), res.as_fid().as_raw_fid(), flags) };
        
        check_error(err.try_into().unwrap())
    }

    // pub(crate) fn bind_av(&self, av: &Rc<AddressVectorImpl>) -> Result<(), crate::error::Error> {
    
    //     self.bind(&av, 0)
    // }

    pub(crate) fn alias(&self, flags: u64) -> Result<ScalableEndpointImpl, crate::error::Error> {
        let mut c_sep: EpRawFid = std::ptr::null_mut();
        let c_sep_ptr: *mut EpRawFid = &mut c_sep;
        let err = unsafe { libfabric_sys::inlined_fi_ep_alias(self.as_raw_typed_fid(), c_sep_ptr, flags) };
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(
                ScalableEndpointImpl { 
                    c_sep: OwnedEpFid::from(c_sep),
                    _domain_rc: self._domain_rc.clone(), 
                })
        }
    }
}

impl<E> ScalableEndpoint<E> {

    pub fn bind_av(&self, av: &AddressVector) -> Result<(), crate::error::Error> {
        self.inner.bind(&av.inner, 0)
    }

    pub fn alias(&self, flags: u64) -> Result<ScalableEndpoint<E>, crate::error::Error> {
        Ok(
            Self {
                inner: Rc::new(self.inner.alias(flags)?),
                phantom: PhantomData,
            }
        )
    }    
}


impl AsFid for ScalableEndpointImpl {
    fn as_fid(&self) -> fid::BorrowedFid<'_> {
        self.c_sep.as_fid()
    }
}
impl<E> AsFid for ScalableEndpoint<E> {
    fn as_fid(&self) -> fid::BorrowedFid<'_> {
        self.inner.as_fid()
    }
}

impl AsRawFid for ScalableEndpointImpl {
    fn as_raw_fid(&self) -> RawFid {
        self.c_sep.as_raw_fid()
    }
}
impl<E> AsRawFid for ScalableEndpoint<E> {
    fn as_raw_fid(&self) -> RawFid {
        self.inner.as_raw_fid()
    }
}

impl AsTypedFid<EpRawFid> for ScalableEndpointImpl {
    fn as_typed_fid(&self) -> fid::BorrowedTypedFid<EpRawFid> {
        self.c_sep.as_typed_fid()
    }
}
impl<E> AsTypedFid<EpRawFid> for ScalableEndpoint<E> {
    fn as_typed_fid(&self) -> fid::BorrowedTypedFid<EpRawFid> {
        self.inner.as_typed_fid()
    }
}

impl AsRawTypedFid for ScalableEndpointImpl {
    type Output = EpRawFid;
    
    fn as_raw_typed_fid(&self) -> Self::Output {
        self.c_sep.as_raw_typed_fid()
    }
}

impl<E> AsRawTypedFid for ScalableEndpoint<E> {
    type Output = EpRawFid;
    
    fn as_raw_typed_fid(&self) -> Self::Output {
       self.inner.as_raw_typed_fid()
    }
}

impl<E> BaseEndpoint for ScalableEndpoint<E> {}


impl BaseEndpoint for ScalableEndpointImpl {}


impl ActiveEndpoint for ScalableEndpointImpl {}

impl<E> ActiveEndpoint for ScalableEndpoint<E> {}

impl<E> AsFd for ScalableEndpoint<E> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.wait_fd().unwrap()
    }
}

//================== Passive Endpoint (fi_passive_ep) ==================//

pub(crate) struct PassiveEndpointImplBase<E, EQ: ?Sized> {
    pub(crate) c_pep: OwnedPepFid,
    _sync_rcs: RefCell<Vec<Rc<dyn crate::AsRawFid>>>,
    pub(crate) eq: OnceCell<Rc<EQ>>,
    phantom: PhantomData<E>,
    _fabric_rc: Rc<FabricImpl>,
}

pub type PassiveEndpoint<E>  = PassiveEndpointBase<E, dyn ReadEq>;

pub struct PassiveEndpointBase<E, EQ: ?Sized> {
    pub(crate) inner: Rc<PassiveEndpointImplBase<E, EQ>>,
}

impl<EQ: ?Sized> PassiveEndpointBase<(), EQ> {
    pub fn new<T0, E>(fabric: &crate::fabric::Fabric, info: &InfoEntry<E>, context: Option<&mut T0>) -> Result<PassiveEndpointBase<E, EQ>, crate::error::Error> {
        Ok(
            PassiveEndpointBase::<E, EQ> {
                inner: 
                    Rc::new(PassiveEndpointImplBase::new(&fabric.inner, info, context)?)
            }
        )
    }
}

impl<EQ: ?Sized> PassiveEndpointImplBase<(), EQ> {

    pub fn new<T0, E>(fabric: &Rc<crate::fabric::FabricImpl>, info: &InfoEntry<E>, context: Option<&mut T0>) -> Result<PassiveEndpointImplBase<E, EQ>, crate::error::Error> {
        let mut c_pep: PepRawFid = std::ptr::null_mut();
        let err = 
            if let Some(ctx) = context {
                unsafe { libfabric_sys::inlined_fi_passive_ep(fabric.as_raw_typed_fid(), info.c_info, &mut  c_pep, (ctx as *mut T0).cast()) }
            }
            else {
                unsafe { libfabric_sys::inlined_fi_passive_ep(fabric.as_raw_typed_fid(), info.c_info, &mut  c_pep, std::ptr::null_mut()) }
            };
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(
                PassiveEndpointImplBase::<E, EQ> { 
                    c_pep: OwnedPepFid::from(c_pep),
                    eq: OnceCell::new(),
                    _sync_rcs: RefCell::new(Vec::new()),
                    _fabric_rc: fabric.clone(),
                    phantom: PhantomData,
                })
        }
    }
}


impl<E> PassiveEndpointImplBase<E, dyn ReadEq> {


    pub(crate) fn bind<T: ReadEq + 'static>(&self, res: &Rc<T>, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_pep_bind(self.as_raw_typed_fid(), res.as_raw_fid(), flags) };
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            // self._sync_rcs.borrow_mut().push(res.clone()); 
            if self.eq.set(res.clone()).is_err() {panic!("Could not set oncecell")}
            Ok(())
        }
    }
}


impl<E> PassiveEndpointBase<E, dyn ReadEq> {

    pub fn bind<T: ReadEq + 'static>(&self, res: &EventQueueBase<T>, flags: u64) -> Result<(), crate::error::Error> {
        self.inner.bind(&res.inner, flags)
    }
}

impl<E, EQ: ?Sized + ReadEq> PassiveEndpointImplBase<E, EQ> {

    pub fn listen(&self) -> Result<(), crate::error::Error> {
        let err = unsafe {libfabric_sys::inlined_fi_listen(self.as_raw_typed_fid())};
        
        check_error(err.try_into().unwrap())
    }

    pub fn reject<T0>(&self, fid: &impl AsFid, params: &[T0]) -> Result<(), crate::error::Error> {
        let err = unsafe {libfabric_sys::inlined_fi_reject(self.as_raw_typed_fid(), fid.as_fid().as_raw_fid(), params.as_ptr().cast(), params.len())};

        check_error(err.try_into().unwrap())

    }

    pub fn set_backlog_size(&self, size: i32) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_control(self.as_fid().as_raw_fid(), FI_BACKLOG as i32, (&mut size.clone() as *mut i32).cast())};
        check_error(err.try_into().unwrap())
    }
}


impl<E, EQ: ?Sized + ReadEq> PassiveEndpointBase<E, EQ> {

    pub fn listen(&self) -> Result<(), crate::error::Error> {
        self.inner.listen()
    }

    pub fn reject<T0>(&self, fid: &impl AsFid, params: &[T0]) -> Result<(), crate::error::Error> {
        self.inner.reject(fid, params)
    }

    pub fn set_backlog_size(&self, size: i32) -> Result<(), crate::error::Error> {
        self.inner.set_backlog_size(size)
    }
}

impl<E, EQ: ?Sized> BaseEndpoint for PassiveEndpointBase<E, EQ> {}

impl<E, EQ: ?Sized + ReadEq> BaseEndpoint for PassiveEndpointImplBase<E, EQ> {}

impl<E, EQ: ?Sized + ReadEq> AsFid for PassiveEndpointImplBase<E, EQ> {
    fn as_fid(&self) -> fid::BorrowedFid {
        self.c_pep.as_fid()
    }    
}

impl<E, EQ: ?Sized + ReadEq> AsFid for PassiveEndpointBase<E, EQ> {
    fn as_fid(&self) -> fid::BorrowedFid {
        self.inner.as_fid()
    }    
}

impl<E, EQ: ?Sized> AsRawFid for PassiveEndpointImplBase<E, EQ> {
    fn as_raw_fid(&self) -> RawFid {
        self.c_pep.as_raw_fid()
    }    
}

impl<E, EQ: ?Sized> AsRawFid for PassiveEndpointBase<E, EQ> {
    fn as_raw_fid(&self) -> RawFid {
        self.inner.as_raw_fid()
    }    
}

impl<E, EQ: ?Sized + ReadEq> AsTypedFid<PepRawFid> for PassiveEndpointImplBase<E, EQ> {
    fn as_typed_fid(&self) -> fid::BorrowedTypedFid<PepRawFid> {
        self.c_pep.as_typed_fid()
    }    
}

impl<E, EQ: ?Sized + ReadEq> AsTypedFid<PepRawFid> for PassiveEndpointBase<E, EQ> {
    fn as_typed_fid(&self) -> fid::BorrowedTypedFid<PepRawFid> {
        self.inner.as_typed_fid()
    }
}

impl<E, EQ: ?Sized + ReadEq> AsRawTypedFid for PassiveEndpointImplBase<E, EQ> {
    type Output = PepRawFid;
    
    fn as_raw_typed_fid(&self) -> Self::Output {
        self.c_pep.as_raw_typed_fid()
    }    
}

impl<E, EQ: ?Sized + ReadEq> AsRawTypedFid for PassiveEndpointBase<E, EQ> {
    type Output = PepRawFid;
    
    fn as_raw_typed_fid(&self) -> Self::Output {
        self.inner.as_raw_typed_fid()
    }    
}

impl<E, EQ: ?Sized + ReadEq> AsFd for PassiveEndpointImplBase<E, EQ> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.wait_fd().unwrap()
    }
}

impl<E> AsFd for PassiveEndpoint<E> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.wait_fd().unwrap()
    }
}

//================== Endpoint (fi_endpoint) ==================//

pub struct IncompleteBindCq<'a, EP> {
    pub(crate) ep: &'a EP,
    pub(crate) flags: u64,
}

impl<'a, EP> IncompleteBindCq<'a, EP> {
    pub fn recv(&mut self, selective: bool) -> &mut Self {
        if selective {
            self.flags |= libfabric_sys::FI_SELECTIVE_COMPLETION | libfabric_sys::FI_RECV  as u64 ;
        
            self
        }
        else {
            self.flags |= libfabric_sys::FI_RECV as u64;

            self
        }
    }

    pub fn transmit(&mut self, selective: bool) -> &mut Self {
        if selective {
            self.flags |= libfabric_sys::FI_SELECTIVE_COMPLETION | libfabric_sys::FI_TRANSMIT as u64;

            self
        }
        else {
            self.flags |= libfabric_sys::FI_TRANSMIT as u64;

            self
        }
    }
}

impl<'a, EP> IncompleteBindCq<'a, EndpointImplBase<EP, dyn ReadEq, dyn ReadCq>> {

    pub fn cq<T: AsRawFid+ ReadCq + 'static>(&mut self, cq: &crate::cq::CompletionQueueBase<T>) -> Result<(), crate::error::Error> {
        self.ep.bind_cq_(&cq.inner, self.flags)
    }
}

pub struct IncompleteBindCntr<'a, EP, EQ: ?Sized + ReadEq, CQ: ?Sized + ReadCq> {
    pub(crate) ep: &'a EndpointImplBase<EP, EQ, CQ>,
    pub(crate) flags: u64,
}

impl<'a, EP, EQ: ?Sized + ReadEq + AsRawFid + 'static, CQ: ?Sized + ReadCq> IncompleteBindCntr<'a, EP, EQ, CQ> {

    pub fn read(&mut self) -> &mut Self {
        self.flags |= libfabric_sys::FI_READ as u64;

        self
    }

    pub fn recv(&mut self) -> &mut Self {
        self.flags |= libfabric_sys::FI_RECV as u64;

        self
    }

    pub fn remote_read(&mut self) -> &mut Self {
        self.flags |= libfabric_sys::FI_REMOTE_READ as u64;

        self
    }

    pub fn remote_write(&mut self) -> &mut Self {
        self.flags |= libfabric_sys::FI_REMOTE_WRITE as u64;

        self
    }

    pub fn send(&mut self) -> &mut Self {
        self.flags |= libfabric_sys::FI_SEND as u64;

        self
    }

    pub fn write(&mut self) -> &mut Self {
        self.flags |= libfabric_sys::FI_WRITE as u64;

        self
    }

    pub fn cntr(&mut self, cntr: &Counter<impl ReadCntr + 'static>) -> Result<(), crate::error::Error> {
        self.ep.bind_cntr_(&cntr.inner, self.flags)
    }
}

impl<T, EQ: ?Sized + ReadEq, CQ: ?Sized + ReadCq> EndpointImplBase<T, EQ, CQ> {

    pub(crate) fn new<T0, E, DEQ: ?Sized + 'static>(domain: &Rc<crate::domain::DomainImplBase<DEQ>>, info: &InfoEntry<E>, flags: u64, context: Option<&mut T0>) -> Result< Self, crate::error::Error> {
        let mut c_ep: EpRawFid = std::ptr::null_mut();
        let err =
            if let Some(ctx) = context {
                unsafe { libfabric_sys::inlined_fi_endpoint2(domain.as_raw_typed_fid(), info.c_info, &mut c_ep, flags, (ctx as *mut T0).cast()) }
            } 
            else {
                unsafe { libfabric_sys::inlined_fi_endpoint2(domain.as_raw_typed_fid(), info.c_info, &mut c_ep, flags, std::ptr::null_mut()) }
            };

        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(
                Self { 
                    c_ep: OwnedEpFid::from(c_ep),
                    _sync_rcs: RefCell::new(Vec::new()),
                    rx_cq: OnceCell::new(),
                    tx_cq: OnceCell::new(),
                    eq: OnceCell::new(),
                    _domain_rc: domain.clone(),
                    phantom: PhantomData,
                })
        }
    }
}

impl Endpoint<()> {
    pub fn new<T0, E, DEQ:?Sized + 'static >(domain: &crate::domain::DomainBase<DEQ>, info: &InfoEntry<E>, flags: u64, context: Option<&mut T0>) -> Result< Endpoint<E>, crate::error::Error> {
        Ok(
            EndpointBase::<EndpointImplBase<E, dyn ReadEq, dyn ReadCq>> {
                inner:Rc::new(EndpointImplBase::new(&domain.inner, info, flags, context)?),
            }
        )
    }
}

// pub(crate) fn from_attr(domain: &crate::domain::Domain, mut rx_attr: crate::RxAttr) -> Result<Self, crate::error::Error> {
    //     let mut c_ep: EpRawFid = std::ptr::null_mut();
    //     let c_ep_ptr: *mut EpRawFid = &mut c_ep;
    //     let err = unsafe { libfabric_sys::inlined_fi_srx_context(domain.handle(), rx_attr.get_mut(), c_ep_ptr,  std::ptr::null_mut()) };

    //     if err != 0 {
    //         Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
    //     }
    //     else {
    //         Ok(
    //             Self { c_ep, fid: OwnedFid { fid: unsafe{ &mut (*c_ep).fid } } }        
    //         )
    //     }

    // }

    // pub(crate) fn from_attr_with_context<T0>(domain: &crate::domain::Domain, mut rx_attr: crate::RxAttr, context: &mut T0) -> Result<Self, crate::error::Error> {
    //     let mut c_ep: EpRawFid = std::ptr::null_mut();
    //     let c_ep_ptr: *mut EpRawFid = &mut c_ep;
    //     let err = unsafe { libfabric_sys::inlined_fi_srx_context(domain.handle(), rx_attr.get_mut(), c_ep_ptr, context as *mut T0 as *mut std::ffi::c_void) };

    //     if err != 0 {
    //         Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
    //     }
    //     else {
    //         Ok(
    //             Self { c_ep, fid: OwnedFid { fid: unsafe{ &mut (*c_ep).fid } } }        
    //         )
    //     }

    // }
impl<EP, EQ: ?Sized + ReadEq + 'static, CQ: ?Sized + ReadCq> EndpointImplBase<EP, EQ, CQ> {
    
    pub(crate) fn bind<T: crate::Bind + AsRawFid>(&self, res: &T, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_ep_bind(self.as_raw_typed_fid(), res.as_raw_fid(), flags) };
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            self._sync_rcs.borrow_mut().push(res.inner().clone()); //  [TODO] Do this with inner mutability.
            Ok(())
        }
    } 

    pub(crate) fn bind_cntr_(&self, res: &Rc<impl ReadCntr + 'static>, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_ep_bind(self.as_raw_typed_fid(), res.as_raw_fid(), flags) };
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            self._sync_rcs.borrow_mut().push(res.clone()); //  [TODO] Do this with inner mutability.
            Ok(())
        }
    } 
}

impl<EP, EQ: ?Sized + ReadEq + 'static> EndpointImplBase<EP, EQ, dyn ReadCq> {
    pub(crate) fn bind_cq_<T: AsRawFid + ReadCq + 'static>(&self, cq: &Rc<T>, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_ep_bind(self.as_raw_typed_fid(), cq.as_raw_fid(), flags) };
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {

            if (flags & libfabric_sys::FI_TRANSMIT as u64) != 0 && (flags & libfabric_sys::FI_RECV as u64) != 0{
                if self.tx_cq.set(cq.clone()).is_err() {
                    panic!("Endpoint is already bound to a CompletionQueue for this direction");
                }
                if self.rx_cq.set(cq.clone()).is_err() {
                    panic!("Endpoint is already bound to a CompletionQueue for this direction");
                }
            }
            else if flags & libfabric_sys::FI_TRANSMIT as u64 != 0 {
                if self.tx_cq.set(cq.clone()).is_err() {
                    panic!("Endpoint is already bound to a CompletionQueue for this direction");
                }
            }
            else if flags & libfabric_sys::FI_RECV as u64 != 0{
                if self.rx_cq.set(cq.clone()).is_err() {
                    panic!("Endpoint is already bound to a CompletionQueue for this direction");
                }
            }
            else {
                panic!("Binding to Endpoint without specifying direction");
            }

            // self._sync_rcs.borrow_mut().push(cq.inner().clone()); //  [TODO] Do we need this for cq?
            Ok(())
        }
    } 

    pub(crate) fn bind_cq(&self) -> IncompleteBindCq<Self> {
        IncompleteBindCq { ep: self, flags: 0}
    }
}

impl<EP, CQ: ?Sized + ReadCq> EndpointImplBase<EP, dyn ReadEq, CQ> {

    pub(crate) fn bind_eq<T: ReadEq + 'static>(&self, eq: &Rc<T>) -> Result<(), crate::error::Error>  {
            
        let err = unsafe { libfabric_sys::inlined_fi_ep_bind(self.as_raw_typed_fid(), eq.as_raw_fid(), 0) };
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            if self.eq.set(eq.clone()).is_err() {
                panic!("Endpoint is already bound to another EventQueue"); // Should never reach this since inlined_fi_ep_bind will throw an error ealier
                                                                        // but keep it here to satisfy the compiler.
            }

            // self._sync_rcs.borrow_mut().push(cq.inner().clone()); //  [TODO] Do we need this for eq?
            Ok(())
        }
    }
}

impl<EP, EQ: ?Sized +  AsRawFid + 'static + ReadEq, CQ: ?Sized + ReadCq> EndpointImplBase<EP, EQ, CQ> {

    pub(crate) fn bind_cntr(&self) -> IncompleteBindCntr<EP, EQ, CQ> {
        IncompleteBindCntr { ep: self, flags: 0}
    }

    // pub(crate) fn bind_eq<T: EqConfig + 'static>(&self, eq: &EventQueue<T>) -> Result<(), crate::error::Error>  {
        
    //     self.bind_eq__eq_(&eq.inner, 0)
    // }



    pub(crate) fn bind_av<AVEQ: ?Sized + ReadEq + 'static>(&self, av: &AddressVectorBase<AVEQ>) -> Result<(), crate::error::Error> {
    
        self.bind(av, 0)
    }

    #[allow(dead_code)]
    pub(crate) fn alias(&self, flags: u64) -> Result<Self, crate::error::Error> {
        let mut c_ep: EpRawFid = std::ptr::null_mut();
        let err = unsafe { libfabric_sys::inlined_fi_ep_alias(self.as_raw_typed_fid(), &mut c_ep, flags) };
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(
                Self { 
                    c_ep: OwnedEpFid::from(c_ep),
                    _sync_rcs: RefCell::new(Vec::new()),
                    rx_cq: OnceCell::new(),
                    tx_cq: OnceCell::new(),
                    eq: OnceCell::new(),
                    _domain_rc: self._domain_rc.clone(),
                    phantom: PhantomData,
                })
        }
    }
}

impl<EP> EndpointBase<EndpointImplBase<EP, dyn ReadEq, dyn ReadCq>> {
    pub fn bind_cq(&self) -> IncompleteBindCq<EndpointImplBase<EP, dyn ReadEq, dyn ReadCq>> {
        self.inner.bind_cq()
    }
}


impl<EP> EndpointBase<EndpointImplBase<EP, dyn ReadEq, dyn ReadCq>> {
    pub fn bind_eq<T: ReadEq + 'static>(&self, eq: &EventQueueBase<T>) -> Result<(), crate::error::Error>  {
        self.inner.bind_eq(&eq.inner)
    }
}

impl<EP> EndpointBase<EndpointImplBase<EP, dyn ReadEq, dyn ReadCq>> {
    pub fn bind_cntr(&self) -> IncompleteBindCntr<EP, dyn ReadEq, dyn ReadCq> {
        self.inner.bind_cntr()
    }

    pub fn bind_av<EQ: ?Sized + ReadEq + 'static>(&self, av: &AddressVectorBase<EQ>) -> Result<(), crate::error::Error> {
        self.inner.bind_av(av)
    }

    // pub fn alias(&self, flags: u64) -> Result<Self, crate::error::Error> {
    //     Ok(
    //         Self {
    //             inner: Rc::new (self.inner.alias(flags)?),
    //         }
    //     )
    // }
}


impl<E: AsFid> AsFid for EndpointBase<E> {
    fn as_fid(&self) -> fid::BorrowedFid<'_> {
        self.inner.as_fid()
    }
}

impl<E: AsRawFid> AsRawFid for EndpointBase<E> {
    fn as_raw_fid(&self) -> RawFid {
        self.inner.as_raw_fid()
    }
}

impl<E: AsTypedFid<EpRawFid>> AsTypedFid<EpRawFid> for EndpointBase<E> {
    fn as_typed_fid(&self) -> fid::BorrowedTypedFid<EpRawFid> {
        self.inner.as_typed_fid()
    }
}

impl<E: AsRawTypedFid<Output = *mut libfabric_sys::fid_ep>> AsRawTypedFid for EndpointBase<E> {
    type Output = EpRawFid;

    fn as_raw_typed_fid(&self) -> Self::Output {
        self.inner.as_raw_typed_fid()
    }
}

impl<EP, EQ: ?Sized + ReadEq, CQ: ?Sized + ReadCq> AsFid for EndpointImplBase<EP, EQ, CQ> {
    fn as_fid(&self) -> fid::BorrowedFid<'_> {
        self.c_ep.as_fid()
    }
}

impl<EP, EQ: ?Sized + ReadEq, CQ: ?Sized + ReadCq> AsRawFid for EndpointImplBase<EP, EQ, CQ> {
    fn as_raw_fid(&self) -> RawFid {
        self.c_ep.as_raw_fid()
    }
}

impl<EP, EQ: ?Sized + ReadEq, CQ: ?Sized + ReadCq> AsTypedFid<EpRawFid> for EndpointImplBase<EP, EQ, CQ> {
    fn as_typed_fid(&self) -> fid::BorrowedTypedFid<EpRawFid> {
        self.c_ep.as_typed_fid()
    }
}

impl<EP, EQ: ?Sized, CQ: ?Sized + ReadCq> AsRawTypedFid for EndpointImplBase<EP, EQ, CQ> {
    type Output = EpRawFid;
    
    fn as_raw_typed_fid(&self) -> Self::Output {
        self.c_ep.as_raw_typed_fid()
    }
}

// impl<EP, EQ: ?Sized + ReadEq, CQ: ?Sized + ReadCq> ActiveEndpointImpl for EndpointImplBase<EP, EQ, CQ> {}

pub trait ActiveEndpoint: AsRawTypedFid<Output = EpRawFid>{

    fn enable(&self) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_enable(self.as_raw_typed_fid()) };
        check_error(err.try_into().unwrap())
    }

    fn cancel(&self) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_cancel(self.as_raw_typed_fid().as_raw_fid(), std::ptr::null_mut()) };
        check_error(err)
    }

    fn cancel_with_context<T0>(&self, context: &mut T0) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_cancel(self.as_raw_typed_fid().as_raw_fid(), (context as *mut T0).cast()) };
        check_error(err)
    }

    fn rx_size_left(&self) -> Result<usize, crate::error::Error> {
        let ret = unsafe {libfabric_sys::inlined_fi_rx_size_left(self.as_raw_typed_fid())};

        if ret < 0 {
            Err(crate::error::Error::from_err_code((-ret).try_into().unwrap()))
        }
        else {
            Ok(ret as usize)
        }
    }

    fn tx_size_left(&self) -> Result<usize, crate::error::Error> {
        let ret = unsafe {libfabric_sys::inlined_fi_tx_size_left(self.as_raw_typed_fid())};

        if ret < 0 {
            Err(crate::error::Error::from_err_code((-ret).try_into().unwrap()))
        }
        else {
            Ok(ret as usize)
        }
    }

    fn getpeer(&self) -> Result<Address, crate::error::Error> {
        let mut len = 0;
        let err = unsafe { libfabric_sys::inlined_fi_getpeer(self.as_raw_typed_fid(), std::ptr::null_mut(), &mut len)};
        
        if -err as u32 ==  libfabric_sys::FI_ETOOSMALL{
            let mut address = vec![0; len];
            let err = unsafe { libfabric_sys::inlined_fi_getpeer(self.as_raw_typed_fid(), address.as_mut_ptr().cast(), &mut len)};
            if err != 0 {
                Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
            }
            else {
                Ok(Address{address})
            }
        }
        else {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
    }

    fn connect_with<T>(&self, addr: &Address, param: &[T]) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_connect(self.as_raw_typed_fid(), addr.as_bytes().as_ptr().cast(), param.as_ptr().cast(), param.len()) };
        
        check_error(err.try_into().unwrap())
    }

    fn connect(&self, addr: &Address) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_connect(self.as_raw_typed_fid(), addr.as_bytes().as_ptr().cast(), std::ptr::null_mut(), 0) };

        check_error(err.try_into().unwrap())
    }

    fn accept_with<T0>(&self, param: &[T0]) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_accept(self.as_raw_typed_fid(), param.as_ptr().cast(), param.len()) };
        
        check_error(err.try_into().unwrap())
    }

    fn accept(&self) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_accept(self.as_raw_typed_fid(), std::ptr::null_mut(), 0) };
        
        check_error(err.try_into().unwrap())
    }

    fn shutdown(&self) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_shutdown(self.as_raw_typed_fid(), 0) };

        check_error(err.try_into().unwrap())
    }

    fn transmit_options(&self) -> Result<TransferOptions, crate::error::Error> {
        let mut ops = libfabric_sys::FI_TRANSMIT;
        let err = unsafe{ inlined_fi_control(self.as_raw_typed_fid().as_raw_fid(), FI_GETOPSFLAG as i32, (&mut ops as *mut u32).cast())}; 

        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(TransferOptions::from_value(ops))
        }
    }

    fn receive_options(&self) -> Result<TransferOptions, crate::error::Error> {
        let mut ops = libfabric_sys::FI_RECV;
        let err = unsafe{ inlined_fi_control(self.as_raw_typed_fid().as_raw_fid(), FI_GETOPSFLAG as i32, (&mut ops as *mut u32).cast())}; 

        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(TransferOptions::from_value(ops))
        }
    }

    fn set_transmit_options(&self, ops: TransferOptions) -> Result<(), crate::error::Error> {

        ops.transmit();
        let err = unsafe{ inlined_fi_control(self.as_raw_typed_fid().as_raw_fid(), libfabric_sys::FI_SETOPSFLAG as i32, (&mut ops.get_value() as *mut u32).cast())}; 

        check_error(err.try_into().unwrap())
    }

    fn set_receive_options(&self, ops: TransferOptions) -> Result<(), crate::error::Error> {
        
        ops.recv();
        let err = unsafe{ inlined_fi_control(self.as_raw_typed_fid().as_raw_fid(), libfabric_sys::FI_SETOPSFLAG as i32, (&mut ops.get_value() as *mut u32).cast())}; 

        check_error(err.try_into().unwrap())
    }
}
//================== Endpoint attribute ==================//
#[derive(Clone)]
pub struct EndpointAttr {
    c_attr: libfabric_sys::fi_ep_attr,
}

impl EndpointAttr {
    pub fn new() -> Self {
        let c_attr = libfabric_sys::fi_ep_attr{
            type_: libfabric_sys::fi_ep_type_FI_EP_UNSPEC,
            protocol: libfabric_sys::FI_PROTO_UNSPEC,
            protocol_version: 0,
            max_msg_size: 0,
            msg_prefix_size: 0,
            max_order_raw_size: 0,
            max_order_war_size: 0,
            max_order_waw_size: 0,
            mem_tag_format: 0,
            tx_ctx_cnt: 0,
            rx_ctx_cnt: 0,
            auth_key_size: 0,
            auth_key: std::ptr::null_mut(),
        };

        Self { c_attr }
    }

    pub(crate) fn from(c_ep_attr: *mut libfabric_sys::fi_ep_attr) -> Self {
        let c_attr = unsafe { *c_ep_attr };

        Self { c_attr }
    }

    pub fn ep_type(&mut self, type_: crate::enums::EndpointType) -> &mut Self {

        self.c_attr.type_ = type_.get_value();
        self
    }

    pub fn protocol(&mut self, proto: crate::enums::Protocol) -> &mut Self {

        self.c_attr.protocol = proto.get_value();
        self
    }

    pub fn max_msg_size(&mut self, size: usize) -> &mut Self {

        self.c_attr.max_msg_size = size;
        self
    }

    pub fn msg_prefix_size(&mut self, size: usize) -> &mut Self {

        self.c_attr.msg_prefix_size = size;
        self
    }

    pub fn max_order_raw_size(&mut self, size: usize) -> &mut Self {

        self.c_attr.max_order_raw_size = size;
        self
    }

    pub fn max_order_war_size(&mut self, size: usize) -> &mut Self {

        self.c_attr.max_order_war_size = size;
        self
    }

    pub fn max_order_waw_size(&mut self, size: usize) -> &mut Self {

        self.c_attr.max_order_waw_size = size;
        self
    }

    pub fn mem_tag_format(&mut self, tag: u64) -> &mut Self {

        self.c_attr.mem_tag_format = tag;
        self
    }

    pub fn tx_ctx_cnt(&mut self, size: usize) -> &mut Self {

        self.c_attr.tx_ctx_cnt = size;
        self
    }

    pub fn rx_ctx_cnt(&mut self, size: usize) -> &mut Self {

        self.c_attr.rx_ctx_cnt = size;
        self
    }

    pub fn auth_key(&mut self, key: &mut [u8]) -> &mut Self {

        self.c_attr.auth_key_size = key.len();
        self.c_attr.auth_key = key.as_mut_ptr();
        self
    }

    pub fn get_type(&self) -> crate::enums::EndpointType {
        crate::enums::EndpointType::from(self.c_attr.type_)
    }

    pub fn get_max_msg_size(&self) -> usize {
        self.c_attr.max_msg_size 
    }

    pub fn get_msg_prefix_size(&self) -> usize {
        self.c_attr.msg_prefix_size
    }

    pub(crate) fn get(&self) ->  *const libfabric_sys::fi_ep_attr {
        &self.c_attr
    }

    #[allow(dead_code)]
    pub(crate) fn get_mut(&mut self) ->  *mut libfabric_sys::fi_ep_attr {
        &mut self.c_attr
    }
}

impl Default for EndpointAttr {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EndpointBuilder<'a, T, E> {
    ep_attr: EndpointAttr,
    flags: u64,
    info: &'a InfoEntry<E>,
    ctx: Option<&'a mut T>,
}

impl<'a> EndpointBuilder<'a, (), ()> {

    pub fn new<E>(info: &'a InfoEntry<E>, ) -> EndpointBuilder<'a, (), E> {
        EndpointBuilder::<(), E> {
            ep_attr: EndpointAttr::new(),
            flags: 0,
            info,
            ctx: None,
        }
    }
}

impl<'a, E> EndpointBuilder<'a, (), E> {

    pub fn build<EQ: ?Sized + 'static>(self, domain: &crate::domain::DomainBase<EQ>) -> Result<Endpoint<E>, crate::error::Error> {
        Endpoint::new(domain, self.info, self.flags, self.ctx)
    }

    pub fn build_scalable<EQ: ?Sized + 'static>(self, domain: &crate::domain::DomainBase<EQ>) -> Result<ScalableEndpoint<E>, crate::error::Error> {
        ScalableEndpoint::new(domain, self.info, self.ctx)
    }

    pub fn build_passive(self, fabric: &crate::fabric::Fabric) -> Result<PassiveEndpoint<E>, crate::error::Error> {
        PassiveEndpoint::new(fabric, self.info, self.ctx)
    }

    // pub(crate) fn from(c_ep_attr: *mut libfabric_sys::fi_ep_attr) -> Self {
    //     let c_attr = unsafe { *c_ep_attr };

    //     Self { c_attr }
    // }

    pub fn flags(mut self, flags: u64) -> Self {
        self.flags = flags;
        self
    }

    pub fn ep_type(mut self, type_: crate::enums::EndpointType) -> Self {

        self.ep_attr.ep_type(type_);
        self
    }

    pub fn protocol(mut self, proto: crate::enums::Protocol) -> Self{
        
        self.ep_attr.protocol(proto);
        self
    }

    pub fn max_msg_size(mut self, size: usize) -> Self {

        self.ep_attr.max_msg_size(size);
        self
    }

    pub fn msg_prefix_size(mut self, size: usize) -> Self {

        self.ep_attr.msg_prefix_size(size);
        self
    }

    pub fn max_order_raw_size(mut self, size: usize) -> Self {

        self.ep_attr.max_order_raw_size(size);
        self
    }

    pub fn max_order_war_size(mut self, size: usize) -> Self {

        self.ep_attr.max_order_war_size(size);
        self
    }

    pub fn max_order_waw_size(mut self, size: usize) -> Self {

        self.ep_attr.max_order_waw_size(size);
        self
    }

    pub fn mem_tag_format(mut self, tag: u64) -> Self {

        self.ep_attr.mem_tag_format(tag);
        self
    }

    pub fn tx_ctx_cnt(mut self, size: usize) -> Self {

        self.ep_attr.tx_ctx_cnt(size);
        self
    }

    pub fn rx_ctx_cnt(mut self, size: usize) -> Self {

        self.ep_attr.rx_ctx_cnt(size);
        self
    }

    pub fn auth_key(mut self, key: &mut [u8]) -> Self {

        self.ep_attr.auth_key(key);
        self
    }
}




