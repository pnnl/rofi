use debug_print::debug_println;

#[allow(unused_imports)]
use crate::FID;

//================== Domain (fi_domain) ==================//

pub struct Domain {
    pub(crate) c_domain: *mut libfabric_sys::fid_domain,
}

impl Domain {

    pub(crate) fn new(fabric: &crate::fabric::Fabric, info: &crate::InfoEntry) -> Result<Self, crate::error::Error> {
        let mut c_domain: *mut libfabric_sys::fid_domain = std::ptr::null_mut();
        let c_domain_ptr: *mut *mut libfabric_sys::fid_domain = &mut c_domain;
        let err = unsafe { libfabric_sys::inlined_fi_domain(fabric.c_fabric, info.c_info, c_domain_ptr, std::ptr::null_mut()) };
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()) )
        }
        else {
            Ok(
                Self { c_domain } 
            )
        }
    }

    pub(crate) fn new2(fabric: &crate::fabric::Fabric, info: &crate::InfoEntry, flags: u64) -> Result<Self, crate::error::Error> {
        let mut c_domain: *mut libfabric_sys::fid_domain = std::ptr::null_mut();
        let c_domain_ptr: *mut *mut libfabric_sys::fid_domain = &mut c_domain;
        let err = unsafe { libfabric_sys::inlined_fi_domain2(fabric.c_fabric, info.c_info, c_domain_ptr, flags, std::ptr::null_mut()) };

        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()) )
        }
        else {
            Ok(
                Self { c_domain } 
            )
        }
    }

    pub fn bind(self, fid: &impl crate::FID, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe{ libfabric_sys::inlined_fi_domain_bind(self.c_domain, fid.fid(), flags)} ;

        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()) )
        }
        else {
            Ok(())
        }
    } 

    pub fn ep(&self, info: &crate::InfoEntry) -> Result<crate::ep::Endpoint, crate::error::Error> {
        crate::ep::Endpoint::new(self, info)
    }

    pub fn srx_context<T0>(&self, rx_attr: crate::RxAttr) -> Result<crate::ep::Endpoint, crate::error::Error> {
        crate::ep::Endpoint::from_attr(self, rx_attr)
    }
    
    pub fn srx_context_with_context<T0>(&self, rx_attr: crate::RxAttr, context: &mut T0) -> Result<crate::ep::Endpoint, crate::error::Error> {
        crate::ep::Endpoint::from_attr_with_context(self, rx_attr, context)
    }

    pub fn query_atomic(&self, datatype: crate::DataType, op: crate::enums::Op, mut attr: crate::AtomicAttr, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_query_atomic(self.c_domain, datatype, op.get_value(), attr.get_mut(), flags )};

        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(())
        }
    }
    pub fn cq_open(&self, attr: crate::cq::CompletionQueueAttr) -> Result<crate::cq::CompletionQueue, crate::error::Error> {
        crate::cq::CompletionQueue::new(self, attr)
    }

    pub fn cq_open_with_context<T0>(&self, attr: crate::cq::CompletionQueueAttr, context: &mut T0) -> Result<crate::cq::CompletionQueue, crate::error::Error> {
        crate::cq::CompletionQueue::new_with_context(self, attr, context)
    }

    pub fn cntr_open(&self, attr: crate::cntr::CounterAttr) -> Result<crate::cntr::Counter, crate::error::Error> {
        crate::cntr::Counter::new(self, attr)
    }


    pub fn poll_open(&self, attr: crate::sync::PollAttr) -> Result<crate::sync::Poll, crate::error::Error> {
        crate::sync::Poll::new(self, attr)
    }

    pub fn av_open(&self, attr: crate::av::AddressVectorAttr) -> Result<crate::av::AddressVector, crate::error::Error> {
        crate::av::AddressVector::new(self, attr)
    }

    pub fn mr_reg<T0>(&self, buf: &[T0], acs: u64, offset: u64, requested_key: u64, flags: u64) -> Result<crate::mr::MemoryRegion, crate::error::Error> {
        crate::mr::MemoryRegion::from_buffer(self, buf, acs, offset, requested_key, flags)
    }

    pub fn mr_regv<T0>(&self,  iov : &crate::IoVec, count: usize, acs: u64, offset: u64, requested_key: u64, flags: u64) -> Result<crate::mr::MemoryRegion, crate::error::Error> {
        crate::mr::MemoryRegion::from_iovec(self, iov, count, acs, offset, requested_key, flags)
    }

    pub fn mr_regattr(&self, attr: crate::mr::MemoryRegionAttr ,  flags: u64) -> Result<crate::mr::MemoryRegion, crate::error::Error> {
        crate::mr::MemoryRegion::from_attr(self, attr,  flags)
    }

    pub fn map_raw(&self, base_addr: u64, raw_key: &mut u8, key_size: usize, key: &mut u64, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_mr_map_raw(self.c_domain, base_addr, raw_key as *mut u8, key_size, key as *mut u64, flags) };
        
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(())
        }
    }

    pub fn unmap_key(&self, key: u64) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_mr_unmap_key(self.c_domain, key) };


        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(())
        }
    }

    pub fn stx_context<T0>(&self, attr:crate::TxAttr , context: &mut T0) -> Result<crate::Stx, crate::error::Error> {
        crate::Stx::new(self, attr, context)
    }

    pub fn query_collective(&self, coll: crate::enums::CollectiveOp, mut attr: crate::CollectiveAttr, flags: u64) -> Result<(), crate::error::Error> {
        let err = unsafe { libfabric_sys::inlined_fi_query_collective(self.c_domain, coll.get_value(), attr.get_mut(), flags) };
    
        if err != 0 {
            Err(crate::error::Error::from_err_code((-err).try_into().unwrap()))
        }
        else {
            Ok(())
        }
    }

}

impl crate::FID for Domain {
    fn fid(&self) -> *mut libfabric_sys::fid {
        unsafe {&mut (*self.c_domain).fid }
    }
}

impl Drop for Domain {
    fn drop(&mut self) {
        debug_println!("Dropping domain");
        self.close().unwrap()
    }
}

//================== Domain attribute ==================//

#[derive(Clone, Debug)]
pub struct DomainAttr {
    pub(crate) c_attr : libfabric_sys::fi_domain_attr,
}

impl DomainAttr {

    pub fn new() -> Self {
        let c_attr = libfabric_sys::fi_domain_attr {
            domain: std::ptr::null_mut(),
            name: std::ptr::null_mut(),
            threading: crate::enums::Threading::UNSPEC.get_value(),
            control_progress: crate::enums::Progress::UNSPEC.get_value(),
            data_progress: crate::enums::Progress::UNSPEC.get_value(),
            resource_mgmt: crate::enums::ResourceMgmt::UNSPEC.get_value(),
            av_type: crate::enums::AddressVectorType::UNSPEC.get_value(),
            mr_mode: 0,
            mr_key_size: 0,
            cq_data_size: 0,
            cq_cnt: 0,
            ep_cnt: 0,
            tx_ctx_cnt: 0,
            rx_ctx_cnt: 0,
            max_ep_tx_ctx: 0,
            max_ep_rx_ctx: 0,
            max_ep_stx_ctx: 0,
            max_ep_srx_ctx: 0,
            cntr_cnt: 0,
            mr_iov_limit: 0,
            caps: 0,
            mode: 0,
            auth_key: std::ptr::null_mut(),
            auth_key_size: 0,
            max_err_data: 0,
            mr_cnt: 0,
            tclass: 0,         
        };
        Self { c_attr }
    }

    pub fn mode(mut self, mode: crate::enums::Mode) -> Self {
        self.c_attr.mode = mode.get_value();
        
        self
    }
    
    pub fn mr_mode(mut self, mr_mode: crate::enums::MrMode) -> Self {
        self.c_attr.mr_mode = mr_mode.get_value() as i32;
        self
    }

    pub fn threading(mut self, threading: crate::enums::Threading) -> Self {
        self.c_attr.threading = threading.get_value();

        self
    }

    pub fn resource_mgmt(mut self, res_mgmt: crate::enums::ResourceMgmt) -> Self {
        self.c_attr.resource_mgmt = res_mgmt.get_value();
        
        self
    }

    pub fn data_progress(mut self, data_progress: crate::enums::Progress) -> Self {
        self.c_attr.data_progress = data_progress.get_value();

        self
    }

    pub fn get_mode(&self) -> u64 {
        self.c_attr.mode 
    }

    pub fn get_mr_mode(&self) -> crate::enums::MrMode {
        crate::enums::MrMode::from_value(self.c_attr.mr_mode.try_into().unwrap())
    }

    pub fn get_av_type(&self) ->  crate::enums::AddressVectorType {
        crate::enums::AddressVectorType::from_value( self.c_attr.av_type)
    }

    pub fn get_data_progress(&self) -> crate::enums::Progress {
        
        crate::enums::Progress::from_value(self.c_attr.data_progress)
    }

    pub fn get_mr_iov_limit(&self) -> usize {
        self.c_attr.mr_iov_limit
    }

    pub fn get_cntr_cnt(&self) -> usize {
        self.c_attr.cntr_cnt
    }

    pub fn get_cq_data_size(&self) -> u64 {
        self.c_attr.cq_data_size as u64
    }

    pub(crate) fn get(&self) -> *const libfabric_sys::fi_domain_attr {
        &self.c_attr
    }

    pub(crate) fn get_mut(&mut self) -> *mut libfabric_sys::fi_domain_attr {
        &mut self.c_attr
    }
}

impl Default for DomainAttr {
    fn default() -> Self {
        Self::new()
    }
}

//================== Domain tests ==================//

#[cfg(test)]
mod tests {
    #[test]
    fn domain_test() {
        let info = crate::Info::new().request().unwrap();
        let entries = info.get();
        
        let fab = crate::fabric::Fabric::new(entries[0].fabric_attr.clone()).unwrap();
        let count = 10;
        let mut doms = Vec::new();
        for _ in 0..count {
            let domain = fab.domain(&entries[0]).unwrap();
            doms.push(domain);
        }
    }
}