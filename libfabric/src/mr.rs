use crate::{cntr::ReadCntr, cq::ReadCq, domain::DomainImplT, enums::{MrAccess, MrMode, MrRegOpt}, eq::ReadEq, fid::{self, AsRawFid, AsRawTypedFid, AsTypedFid, MrRawFid, OwnedMrFid, RawFid}, iovec::IoVec, utils::check_error, MyOnceCell, MyRc};
#[allow(unused_imports)]
use crate::fid::AsFid;


/// Represents a key needed to access a remote [MemoryRegion].
/// 
/// This enum encapsulates either a  'regular' key obtained from `fi_mr_key` or a 'raw' key obtained from `fir_mr_raw_attr`,
/// depending on the requirements of the provider.
pub enum MemoryRegionKey {
    Key(u64),
    RawKey((Vec<u8>, u64)),
}

impl MemoryRegionKey {

    // pub unsafe fn from_raw_parts(raw: *const u8, len: usize) -> Self {
    //     let mut raw_key = vec![0u8; len];
    //     raw_key.copy_from_slice(std::slice::from_raw_parts(raw, len));
    //     Self::RawKey(raw_key)
    // }

    /// Construct a new [MemoryRegionKey] from a slice of bytes, usually received
    /// from remote node using raw keys.
    ///
    /// # Safety
    /// This function is unsafe since there is not guarantee that the bytes read indeed represent
    /// a key
    ///
    pub unsafe fn from_bytes<EQ: ?Sized>(raw: &[u8], domain: &crate::domain::DomainBase<EQ>) -> Self {
        MemoryRegionKey::from_bytes_impl(raw, &*domain.inner)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            MemoryRegionKey::Key(key) => {
                let mut bytes = vec![0; std::mem::size_of::<u64>()];
                unsafe{bytes.copy_from_slice(std::slice::from_raw_parts(key as *const u64 as *const u8,  std::mem::size_of::<u64>()))};
                bytes
            }
            MemoryRegionKey::RawKey(key) => {
                [&key.0[..], unsafe{std::slice::from_raw_parts(&key.1 as *const u64 as *const u8,  std::mem::size_of::<u64>()) }].concat()
            }

        }
    }

    unsafe fn from_bytes_impl(raw: &[u8], domain: &(impl DomainImplT + ?Sized)) -> Self {
        if domain.get_mr_mode().is_raw() {
            assert!(raw.len() == domain.get_mr_key_size());
            let base_addr = *(raw[raw.len()-std::mem::size_of::<u64>()..].as_ptr() as *const u64);
            Self::RawKey((raw[0..raw.len()-std::mem::size_of::<u64>()].to_vec(), base_addr))
        }
        else {
            let mut key = 0u64;
            unsafe {std::slice::from_raw_parts_mut(&mut key as *mut u64 as * mut u8, 8).copy_from_slice(raw)};
            Self::Key(key)
        }
    }

    /// Construct a new [MemoryRegionKey] from a u64 value as received from the a remote node.
    ///
    /// # Safety
    /// This function is unsafe since there is not guarantee that the u64 value read represents
    /// a valid key.
    ///
    pub unsafe fn from_u64(key: u64) -> Self {
        MemoryRegionKey::Key(key) 
    }

    pub fn into_mapped<EQ: ?Sized + 'static>(mut self, domain: &crate::domain::DomainBase<EQ>) -> Result<MappedMemoryRegionKey, crate::error::Error> {
        match self {
            MemoryRegionKey::Key(mapped_key) => {
                Ok(MappedMemoryRegionKey{inner: MappedMemoryRegionKeyImpl::Key(mapped_key)})
            }
            MemoryRegionKey::RawKey(_) => {
                let mapped_key = domain.map_raw( &mut self, 0)?;
                Ok(MappedMemoryRegionKey{inner: MappedMemoryRegionKeyImpl::MappedRawKey((mapped_key, domain.inner.clone()))})
            }
        }
    }
}

#[derive(Clone)]
enum MappedMemoryRegionKeyImpl {
    Key(u64),
    MappedRawKey((u64, MyRc<dyn DomainImplT  >)),
}

/// Uniformly represents a (mapped if raw) memory region  key that can be used to
/// access remote [MemoryRegion]s. This struct will automatically unmap the key
/// if needed when it is dropped.
#[derive(Clone)]
pub struct MappedMemoryRegionKey {
    inner: MappedMemoryRegionKeyImpl,
}

impl MappedMemoryRegionKey {
    pub(crate) fn get_key(&self) -> u64 {
        match self.inner {
            MappedMemoryRegionKeyImpl::Key(key) | MappedMemoryRegionKeyImpl::MappedRawKey((key, _)) => key 
        }
    }
}

impl Drop for MappedMemoryRegionKey {
    fn drop(&mut self) {
        match self.inner {
            MappedMemoryRegionKeyImpl::Key(_) => {}
            MappedMemoryRegionKeyImpl::MappedRawKey((key, ref domain_impl)) => { 
                domain_impl.unmap_key(key).unwrap();
            }
        }
    }
}


pub trait DataDescriptor {
    fn get_desc(&mut self) -> *mut std::ffi::c_void;
    fn get_desc_ptr(&mut self) -> *mut *mut std::ffi::c_void;
}

pub fn default_desc() -> MemoryRegionDesc { MemoryRegionDesc { c_desc: std::ptr::null_mut() }}

// impl DataDescriptor for DefaultMemDesc {
//     fn get_desc(&mut self) -> *mut std::ffi::c_void {
//         std::ptr::null_mut()
//     }
    
//     fn get_desc_ptr(&mut self) -> *mut *mut std::ffi::c_void {
//         std::ptr::null_mut()
//     }
// }

// impl Drop for MemoryRegion {
//     fn drop(&mut self) {
//        println!("Dropping MemoryRegion\n");
//     }
// }

//================== Memory Region (fi_mr) ==================//

pub(crate) struct MemoryRegionImpl {
    pub(crate) c_mr: OwnedMrFid,
    pub(crate) _domain_rc: MyRc<dyn DomainImplT  >,
    pub(crate) bound_cntr: MyOnceCell<MyRc<dyn ReadCntr  >>, 
    pub(crate) bound_ep: MyOnceCell<MyRc<dyn AsRawFid >>, 
}

/// Owned wrapper around a libfabric `fid_mr`.
/// 
/// This type wraps an instance of a `fid_mr`, monitoring its lifetime and closing it when it goes out of scope.
/// For more information see the libfabric [documentation](https://ofiwg.github.io/libfabric/v1.19.0/man/fi_mr.3.html).
/// 
/// Note that other objects that rely on a MemoryRegion (e.g., [`MemoryRegionKey`]) will extend its lifetime until they
/// are also dropped.
pub struct MemoryRegion {
    pub(crate) inner: MyRc<MemoryRegionImpl>,
}

impl MemoryRegionImpl {

    #[allow(dead_code)]
    fn from_buffer<T, T0, EQ: 'static >(domain: &MyRc<crate::domain::DomainImplBase<EQ>>, buf: &[T], access: &MrAccess, requested_key: u64, flags: MrRegOpt, context: Option<&mut T0>) -> Result<Self, crate::error::Error> {
        let mut c_mr: *mut libfabric_sys::fid_mr = std::ptr::null_mut();
        let err = if let Some(ctx) = context {
                unsafe { libfabric_sys::inlined_fi_mr_reg(domain.as_raw_typed_fid(), buf.as_ptr().cast(), std::mem::size_of_val(buf), access.as_raw().into(), 0, requested_key, flags.as_raw() as u64, &mut c_mr, (ctx as *mut T0).cast() ) }
            } 
            else {
                unsafe { libfabric_sys::inlined_fi_mr_reg(domain.as_raw_typed_fid(), buf.as_ptr().cast(), std::mem::size_of_val(buf), access.as_raw().into(), 0, requested_key, flags.as_raw() as u64, &mut c_mr, std::ptr::null_mut()) }
            };
        
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {

            Ok(
                Self {
                    c_mr: OwnedMrFid::from(c_mr),
                    _domain_rc: domain.clone(),
                    bound_cntr: MyOnceCell::new(),
                    bound_ep: MyOnceCell::new(),
                })
        }
    }

    pub(crate) fn from_attr<EQ: ?Sized + 'static >(domain: &MyRc<crate::domain::DomainImplBase<EQ>>, attr: MemoryRegionAttr, flags: MrRegOpt) -> Result<Self, crate::error::Error> { // [TODO] Add context version
        let mut c_mr: *mut libfabric_sys::fid_mr = std::ptr::null_mut();
        let c_mr_ptr: *mut *mut libfabric_sys::fid_mr = &mut c_mr;
        let err = unsafe { libfabric_sys::inlined_fi_mr_regattr(domain.as_raw_typed_fid(), attr.get(), flags.as_raw() as u64, c_mr_ptr) };
    
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(
                Self {
                    c_mr: OwnedMrFid::from(c_mr),
                    _domain_rc: domain.clone(),
                    bound_cntr: MyOnceCell::new(),
                    bound_ep: MyOnceCell::new(),
                })
        }
    }
            
    #[allow(dead_code)]
    fn from_iovec<T0, EQ: 'static >(domain: &MyRc<crate::domain::DomainImplBase<EQ>>,  iov : &[crate::iovec::IoVec], access: &MrAccess, requested_key: u64, flags: MrRegOpt, context: Option<&mut T0>) -> Result<Self, crate::error::Error> {
        let mut c_mr: *mut libfabric_sys::fid_mr = std::ptr::null_mut();
        let c_mr_ptr: *mut *mut libfabric_sys::fid_mr = &mut c_mr;
        let err =
        if let Some(ctx) = context {
            unsafe { libfabric_sys::inlined_fi_mr_regv(domain.as_raw_typed_fid(), iov.as_ptr().cast(), iov.len(), access.as_raw().into(), 0, requested_key, flags.as_raw() as u64, c_mr_ptr, (ctx as *mut T0).cast()) }
        }
        else {
            unsafe { libfabric_sys::inlined_fi_mr_regv(domain.as_raw_typed_fid(), iov.as_ptr().cast(), iov.len(), access.as_raw().into(), 0, requested_key, flags.as_raw() as u64, c_mr_ptr, std::ptr::null_mut()) }
        };
    
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(
                Self {
                    c_mr: OwnedMrFid::from(c_mr),
                    _domain_rc: domain.clone(),
                    bound_cntr: MyOnceCell::new(),
                    bound_ep: MyOnceCell::new(),
                })
        }
    
    }

    pub(crate) fn key(&self) -> Result<MemoryRegionKey, crate::error::Error> {
        
        if self._domain_rc.get_mr_mode().is_raw()
        {
            self.raw_key(0)
        }
        else {
            let ret = unsafe { libfabric_sys::inlined_fi_mr_key(self.as_raw_typed_fid()) };
            if ret == crate::FI_KEY_NOTAVAIL {
                Err(crate::error::Error::from_err_code(libfabric_sys::FI_ENOKEY))
            }
            else {  
                Ok(MemoryRegionKey::Key(ret))
            }
        }
    }

    fn raw_key(&self, flags: u64) -> Result<MemoryRegionKey, crate::error::Error>  {
        let mut base_addr = 0u64;
        let mut key_size = self._domain_rc.get_mr_key_size();
        let mut raw_key = vec![0u8; key_size];
        let err = unsafe { libfabric_sys::inlined_fi_mr_raw_attr(self.as_raw_typed_fid(), &mut base_addr, raw_key.as_mut_ptr().cast(), &mut key_size, flags) };
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(unsafe {MemoryRegionKey::from_bytes_impl(&raw_key, &*self._domain_rc)})
        }
    }

    pub(crate) fn bind_cntr(&self, cntr: &MyRc<impl ReadCntr + 'static >, remote_write_event: bool) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_mr_bind(self.as_raw_typed_fid(), cntr.as_raw_fid(), if remote_write_event {libfabric_sys::FI_REMOTE_WRITE as u64} else {0}) } ;
        if err != 0 {
            if self.bound_cntr.set(cntr.clone()).is_err() {
                panic!("Memory Region already bound to an Endpoint");
            }
        }
        check_error(err.try_into().unwrap())
    }
}

impl MemoryRegionImpl {

    #[allow(dead_code)]
    pub(crate) fn bind_ep<EP: 'static , CQ: ?Sized + AsRawFid + ReadCq + 'static >(&self, ep: &MyRc<crate::ep::EndpointImplBase<EP, impl ReadEq + 'static , CQ>>) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_mr_bind(self.as_raw_typed_fid(), ep.as_raw_fid(), 0) } ;
        if err != 0 {
            if self.bound_ep.set(ep.clone()).is_err() {
                panic!("Memory Region already bound to an Endpoint");
            }
        }
        check_error(err.try_into().unwrap())
    }
}

impl MemoryRegionImpl {

pub(crate) fn refresh(&self, iov: &[crate::iovec::IoVec], flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_mr_refresh(self.as_raw_typed_fid(), iov.as_ptr().cast(), iov.len(), flags) };

        check_error(err.try_into().unwrap())
    }

    pub(crate) fn enable(&self) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_mr_enable(self.as_raw_typed_fid()) };

        check_error(err.try_into().unwrap())
    }

    pub(crate) fn address(&self, flags: u64) -> Result<u64, crate::error::Error>  {
        let mut base_addr = 0u64;
        let mut key_size = 0usize;
        let err = unsafe { libfabric_sys::inlined_fi_mr_raw_attr(self.as_raw_typed_fid(), &mut base_addr, std::ptr::null_mut(), &mut key_size, flags) };
        
        if err != 0 && -err as u32 != libfabric_sys::FI_ETOOSMALL {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(base_addr)
        }
    }



    // pub(crate) fn raw_attr(&self, base_addr: &mut u64, key_size: &mut usize, flags: u64) -> Result<(), crate::error::Error> { //[TODO] Return the key as it should be returned
    //     let err = unsafe { libfabric_sys::inlined_fi_mr_raw_attr(self.as_raw_typed_fid(), base_addr, std::ptr::null_mut(), key_size, flags) };

    //     if err != 0 {
    //         Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
    //     } 
    //     else {
    //         Ok(())
    //     }       
    // }

    // pub(crate) fn raw_attr_with_key(&self, base_addr: &mut u64, raw_key: &mut u8, key_size: &mut usize, flags: u64) -> Result<(), crate::error::Error> {
    //     let err = unsafe { libfabric_sys::inlined_fi_mr_raw_attr(self.as_raw_typed_fid(), base_addr, raw_key, key_size, flags) };

    //     if err != 0 {
    //         Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
    //     }        
    //     else {
    //         Ok(())
    //     }
    // }

    pub(crate) fn description(&self) -> MemoryRegionDesc {
        let c_desc = unsafe { libfabric_sys::inlined_fi_mr_desc(self.as_raw_typed_fid())};
        if c_desc.is_null() {
            panic!("fi_mr_desc returned NULL");
        }

        MemoryRegionDesc { c_desc }
    }
}

impl MemoryRegion {
    
    #[allow(dead_code)]
    pub(crate) fn handle(&self) -> *mut libfabric_sys::fid_mr {
        self.inner.as_raw_typed_fid()
    }

    #[allow(dead_code)]
    pub(crate) fn from_impl(mr_impl: &MyRc<MemoryRegionImpl>)  -> Self {
        MemoryRegion {
            inner: mr_impl.clone()
        }
    }

    #[allow(dead_code)]
    fn from_buffer<T, T0, EQ: 'static>(domain: &crate::domain::DomainBase<EQ>, buf: &[T], access: &MrAccess, requested_key: u64, flags: MrRegOpt, context: Option<&mut T0>) -> Result<Self, crate::error::Error> {
        Ok(
            Self {
                inner:
                    MyRc::new(MemoryRegionImpl::from_buffer(&domain.inner, buf, access, requested_key, flags, context)?)
            }
        )
    }
    
    pub(crate) fn from_attr<EQ: ?Sized + 'static>(domain: &crate::domain::DomainBase<EQ>, attr: MemoryRegionAttr, flags: MrRegOpt) -> Result<Self, crate::error::Error> { // [TODO] Add context version
        Ok(
            Self {
                inner: 
                    MyRc::new(MemoryRegionImpl::from_attr(&domain.inner, attr, flags)?)
            }
        )
    }

    #[allow(dead_code)]
    fn from_iovec<T0, EQ: 'static>(domain: &crate::domain::DomainBase<EQ>,  iov : &[crate::iovec::IoVec], access: &MrAccess, requested_key: u64, flags: MrRegOpt, context: Option<&mut T0>) -> Result<Self, crate::error::Error> {
        Ok(
            Self {
                inner: 
                    MyRc::new(MemoryRegionImpl::from_iovec(&domain.inner, iov, access, requested_key, flags, context)?)
            }
        )
    }

    /// Returns the remote key needed to access a registered memory region.
    /// 
    /// This call will automatically request a 'raw' key if the provider requires it.
    /// 
    /// Corresponds to calling `fi_mr_raw_attr` or `fi_mr_key` depending on the requirements of the respective [Domain](crate::domain::Domain). 
    pub fn key(&self) -> Result<MemoryRegionKey, crate::error::Error> {
        self.inner.key()
    }

    // fn raw_key(&self, flags: u64) -> Result<MemoryRegionKey, crate::error::Error>  {
    //     self.inner.raw_key(flags)
    // }
    
    /// Associates the memory region with a counter
    /// 
    /// Bind the memory region to `cntr` and request event generation for remote writes or atomics targeting this memory region.
    /// 
    /// Corresponds to `fi_mr_bind` with a `fid_cntr` 
    pub fn bind_cntr(&self, cntr: &crate::cntr::Counter<impl ReadCntr + 'static >, remote_write_event: bool) -> Result<(), crate::error::Error> {
        self.inner.bind_cntr(&cntr.inner, remote_write_event)
    }

    // /// Associates the memory region with an endpoint
    // /// 
    // /// Bind the memory region to `ep`.
    // /// 
    // /// Corresponds to `fi_mr_bind` with a `fid_ep` 
    // pub fn bind_ep<E>(&self, ep: &crate::ep::EndpointBase<E, EQ, CQ>) -> Result<(), crate::error::Error> {
    //     self.inner.bind_ep(&ep.inner)?;
    //     ep.inner.eq
    //         .get()
    //         .expect("Endpoint is to bound to an Event Queue")
    //         .bind_mr(&self.inner);
    //     Ok(())
    // }

    /// Notify the provider of any change to the physical pages backing a registered memory region.
    /// 
    /// Corresponds to `fi_mr_refresh`
    pub fn refresh(&self, iov: &[crate::iovec::IoVec]) -> Result<(), crate::error::Error> { //[TODO]
        self.inner.refresh(iov, 0)   
    }

    /// Enables a memory region for use.
    /// 
    /// Corresponds to `fi_mr_enable`
    pub fn enable(&self) -> Result<(), crate::error::Error> {
        self.inner.enable()
    }

    /// Retrieves the address of memory backing this memory region
    /// 
    /// Corresponds to `fi_mr_raw_attr`
    pub fn address(&self) -> Result<u64, crate::error::Error>  {
        self.inner.address(0)
    }

    /// Return a local descriptor associated with a registered memory region.
    /// 
    /// Corresponds to `fi_mr_desc`
    pub fn description(&self) -> MemoryRegionDesc {
        self.inner.description()
    }
}

//==================== Async stuff ================================//





/// An opaque wrapper for the descriptor of a [MemoryRegion] as obtained from
/// `fi_mr_desc`.
#[repr(C)]
#[derive(Clone, Debug)]
pub struct MemoryRegionDesc {
    c_desc: *mut std::ffi::c_void,
}

impl DataDescriptor for MemoryRegionDesc {
    
    fn get_desc(&mut self) -> *mut std::ffi::c_void {
        self.c_desc
    }

    fn get_desc_ptr(&mut self) -> *mut *mut std::ffi::c_void {
        &mut self.c_desc
    }
}

impl AsFid for MemoryRegion{
    fn as_fid(&self) -> fid::BorrowedFid<'_> {
        self.inner.as_fid()
    }
}

impl AsFid for MemoryRegionImpl{
    fn as_fid(&self) -> fid::BorrowedFid<'_> {
        self.c_mr.as_fid()
    }
}

impl AsRawFid for MemoryRegion{
    fn as_raw_fid(&self) -> RawFid {
        self.inner.as_raw_fid()
    }
}

impl AsRawFid for MemoryRegionImpl{
    fn as_raw_fid(&self) -> RawFid {
        self.c_mr.as_raw_fid()
    }
}

impl AsTypedFid<MrRawFid> for MemoryRegion{
    fn as_typed_fid(&self) -> fid::BorrowedTypedFid<MrRawFid> {
        self.inner.as_typed_fid()
    }
}

impl AsTypedFid<MrRawFid> for MemoryRegionImpl{
    fn as_typed_fid(&self) -> fid::BorrowedTypedFid<MrRawFid> {
        self.c_mr.as_typed_fid()
    }
}

impl AsRawTypedFid for MemoryRegion{
    type Output = MrRawFid;

    fn as_raw_typed_fid(&self) -> Self::Output {
        self.inner.as_raw_typed_fid()
    }
}

impl AsRawTypedFid for MemoryRegionImpl{
    type Output = MrRawFid;

    fn as_raw_typed_fid(&self) -> Self::Output {
        self.c_mr.as_raw_typed_fid()
    }
}

//================== Memory Region attribute ==================//

pub struct MemoryRegionAttr {
    pub(crate) c_attr: libfabric_sys::fi_mr_attr,
}

impl MemoryRegionAttr {

    pub fn new() -> Self {
        Self {
            c_attr: libfabric_sys::fi_mr_attr {
                mr_iov: std::ptr::null(),
                iov_count: 0,
                access: 0,
                offset: 0,
                requested_key: 0,
                context: std::ptr::null_mut(),
                auth_key_size: 0,
                auth_key: std::ptr::null_mut(),
                iface: libfabric_sys::fi_hmem_iface_FI_HMEM_SYSTEM,
                device: libfabric_sys::fi_mr_attr__bindgen_ty_1 {reserved: 0},
                hmem_data: std::ptr::null_mut(),
            }
        }
    }

    pub fn iov(&mut self, iov: &[crate::iovec::IoVec] ) -> &mut Self {
        self.c_attr.mr_iov = iov.as_ptr().cast();
        self.c_attr.iov_count = iov.len();
        
        self
    }

    pub fn access(&mut self, access: &MrAccess) -> &mut Self {
        self.c_attr.access = access.as_raw() as u64;
        self
    }

    pub fn access_collective(&mut self) -> &mut Self { 
        self.c_attr.access |= libfabric_sys::FI_COLLECTIVE as u64;
        self
    }

    pub fn access_send(&mut self) -> &mut Self { 
        self.c_attr.access |= libfabric_sys::FI_SEND as u64;
        self
    }

    pub fn access_recv(&mut self) -> &mut Self { 
        self.c_attr.access |= libfabric_sys::FI_RECV as u64;
        self
    }

    pub fn access_read(&mut self) -> &mut Self { 
        self.c_attr.access |= libfabric_sys::FI_READ as u64;
        self
    }

    pub fn access_write(&mut self) -> &mut Self { 
        self.c_attr.access |= libfabric_sys::FI_WRITE as u64;
        self
    }

    pub fn access_remote_write(&mut self) -> &mut Self { 
        self.c_attr.access |= libfabric_sys::FI_REMOTE_WRITE as u64;
        self
    }

    pub fn access_remote_read(&mut self) -> &mut Self { 
        self.c_attr.access |= libfabric_sys::FI_REMOTE_READ as u64;
        self
    }

    pub fn offset(&mut self, offset: u64) -> &mut Self {
        self.c_attr.offset = offset;
        self
    }

    pub fn context<T0>(&mut self, ctx: &mut T0) -> &mut Self {
        self.c_attr.context = (ctx as * mut T0).cast();
        self
    }
    
    pub fn requested_key(&mut self, key: u64) -> &mut Self {
        self.c_attr.requested_key = key;
        self
    }

    pub fn auth_key(&mut self, key: &mut [u8]) -> &mut Self {
        self.c_attr.auth_key_size = key.len();
        self.c_attr.auth_key = key.as_mut_ptr();
        self
    }

    pub fn iface(&mut self, iface: crate::enums::HmemIface) -> &mut Self {
        self.c_attr.iface = iface.as_raw();
        self.c_attr.device = match iface {
            crate::enums::HmemIface::Ze(drv_idx, dev_idx) => {
                let ze_id =  unsafe {libfabric_sys::inlined_fi_hmem_ze_device(drv_idx, dev_idx)};
                libfabric_sys::fi_mr_attr__bindgen_ty_1 {
                    ze: ze_id,
                }
            },
            crate::enums::HmemIface::System => libfabric_sys::fi_mr_attr__bindgen_ty_1{reserved: 0},
            crate::enums::HmemIface::Cuda(id) => libfabric_sys::fi_mr_attr__bindgen_ty_1{cuda: id},
            crate::enums::HmemIface::Rocr(id) => libfabric_sys::fi_mr_attr__bindgen_ty_1{cuda: id},
            crate::enums::HmemIface::Neuron(id) => libfabric_sys::fi_mr_attr__bindgen_ty_1{neuron: id},
            crate::enums::HmemIface::SynapseAi(id) => libfabric_sys::fi_mr_attr__bindgen_ty_1{synapseai: id},
        };
        self
    }

    pub(crate) fn get(&self) ->  *const libfabric_sys::fi_mr_attr {
        &self.c_attr
    }

    #[allow(dead_code)]
    pub(crate) fn get_mut(&mut self) ->  *mut libfabric_sys::fi_mr_attr {
        &mut self.c_attr
    }
}

impl Default for MemoryRegionAttr {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for the [MemoryRegion] type.
/// 
/// `MemoryRegionBuilder` is used to configure and build a new [MemoryRegion].
/// It encapsulates an incremental configuration of the address vector set, as provided by a `fi_mr_attr`,
/// followed by a call to `fi_mr_regattr.  
pub struct MemoryRegionBuilder<'a> {
    pub(crate) mr_attr: MemoryRegionAttr,
    pub(crate) iovs: Vec<IoVec<'a>>,
    pub(crate) flags: MrRegOpt,
}

impl<'a> MemoryRegionBuilder<'a> {


    /// Initiates the creation of new [MemoryRegion] on `domain`, with backing memory `buff`.
    /// 
    /// The initial configuration is only setting the fields `fi_mr_attr::mr_iov`, `fi_mr_attr::iface`.
    pub fn new<T>(buff: &'a [T], iface: crate::enums::HmemIface) -> Self {

        let mut mr_attr = MemoryRegionAttr::new();
            mr_attr.iface(iface);
        Self {
            mr_attr,
            flags: MrRegOpt::new(),
            iovs: vec![IoVec::from_slice(buff)],
        }
    }

    /// Add another backing buffer to the memory region
    /// 
    /// Corresponds to 'pusing' another value to the `fi_mr_attr::mr_iov` field.
    pub fn add_buffer<T>(mut self, buff: &'a [T]) -> Self {
        self.iovs.push(IoVec::from_slice(buff));

        self
    }

    // fn iovs(mut self, iov: &[crate::iovec::IoVec<T>] ) -> Self {
    //     self.mr_attr.iov(iov);
    //     self
    // }

    /// Indicates that the MR may be used for collective operations.
    /// 
    /// Corresponds to setting the respective bitflag of the `fi_mr_attr::access` field
    pub fn access_collective(mut self) -> Self {  //[TODO] Required if the FI_MR_COLLECTIVE mr_mode bit has been set on the domain.
                                                  //[TODO] Should be paired with FI_SEND/FI_RECV
        self.mr_attr.access_collective();
        self
    }

    /// Indicates that the MR may be used for send operations.
    /// 
    /// Corresponds to setting the respective bitflag of the `fi_mr_attr::access` field
    pub fn access_send(mut self) -> Self { 
        self.mr_attr.access_send();
        self
    }

    /// Indicates that the MR may be used for receive operations.
    /// 
    /// Corresponds to setting the respective bitflag of the `fi_mr_attr::access` field
    pub fn access_recv(mut self) -> Self { 
        self.mr_attr.access_recv();
        self
    }

    /// Indicates that the MR may be used as buffer to store the results of RMA read operations.
    /// 
    /// Corresponds to setting the respective bitflag of the `fi_mr_attr::access` field
    pub fn access_read(mut self) -> Self { 
        self.mr_attr.access_read();
        self
    }

    /// Indicates that the memory buffer may be used as the source buffer for RMA write and atomic operations on the initiator side
    /// 
    /// Corresponds to setting the respective bitflag of the `fi_mr_attr::access` field
    pub fn access_write(mut self) -> Self { 
        self.mr_attr.access_write();
        self
    }

    /// Indicates that the memory buffer may be used as the target buffer of an RMA write or atomic operation.
    /// 
    /// Corresponds to setting the respective bitflag of the `fi_mr_attr::access` field
    pub fn access_remote_write(mut self) -> Self { 
        self.mr_attr.access_remote_write();
        self
    }

    /// Indicates that the memory buffer may be used as the source buffer of an RMA read operation on the target side
    /// 
    /// Corresponds to setting the respective bitflag of the `fi_mr_attr::access` field
    pub fn access_remote_read(mut self) -> Self { 
        self.mr_attr.access_remote_read();
        self
    }

    /// Another method to provide the access permissions collectively
    /// 
    /// Corresponds to setting the respective bitflags of the `fi_mr_attr::access` field
    pub fn access(mut self, access: &MrAccess) -> Self {
        self.mr_attr.access(access);
        self
    }

    // pub fn offset(mut self, offset: u64) -> Self {
    //     self.mr_attr.offset(offset);
    //     self
    // }

    /// Application context associated with asynchronous memory registration operations.
    /// 
    /// Corresponds to setting the `fi_mr_attr::context` field to `ctx`
    pub fn context<T0>(mut self, ctx: &mut T0) -> Self {
        self.mr_attr.context(ctx);
        self
    }
    
    /// An application specified access key associated with the memory region.
    /// 
    /// Corresponds to setting the `fi_mr_attr::requested_key` field
    pub fn requested_key(mut self, key: u64) -> Self {
        self.mr_attr.requested_key(key);
        self
    }

    /// Indicates the key to associate with this memory registration
    /// 
    /// Corresponds to setting the fields `fi_mr_attr::auth_key` and `fi_mr_attr::auth_key_size`
    pub fn auth_key(mut self, key: &mut [u8]) -> Self {
        self.mr_attr.auth_key(key);
        self
    }

    pub fn hmem_device_only(mut self) -> Self {
        self.flags = self.flags.hmem_device_only();
        self
    }

    pub fn hmem_host_alloc(mut self) -> Self {
        self.flags = self.flags.hmem_host_alloc();
        self
    }

    pub fn rma_event(mut self) -> Self {
        self.flags = self.flags.rma_event();
        self
    }

    pub fn rma_pmem(mut self) -> Self {
        self.flags = self.flags.rma_pmem();
        self
    }

    /// Constructs a new [MemoryRegion] with the configurations requested so far.
    /// 
    /// Corresponds to creating a `fi_mr_attr`, setting its fields to the requested ones,
    /// and passign it to `fi_mr_regattr`.
    pub fn build<EQ: ?Sized + 'static>(mut self, domain: &'a crate::domain::DomainBase<EQ>) -> Result<MemoryRegion, crate::error::Error> {
        if domain.inner._eq_rc.get().is_some() {
            let (_eq, async_reg) = domain.inner._eq_rc.get().unwrap();
            if *async_reg {
                panic!("Manual async memory registration is not supported. Use the ::async_::mr::MemoryRegionBuilder for that.")
            }
        }
        self.mr_attr.iov(&self.iovs);
        MemoryRegion::from_attr(domain, self.mr_attr, self.flags)
    }

    // /// Constructs a new [MemoryRegion] with the configurations requested so far.
    // /// 
    // /// Corresponds to creating a `fi_mr_attr`, setting its fields to the requested ones,
    // /// and passign it to `fi_mr_regattr`.
    // pub async fn build_async(self) -> Result<(Event<usize>,MemoryRegion), crate::error::Error> {
    //     panic!("Async memory registration is currently not supported due to a potential bug in libfabric");
    //     self.mr_attr.iov(&self.iovs);
    //     MemoryRegion::from_attr_async(self.domain, self.mr_attr, self.flags).await
    // }
}

//=================== Async Stuff =========================//




//================== Memory Region tests ==================//
#[cfg(test)]
mod tests {
    use crate::{enums::MrAccess, info::{Info, InfoHints, Version}};

    use super::MemoryRegionBuilder;


    pub fn ft_alloc_bit_combo(fixed: u64, opt: u64) -> Vec<u64> {
        let bits_set = |mut val: u64 | -> u64 { let mut cnt = 0; while val > 0 {  cnt += 1 ; val &= val-1; } cnt };
        let num_flags = bits_set(opt) + 1;
        let len = 1 << (num_flags - 1) ;
        let mut flags = vec![0_u64 ; num_flags as usize];
        let mut num_flags = 0;
        for i in 0..8*std::mem::size_of::<u64>(){
            if opt >> i & 1 == 1 {
                flags[num_flags] = 1 << i; 
                num_flags += 1;
            }
        }
        let mut combos = Vec::new();

        for index in 0..len {
            combos.push(fixed);
            for (i, val) in flags.iter().enumerate().take(8*std::mem::size_of::<u64>()){
                if index >> i & 1 == 1 {
                    combos[index] |= val;
                }
            }
        }

        combos
    }
    pub struct TestSizeParam(pub u64);
    pub const DEF_TEST_SIZES: [TestSizeParam; 6] = [TestSizeParam(1 << 0), TestSizeParam(1 << 1), TestSizeParam(1 << 2), TestSizeParam(1 << 3), TestSizeParam(1 << 4), TestSizeParam(1 << 5) ];

    #[test]
    fn mr_reg() {
        // let ep_attr = crate::ep::EndpointAttr::new();
        // let mut dom_attr = crate::domain::DomainAttr::new();
        // dom_attr.mode = crate::enums::Mode::all();
        // dom_attr.mr_mode = crate::enums::MrMode::new().basic().scalable().local().inverse();
        
        // let hints = InfoHints::new()
        //     .caps(crate::infocapsoptions::InfoCaps::new().msg().rma())
        //     .ep_attr(ep_attr)
        //     .domain_attr(dom_attr);

        let info = Info::new(&Version{major: 1, minor: 19})
            .enter_hints()
                .caps(crate::infocapsoptions::InfoCaps::new().msg().rma())
                .enter_domain_attr()
                    .mode(crate::enums::Mode::all())
                    .mr_mode(crate::enums::MrMode::new().basic().scalable().local().inverse())
                .leave_domain_attr()
            .leave_hints()
            .get()
            .unwrap();

        let entry = info.into_iter().next();
        
        if let Some(entry) = entry {

            let fab = crate::fabric::FabricBuilder::new().build(&entry).unwrap();
            let domain = crate::domain::DomainBuilder::new(&fab, &entry).build().unwrap();

            let mut mr_access: u64 = 0;
            if entry.mode().is_local_mr() || entry.domain_attr().mr_mode().is_local() {

                if entry.caps().is_msg() || entry.caps().is_tagged() {
                    let mut on = false;
                    if entry.caps().is_send() {
                        mr_access |= libfabric_sys::FI_SEND as u64;
                        on = true;
                    }
                    if entry.caps().is_recv() {
                        mr_access |= libfabric_sys::FI_RECV  as u64 ;
                        on = true;
                    }
                    if !on {
                        mr_access |= libfabric_sys::FI_SEND as u64 & libfabric_sys::FI_RECV as u64;
                    }
                }
            }
            else if entry.caps().is_rma() || entry.caps().is_atomic() {
                if entry.caps().is_remote_read() || !(entry.caps().is_read() || entry.caps().is_write() || entry.caps().is_remote_write()) {
                    mr_access |= libfabric_sys::FI_REMOTE_READ as u64 ;
                }
                else {
                    mr_access |= libfabric_sys::FI_REMOTE_WRITE as u64 ;
                }
            }

            let combos = ft_alloc_bit_combo(0, mr_access);
            
            for test in &DEF_TEST_SIZES {
                let buff_size = test.0;
                let buf = vec![0_u64; buff_size as usize];
                for combo in &combos {
                    let _mr = MemoryRegionBuilder::new(&buf, crate::enums::HmemIface::System)
                        // .iov(std::slice::from_mut(&mut IoVec::from_slice_mut(&mut buf)))
                        .access(&MrAccess::from_raw(*combo as u32))
                        .requested_key(0xC0DE)
                        
                        .build(&domain)
                        .unwrap();
                    // mr.close().unwrap();
                }
            }
            
            // domain.close().unwrap();
            // fab.close().unwrap();
        }
        else {
            panic!("No capable fabric found!");
        }
    }
}

#[cfg(test)]
mod libfabric_lifetime_tests {
    use crate::{enums::MrAccess, info::{Info, InfoHints, Version}};

    use super::MemoryRegionBuilder;
    
    #[test]
    fn mr_drops_before_domain() {
        // let ep_attr = crate::ep::EndpointAttr::new();
        // let mut dom_attr = crate::domain::DomainAttr::new();
        //     dom_attr.mode = crate::enums::Mode::all();
        //     dom_attr.mr_mode = crate::enums::MrMode::new().basic().scalable().local().inverse();
        
        // let hints = InfoHints::new()
        //     .caps(crate::infocapsoptions::InfoCaps::new().msg().rma())
        //     .ep_attr(ep_attr)
        //     .domain_attr(dom_attr);

        let info = Info::new(&Version{major: 1, minor: 19})
            .enter_hints()
                .caps(crate::infocapsoptions::InfoCaps::new().msg().rma())
                .enter_domain_attr()
                    .mode(crate::enums::Mode::all())
                    .mr_mode(crate::enums::MrMode::new().basic().scalable().local().inverse())
                .leave_domain_attr()
            .leave_hints()
            .get()
            .unwrap();

        let entry = info.into_iter().next();
        
        if let Some(entry) = entry {

            let fab = crate::fabric::FabricBuilder::new().build(&entry).unwrap();
            let domain = crate::domain::DomainBuilder::new(&fab, &entry).build().unwrap();

            let mut mr_access: u64 = 0;

            if entry.mode().is_local_mr() || entry.domain_attr().mr_mode().is_local() {

                if entry.caps().is_msg() || entry.caps().is_tagged() {
                    let mut on = false;
                    if entry.caps().is_send() {
                        mr_access |= libfabric_sys::FI_SEND as u64;
                        on = true;
                    }
                    if entry.caps().is_recv() {
                        mr_access |= libfabric_sys::FI_RECV  as u64 ;
                        on = true;
                    }
                    if !on {
                        mr_access |= libfabric_sys::FI_SEND as u64 & libfabric_sys::FI_RECV as u64;
                    }
                }
            }
            else if entry.caps().is_rma() || entry.caps().is_atomic() {
                if entry.caps().is_remote_read() || !(entry.caps().is_read() || entry.caps().is_write() || entry.caps().is_remote_write()) {
                    mr_access |= libfabric_sys::FI_REMOTE_READ as u64 ;
                }
                else {
                    mr_access |= libfabric_sys::FI_REMOTE_WRITE as u64 ;
                }
            }

            let combos = super::tests::ft_alloc_bit_combo(0, mr_access);
            
            let mut mrs = Vec::new();
            for test in &super::tests::DEF_TEST_SIZES {
                let buff_size = test.0;
                let buf = vec![0_u64; buff_size as usize ];
                for combo in &combos {
                    let mr = MemoryRegionBuilder::new(&buf, crate::enums::HmemIface::System)
                        .access(&MrAccess::from_raw(*combo as u32))
                        .requested_key(0xC0DE)
                        .build(&domain)
                        .unwrap();
                    mrs.push(mr);
                }
            }
            drop(domain);
        }
        else {
            panic!("No capable fabric found!");
        }
    }
}