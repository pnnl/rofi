use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use libfabric::mr::{MemoryRegion, MemoryRegionDesc};

extern crate memmap;

pub struct MappedMemoryRegion { // [TODO] Make Drop remove this from the rofi memory table
    mem: Rc<RefCell<memmap::MmapMut>>,
    mr: libfabric::mr::MemoryRegion,
    desc: libfabric::mr::MemoryRegionDesc,
    key: u64,
    iovs: Vec<libfabric::RmaIoVec>,
    range: std::ops::Range<usize>,
    pes_map: HashMap<usize, usize>,
}

impl MappedMemoryRegion {

    pub(crate) fn new(mem: memmap::MmapMut, mr: libfabric::mr::MemoryRegion, desc: libfabric::mr::MemoryRegionDesc, key: u64, iovs: Vec<libfabric::RmaIoVec>) -> Self {
        let start =  mem.as_ptr() as usize;
        let end =   mem.last().unwrap() as *const u8 as usize;

        Self {
            mem: Rc::new(RefCell::new(mem)),
            mr,
            desc,
            key,
            iovs,
            range: std::ops::Range {start, end},
            pes_map: HashMap::new(),
        }
    }

    pub(crate) fn new_sub(mem: memmap::MmapMut, mr: libfabric::mr::MemoryRegion, desc: libfabric::mr::MemoryRegionDesc, key: u64, iovs: Vec<libfabric::RmaIoVec>, pes: &[usize]) -> Self {
        let start =  mem.as_ptr() as usize;
        let end =   mem.last().unwrap() as *const u8 as usize;
        let mut id_to_iov_map = HashMap::new();
        
        for (i, pe) in pes.iter().enumerate() {
            id_to_iov_map.insert(*pe, i);
        }

        Self {
            mem: Rc::new(RefCell::new(mem)),
            mr,
            desc,
            key,
            iovs,
            range: std::ops::Range {start, end},
            pes_map: id_to_iov_map,
        }
    }

    pub(crate) fn get_start(&self) -> usize {
        
        self.range.start
    }
    
    pub(crate) fn get_remote_start(&self, remote_id: usize) -> usize {
        
        let id = self.get_real_id(remote_id);
        self.iovs[id].get_address() as usize
    }
    
    #[allow(dead_code)]
    pub(crate) fn get_remote_end(&self, remote_id: usize) -> usize {
        
        let id = self.get_real_id(remote_id);
        let start = self.iovs[id].get_address() as usize;
        start + self.mem.borrow().len()
    }

    #[allow(dead_code)]
    pub(crate) fn get_end(&self) -> usize {
        
        self.range.end
    }

    #[allow(dead_code)]
    pub(crate) fn get_key(&self) -> u64 {
        
        self.key
    }

    pub(crate) fn get_remote_key(&self, remote_id: usize) -> u64 {
        
        let id = self.get_real_id(remote_id);
        self.iovs[id].get_key()
    }

    pub(crate) fn contains(&self, addr: usize) -> bool {

        self.range.contains(&addr)
    }

    #[allow(dead_code)]
    pub(crate) fn remote_contains(&self, remote_id: usize, addr: usize) -> bool {

        let id = self.get_real_id(remote_id);

        let remote_start = self.iovs[id].get_address() as usize;
        let remote_end = self.iovs[id].get_address() as usize + self.mem.borrow().len();

        if addr >= remote_start && addr < remote_end {
            return true;
        }

        return false;
    }

    pub(crate) fn get_mem(&self) -> &Rc<RefCell<memmap::MmapMut>>{
        
        &self.mem
    }

    pub(crate) fn get_mr_desc(&self) -> MemoryRegionDesc {
        
        self.desc.clone()
    }

    #[allow(dead_code)]
    pub(crate) fn get_mr(&self) -> &MemoryRegion {
        
        &self.mr
    }

        
    fn get_real_id(&self, remote_id: usize) -> usize {
        
        if self.pes_map.is_empty() {
            
            remote_id
        }
        else {
            
            *self.pes_map.get(&remote_id).expect(&format!("PE {} is not part of the sub allocation group", remote_id) ) as usize
        }
    }

}