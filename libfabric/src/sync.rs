use std::os::fd::BorrowedFd;

use crate::{enums::{self, WaitObjType2}, fabric::FabricImpl, fid::{self, AsRawFid, AsRawTypedFid, AsTypedFid, OwnedPollFid, OwnedWaitFid, PollRawFid, RawFid, WaitRawFid}, utils::check_error, MyRc};
use crate::fid::AsFid;


//================== Wait (fi_wait) ==================//

pub struct WaitSetBuilder<'a> {
    wait_attr : WaitSetAttr,
    fabric: &'a crate::fabric::Fabric,
}

impl<'a> WaitSetBuilder<'a> {
    pub fn new(fabric: &'a crate::fabric::Fabric) -> Self {
        WaitSetBuilder {
            wait_attr: WaitSetAttr::new(),
            fabric,
        }
    }

    pub fn wait_obj(mut self, wait_obj: enums::WaitObj2) -> Self {
        self.wait_attr.wait_obj(wait_obj);
        self
    }

    pub fn build(self) -> Result<WaitSet, crate::error::Error> {
        WaitSet::new(self.fabric, self.wait_attr)
    }
}

pub(crate) struct WaitSetImpl {
    c_wait: OwnedWaitFid,
    _fabric_rc: MyRc<FabricImpl>,
}

pub struct WaitSet {
    inner: MyRc<WaitSetImpl>,
}

impl WaitSetImpl {

    pub(crate) fn new(fabric: &crate::fabric::Fabric, mut attr: WaitSetAttr) -> Result<Self, crate::error::Error> {
        let mut c_wait: WaitRawFid  = std::ptr::null_mut();

        let err = unsafe {libfabric_sys::inlined_fi_wait_open(fabric.as_raw_typed_fid(), attr.get_mut(), &mut c_wait)};
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(
                Self {
                    c_wait: OwnedWaitFid::from(c_wait),
                    _fabric_rc: fabric.inner.clone(), 
                })
        }
    }

    pub(crate) fn wait(&self, timeout: i32) -> Result<(), crate::error::Error> { 
        let err = unsafe { libfabric_sys::inlined_fi_wait(self.as_raw_typed_fid(), timeout) };

        check_error(err.try_into().unwrap())
    }

    pub(crate) fn wait_object(&self) -> Result<WaitObjType2, crate::error::Error> {
        let mut res: libfabric_sys::fi_wait_obj = 0;
        let err = unsafe{libfabric_sys::inlined_fi_control(self.as_raw_fid(), libfabric_sys::FI_GETWAITOBJ as i32, (&mut res as *mut libfabric_sys::fi_wait_obj).cast() )};
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            let ret = 
            if res == libfabric_sys::fi_wait_obj_FI_WAIT_UNSPEC {
                WaitObjType2::Unspec
            }
            else if res == libfabric_sys::fi_wait_obj_FI_WAIT_FD {
                let mut fd = 0; 
                let err = unsafe{libfabric_sys::inlined_fi_control(self.as_raw_fid(), libfabric_sys::FI_GETWAIT as i32, (&mut fd as *mut i32).cast() )};
                if err != 0 {
                    return Err(crate::error::Error::from_err_code((-err).try_into().unwrap()));
                }
                WaitObjType2::Fd(unsafe{BorrowedFd::borrow_raw(fd)})
            }
            else if res == libfabric_sys::fi_wait_obj_FI_WAIT_MUTEX_COND {
                let mut cond: libfabric_sys::fi_mutex_cond = libfabric_sys::fi_mutex_cond {
                    mutex: std::ptr::null_mut(),
                    cond: std::ptr::null_mut(),
                }; 
                let err = unsafe{libfabric_sys::inlined_fi_control(self.as_raw_fid(), libfabric_sys::FI_GETWAIT as i32, (&mut cond as *mut libfabric_sys::fi_mutex_cond).cast() )};
                if err != 0 {
                    return Err(crate::error::Error::from_err_code((-err).try_into().unwrap()));
                }
                WaitObjType2::MutexCond(cond)
            }
            else if res == libfabric_sys::fi_wait_obj_FI_WAIT_YIELD {
                WaitObjType2::Yield
            }
            else if res == libfabric_sys::fi_wait_obj_FI_WAIT_POLLFD {
                let mut wait: libfabric_sys::fi_wait_pollfd = libfabric_sys::fi_wait_pollfd {
                    change_index: 0,
                    nfds: 0,
                    fd: std::ptr::null_mut(),
                }; 
                let err = unsafe{libfabric_sys::inlined_fi_control(self.as_raw_fid(), libfabric_sys::FI_GETWAIT as i32, (&mut wait as *mut libfabric_sys::fi_wait_pollfd).cast() )};
                if err != 0 {
                    return Err(crate::error::Error::from_err_code((-err).try_into().unwrap()));
                }
                WaitObjType2::PollFd(wait)
            }
            else {
                panic!("Unexpected waitobject type")
            };
            Ok(ret)
        }
    }
}


impl WaitSet {
        
    pub(crate) fn new(fabric: &crate::fabric::Fabric, attr: WaitSetAttr) -> Result<Self, crate::error::Error> {
        Ok (
            Self {
                inner: 
                    MyRc::new(WaitSetImpl::new(fabric, attr)?)
            }
        )
    }

    pub fn wait(&self, timeout: i32) -> Result<(), crate::error::Error> { 
        self.inner.wait(timeout)
    }

    pub fn wait_object(&self) -> Result<WaitObjType2, crate::error::Error> {
        self.inner.wait_object()
    }
}


impl AsFid for WaitSetImpl {
    fn as_fid(&self) -> fid::BorrowedFid<'_> {
        self.c_wait.as_fid()
    }
}

impl AsFid for WaitSet {
    fn as_fid(&self) -> fid::BorrowedFid<'_> {
        self.inner.as_fid()
    }
}

impl AsRawFid for WaitSetImpl {
    fn as_raw_fid(&self) -> RawFid {
        self.c_wait.as_raw_fid()
    }
}

impl AsRawFid for WaitSet {
    fn as_raw_fid(&self) -> RawFid {
        self.inner.as_raw_fid()
    }
}

impl AsTypedFid<WaitRawFid> for WaitSetImpl {
    fn as_typed_fid(&self) -> fid::BorrowedTypedFid<WaitRawFid> {
        self.c_wait.as_typed_fid()
    }
}

impl AsTypedFid<WaitRawFid> for WaitSet {
    fn as_typed_fid(&self) -> fid::BorrowedTypedFid<WaitRawFid> {
        self.inner.as_typed_fid()
    }
}

impl AsRawTypedFid for WaitSetImpl {
    type Output = WaitRawFid;
    
    fn as_raw_typed_fid(&self) -> Self::Output {
        todo!()
    }
}

impl AsRawTypedFid for WaitSet {
    type Output = WaitRawFid;

    fn as_raw_typed_fid(&self) -> Self::Output {
        self.inner.as_raw_typed_fid()
    }
}

//================== Wait attribute ==================//

pub(crate) struct WaitSetAttr {
    pub(crate) c_attr: libfabric_sys::fi_wait_attr,
}

impl WaitSetAttr {

    pub(crate) fn new () -> Self {
        Self {
            c_attr: libfabric_sys::fi_wait_attr {
                wait_obj: libfabric_sys::fi_wait_obj_FI_WAIT_UNSPEC,
                flags: 0,
            }
        }
    }
    
    pub(crate) fn wait_obj(&mut self, wait_obj: enums::WaitObj2) -> &mut Self {
        self.c_attr.wait_obj = wait_obj.get_value();
        self
    }

    #[allow(dead_code)]
    pub(crate) fn get(&self) -> *const libfabric_sys::fi_wait_attr {
        &self.c_attr
    }

    pub(crate) fn get_mut(&mut self) -> *mut libfabric_sys::fi_wait_attr {
        &mut self.c_attr
    }   
}

//================== Poll (fi_poll) ==================//


pub struct PollSetBuilder {
    poll_attr : PollSetAttr,
}

impl PollSetBuilder {
    pub fn new() -> Self {
        PollSetBuilder {
            poll_attr: PollSetAttr::new(),
        }
    }

    pub fn build<EQ: Sync + Send>(self, domain: &crate::domain::DomainBase<EQ>) -> Result<PollSet, crate::error::Error> {
        PollSet::new(domain, self.poll_attr)
    }
}

impl Default for PollSetBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct PollSetImpl {
    pub(crate) c_poll: OwnedPollFid,
}

pub struct PollSet {
    inner: MyRc<PollSetImpl>
}

impl PollSetImpl {

    pub(crate) fn new<EQ: Sync + Send>(domain: &crate::domain::DomainBase<EQ>, mut attr: crate::sync::PollSetAttr) -> Result<Self, crate::error::Error> {
        let mut c_poll: PollRawFid = std::ptr::null_mut();
        let err = unsafe { libfabric_sys::inlined_fi_poll_open(domain.as_raw_typed_fid(), attr.get_mut(), &mut c_poll) };
    
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(
                Self {
                    c_poll: OwnedPollFid::from(c_poll), 
                })
        }
    }

    pub(crate) fn poll<T0>(&self, contexts: &mut [T0]) -> Result<usize, crate::error::Error> {
        let ret = unsafe { libfabric_sys::inlined_fi_poll(self.as_raw_typed_fid(), contexts.as_mut_ptr().cast(),  contexts.len() as i32) };
        
        if ret < 0{
            Err(crate::error::Error::from_err_code((-ret).try_into().unwrap()))
        }
        else {
            Ok(ret as usize)
        }
    }

    pub(crate) fn add(&self, fid: &impl AsFid, flags:u64) -> Result<(), crate::error::Error> { //[TODO] fid should implement Waitable trait
        let err = unsafe { libfabric_sys::inlined_fi_poll_add(self.as_raw_typed_fid(), fid.as_fid().as_raw_fid(), flags) };

        check_error(err.try_into().unwrap())
    }

    pub(crate) fn del(&self, fid: &impl AsFid, flags:u64) -> Result<(), crate::error::Error> { //[TODO] fid should implement Waitable trait
        let err = unsafe { libfabric_sys::inlined_fi_poll_del(self.as_raw_typed_fid(), fid.as_fid().as_raw_fid(), flags) };

        check_error(err.try_into().unwrap())
    }

    pub(crate) fn wait_object(&self) -> Result<WaitObjType2, crate::error::Error> {
        let mut res: libfabric_sys::fi_wait_obj = 0;
        let err = unsafe{libfabric_sys::inlined_fi_control(self.as_raw_fid(), libfabric_sys::FI_GETWAITOBJ as i32, (&mut res as *mut libfabric_sys::fi_wait_obj).cast())};
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            let ret = 
            if res == libfabric_sys::fi_wait_obj_FI_WAIT_UNSPEC {
                WaitObjType2::Unspec
            }
            else if res == libfabric_sys::fi_wait_obj_FI_WAIT_FD {
                let mut fd = 0; 
                let err = unsafe{libfabric_sys::inlined_fi_control(self.as_raw_fid(), libfabric_sys::FI_GETWAIT as i32, (&mut fd as *mut i32).cast())};
                if err != 0 {
                    return Err(crate::error::Error::from_err_code((-err).try_into().unwrap()));
                }
                WaitObjType2::Fd(unsafe{BorrowedFd::borrow_raw(fd)})
            }
            else if res == libfabric_sys::fi_wait_obj_FI_WAIT_MUTEX_COND {
                let mut cond: libfabric_sys::fi_mutex_cond = libfabric_sys::fi_mutex_cond {
                    mutex: std::ptr::null_mut(),
                    cond: std::ptr::null_mut(),
                }; 
                let err = unsafe{libfabric_sys::inlined_fi_control(self.as_raw_fid(), libfabric_sys::FI_GETWAIT as i32, (&mut cond as *mut libfabric_sys::fi_mutex_cond).cast())};
                if err != 0 {
                    return Err(crate::error::Error::from_err_code((-err).try_into().unwrap()));
                }
                WaitObjType2::MutexCond(cond)
            }
            else if res == libfabric_sys::fi_wait_obj_FI_WAIT_YIELD {
                WaitObjType2::Yield
            }
            else if res == libfabric_sys::fi_wait_obj_FI_WAIT_POLLFD {
                let mut wait: libfabric_sys::fi_wait_pollfd = libfabric_sys::fi_wait_pollfd {
                    change_index: 0,
                    nfds: 0,
                    fd: std::ptr::null_mut(),
                }; 
                let err = unsafe{libfabric_sys::inlined_fi_control(self.as_raw_fid(), libfabric_sys::FI_GETWAIT as i32, (&mut wait as *mut libfabric_sys::fi_wait_pollfd).cast())};
                if err != 0 {
                    return Err(crate::error::Error::from_err_code((-err).try_into().unwrap()));
                }
                WaitObjType2::PollFd(wait)
            }
            else {
                panic!("Unexpected waitobject type")
            };
            Ok(ret)
        }
    }
}

impl PollSet {
    
    pub(crate) fn new<EQ: Sync + Send>(domain: &crate::domain::DomainBase<EQ>, attr: crate::sync::PollSetAttr) -> Result<Self, crate::error::Error> {
        Ok(
            Self {
                inner: 
                    MyRc::new(PollSetImpl::new(domain, attr)?)
            }
        )
    }

    pub fn poll<T0>(&self, contexts: &mut [T0]) -> Result<usize, crate::error::Error> {
        self.inner.poll(contexts)
    }


    pub fn add(&self, fid: &impl AsFid) -> Result<(), crate::error::Error> { //[TODO] fid should implement Waitable trait
        self.inner.add(fid, 0)
    }

    pub fn del(&self, fid: &impl AsFid) -> Result<(), crate::error::Error> { //[TODO] fid should implement Waitable trait
        self.inner.del(fid, 0)
    }

    pub fn wait_object(&self) -> Result<WaitObjType2, crate::error::Error> {
        self.inner.wait_object()
    }
}

impl AsFid for PollSetImpl {
    fn as_fid(&self) -> fid::BorrowedFid<'_> {
        self.c_poll.as_fid()
    }
}

impl AsFid for PollSet {
    fn as_fid(&self) -> fid::BorrowedFid<'_> {
        self.inner.as_fid()
    }
}

impl AsRawFid for PollSetImpl {
    fn as_raw_fid(&self) -> RawFid {
        self.c_poll.as_raw_fid()
    }
}

impl AsRawFid for PollSet {
    fn as_raw_fid(&self) -> RawFid {
        self.inner.as_raw_fid()
    }
}

impl AsTypedFid<PollRawFid> for PollSetImpl {
    fn as_typed_fid(&self) -> fid::BorrowedTypedFid<PollRawFid> {
        self.c_poll.as_typed_fid()
    }
}

impl AsTypedFid<PollRawFid> for PollSet {
    fn as_typed_fid(&self) -> fid::BorrowedTypedFid<PollRawFid> {
        self.inner.as_typed_fid()
    }
}

impl AsRawTypedFid for PollSetImpl {
    type Output = PollRawFid;

    fn as_raw_typed_fid(&self) -> Self::Output {
        self.c_poll.as_raw_typed_fid()
    }
}

impl AsRawTypedFid for PollSet {
    type Output = PollRawFid;

    fn as_raw_typed_fid(&self) -> Self::Output {
        self.inner.as_raw_typed_fid()
    }
}

//================== Poll attribute ==================//

pub struct PollSetAttr {
    pub(crate) c_attr: libfabric_sys::fi_poll_attr,
}

impl PollSetAttr {
    pub(crate) fn new() -> Self {
        Self {
            c_attr: libfabric_sys::fi_poll_attr {
                flags: 0,
            }
        }
    }

    #[allow(dead_code)]
    pub(crate) fn get(&self) ->  *const libfabric_sys::fi_poll_attr {
        &self.c_attr
    }   

    pub(crate) fn get_mut(&mut self) ->  *mut libfabric_sys::fi_poll_attr {
        &mut self.c_attr
    }      
}