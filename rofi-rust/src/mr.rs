use std::cell::{OnceCell,RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::vec::Vec;
use crate::transport;
use libfabric::mr::{MemoryRegion, MemoryRegionDesc};

extern crate memmap;

pub struct MappedMemoryRegion { // [TODO] Make Drop remove this from the rofi memory table
    mem: Rc<RefCell<memmap::MmapMut>>,
    mr: libfabric::mr::MemoryRegion,
    desc: libfabric::mr::MemoryRegionDesc,
    key: u64,
    iovs: OnceCell<Vec<libfabric::RmaIoVec>>,
    range: std::ops::Range<usize>,
    pes_map: OnceCell<HashMap<usize, usize>>,
}

impl MappedMemoryRegion {

    pub(crate) fn new(mem: memmap::MmapMut, mr: libfabric::mr::MemoryRegion, desc: libfabric::mr::MemoryRegionDesc, key: u64) -> Self {
        let start =  mem.as_ptr() as usize;
        let end =   mem.last().unwrap() as *const u8 as usize;

        Self {
            mem: Rc::new(RefCell::new(mem)),
            mr,
            desc,
            key,
            iovs: OnceCell::new(),
            range: std::ops::Range {start, end},
            pes_map: OnceCell::new(),
        }
    }

    pub(crate) fn get_start(&self) -> usize {
        
        self.range.start
    }
    
    pub(crate) fn get_remote_start(&self, remote_id: usize) -> usize {
        
        let id = self.get_real_id(remote_id);
        self.iovs.get().unwrap()[id].get_address() as usize
    }

    pub(crate) fn set_iovs(&self, iovs: Vec<libfabric::RmaIoVec>) {
        self.iovs.set(iovs).unwrap();
        self.pes_map.set(HashMap::new()).unwrap();
    }

    pub(crate) fn set_sub_iovs(&self, iovs: Vec<libfabric::RmaIoVec>, pes: &[usize]) {
        self.iovs.set(iovs).expect("Could not set iovs");
        
        let mut id_to_iov_map = HashMap::new();
        
        for (i, pe) in pes.iter().enumerate() {
            id_to_iov_map.insert(*pe, i);
        }
        self.pes_map.set(id_to_iov_map).unwrap();
    }
    
    #[allow(dead_code)]
    pub(crate) fn get_remote_end(&self, remote_id: usize) -> usize {
        
        let id = self.get_real_id(remote_id);
        let start = self.iovs.get().unwrap()[id].get_address() as usize;
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
        self.iovs.get().unwrap()[id].get_key()
    }

    pub(crate) fn contains(&self, addr: usize) -> bool {

        self.range.contains(&addr)
    }

    #[allow(dead_code)]
    pub(crate) fn remote_contains(&self, remote_id: usize, addr: usize) -> bool {

        let id = self.get_real_id(remote_id);

        let remote_start = self.iovs.get().unwrap()[id].get_address() as usize;
        let remote_end = self.iovs.get().unwrap()[id].get_address() as usize + self.mem.borrow().len();

        if addr >= remote_start && addr < remote_end {
            return true;
        }

        false
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
        
        if self.pes_map.get().unwrap().is_empty() {
            
            remote_id
        }
        else {
            
            *self.pes_map.get().unwrap().get(&remote_id).unwrap_or_else(|| panic!("PE {} is not part of the sub allocation group", remote_id) )
        }
    }
}


impl Drop for MappedMemoryRegion {
    fn drop(&mut self) {
       
    }
}

pub(crate) struct MemoryRegionManager {

    mr_table: Vec<Rc<MappedMemoryRegion>>,
    mr_next_key: u64, 
}

impl MemoryRegionManager {

    pub(crate) fn new() -> Self {
        Self {
            mr_table: Vec::new(),
            mr_next_key: 0,
        }
    }

    pub(crate) fn alloc(&mut self, info: &libfabric::InfoEntry, domain: &libfabric::domain::Domain, ep: &libfabric::ep::Endpoint, size: usize) ->  Rc<MappedMemoryRegion> {
        let mem_size = if (page_size::get() - 1) & size != 0 { (size + page_size::get()) & !(page_size::get()-1) } else { size}; 

        let mut mem = memmap::MmapOptions::new().len(mem_size).map_anon().unwrap();
        mem.iter_mut().map(|x| *x = 0).count();
        let (mr, mr_desc, key) = transport::reg_mr(info, domain, ep, &mut mem, self.mr_next_key).unwrap();

        self.mr_next_key += 1;

        let res = Rc::new(MappedMemoryRegion::new(mem, mr, mr_desc, key));
        self.mr_table.push(res.clone());

        
        res.clone()
    }


    pub(crate) fn mr_get(&self, addr: usize) -> Option<Rc<MappedMemoryRegion >>{
        
        self.mr_table.iter().find(|x| x.contains(addr)).cloned()
    }

    #[allow(dead_code)]
    pub(crate) fn mr_get_from_remote(&self, addr: usize, remote_id: usize) -> Option<Rc<MappedMemoryRegion>> {

        self.mr_table.iter().find(|x| x.remote_contains(remote_id, addr)).cloned()
    }

    #[allow(dead_code)]
    pub(crate) fn mr_rm(&mut self, addr: usize) {
        
        let to_remove = self.mr_table.iter().position(|x| x.get_start() == addr).expect("Address to remove not found"); 
        self.mr_table.remove(to_remove);
    }
}