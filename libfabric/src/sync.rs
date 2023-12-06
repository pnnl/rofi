
//================== Wait (fi_wait) ==================//
pub struct Wait {
    pub(crate) c_wait: *mut libfabric_sys::fid_wait,
}

impl Wait {
    pub(crate) fn new(fabric: &crate::fabric::Fabric, mut attr: WaitAttr) -> Self {
        let mut c_wait: *mut libfabric_sys::fid_wait  = std::ptr::null_mut();
        let c_wait_ptr: *mut *mut libfabric_sys::fid_wait = &mut c_wait;

        let err = unsafe {libfabric_sys::inlined_fi_wait_open(fabric.c_fabric, attr.get_mut(), c_wait_ptr)};
        if err != 0 {
            panic!("fi_eq_open failed {}", err);
        }

        Self { c_wait }        
    }

    pub fn wait(&self, timeout: i32) -> i32 { // [TODO] Probably returns error when timeout occurs, 0 if done, or error
        unsafe { libfabric_sys::inlined_fi_wait(self.c_wait, timeout) } 
    }
}

//================== Wait attribute ==================//

pub struct WaitAttr {
    pub(crate) c_attr: libfabric_sys::fi_wait_attr,
}

impl WaitAttr {
    
    #[allow(dead_code)]
    pub(crate) fn get(&self) -> *const libfabric_sys::fi_wait_attr {
        &self.c_attr
    }

    pub(crate) fn get_mut(&mut self) -> *mut libfabric_sys::fi_wait_attr {
        &mut self.c_attr
    }   
}

//================== Poll (fi_poll) ==================//


pub struct Poll {
    pub(crate) c_poll: *mut libfabric_sys::fid_poll,
}

impl Poll {
    pub(crate) fn new(domain: &crate::domain::Domain, mut attr: crate::sync::PollAttr) -> Self {
        let mut c_poll: *mut libfabric_sys::fid_poll = std::ptr::null_mut();
        let c_poll_ptr: *mut *mut libfabric_sys::fid_poll = &mut c_poll;
        let err = unsafe { libfabric_sys::inlined_fi_poll_open(domain.c_domain, attr.get_mut(), c_poll_ptr) };
    
        if err != 0 {
            panic!("fi_poll_open failed {}", err);
        }
    
        Self { c_poll }
    }

    pub fn poll<T0>(&self, contexts: &mut [T0]) {
        let err = unsafe { libfabric_sys::inlined_fi_poll(self.c_poll, contexts.as_mut_ptr() as *mut *mut std::ffi::c_void,  contexts.len() as i32) };
        
        if err != 0{
            panic!("fi_poll failed {}", err);
        }
    }

    pub fn add(&self, fid: &impl crate::FID, flags:u64) {
        let err = unsafe { libfabric_sys::inlined_fi_poll_add(self.c_poll, fid.fid(), flags) };

        if err != 0 {
            panic!("fi_poll_add failed {}", err);
        }
    }

    pub fn del(&self, fid: &impl crate::FID, flags:u64) {
        let err = unsafe { libfabric_sys::inlined_fi_poll_del(self.c_poll, fid.fid(), flags) };

        if err != 0 {
            panic!("fi_poll_del failed {}", err);
        }
    }


}

//================== Poll attribute ==================//

pub struct PollAttr {
    pub(crate) c_attr: libfabric_sys::fi_poll_attr,
}

impl PollAttr {

    #[allow(dead_code)]
    pub(crate) fn get(&self) ->  *const libfabric_sys::fi_poll_attr {
        &self.c_attr
    }   

    pub(crate) fn get_mut(&mut self) ->  *mut libfabric_sys::fi_poll_attr {
        &mut self.c_attr
    }      
}

//================== PollFd (pollfd) ==================//

pub struct PollFd {
    c_poll: libfabric_sys::pollfd,
}

impl PollFd {
    pub fn new() -> Self {
        let c_poll = libfabric_sys::pollfd{ fd: 0, events: 0, revents: 0 };
        Self { c_poll }
    }

    pub fn fd(&mut self, fd: i32) -> &mut Self {
        
        self.c_poll.fd = fd;
        self
    }

    pub fn events(&mut self, events: i16) -> &mut Self {
        
        self.c_poll.events = events;
        self
    }

    pub fn revents(&mut self, revents: i16) -> &mut Self {
        
        self.c_poll.revents = revents;
        self
    }
}