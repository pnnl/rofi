use std::{collections::HashMap, sync::atomic::{AtomicUsize, Ordering}};

use parking_lot::RwLock;

pub mod rofi;
#[cfg(feature="async-rofi")]
pub mod async_;

enum BarrierImpl {
    Uninit,
    Collective(libfabric::comm::collective::MulticastGroupCollective),
    Manual(usize, AtomicUsize),
}


pub(crate) struct AllocInfoManager {
    pub(crate) mr_info_table: RwLock<Vec<AllocInfo>>,
    mr_next_key: AtomicUsize,
    page_size: usize,
}

impl AllocInfoManager {
    pub(crate) fn new() -> Self {
        Self {
            mr_info_table: RwLock::new(Vec::new()),
            mr_next_key: AtomicUsize::new(0),
            page_size: page_size::get(),
        }
    }

    pub(crate) fn insert(&self, alloc: AllocInfo) {
        self.mr_info_table.write().push(alloc)
    }

    pub(crate) fn remove(&self, mem_addr: &usize) {
        let mut table = self.mr_info_table.write();
        let idx = table.iter().position(|e| {
            e.start() == *mem_addr
        })
        .expect("Error! Invalid memory address");

        table.remove(idx);
    }

    pub(crate) fn local_addr(&self, remote_pe: &usize, remote_addr: &usize) -> Option<usize> {

        let table = self.mr_info_table.read();
        if let Some(alloc_info) =  table.iter()
            .find(|x| x.remote_contains(remote_pe, remote_addr))
        {
            let offset = remote_addr - alloc_info.remote_start(remote_pe);
            Some(alloc_info.start() + offset)
        }
        else {
            None
        }
    }

    pub(crate) fn remote_addr(&self, remote_pe: &usize, local_addr: &usize) -> Option<usize> {

        let table = self.mr_info_table.read();
        if let Some(alloc_info ) = table.iter().find(|x|  x.contains(local_addr)) {
            if let Some(remote_alloc_info) = alloc_info.remote_allocs.get(remote_pe) {
                let offset = local_addr - alloc_info.start();
                Some(remote_alloc_info.start() + offset)
            }
            else {
                None
            }
        }
        else {
            None
        }
    }

    pub(crate) fn page_size(&self) -> usize {
        self.page_size
    }

    pub(crate) fn next_key(&self) -> usize{
        self.mr_next_key.fetch_add(1, Ordering::SeqCst)
    }
}

#[derive(Clone)]
pub(crate) struct RemoteAllocInfo {
    key: libfabric::mr::MappedMemoryRegionKey,
    range: std::ops::Range<usize>,
}

impl RemoteAllocInfo {

    pub(crate) fn from_rma_iov(rma_iov: libfabric::iovec::RmaIoVec, key: libfabric::mr::MappedMemoryRegionKey) -> Self {
        let start = rma_iov.get_address() as usize;
        let end = start + rma_iov.get_len();

        Self {
            key,
            range: std::ops::Range {start, end},
        }
    }

    #[allow(dead_code)]
    pub(crate) fn new(range: std::ops::Range<usize>, key: libfabric::mr::MappedMemoryRegionKey) -> Self {
        Self {
            key,
            range,
        }
    }

    pub(crate) fn start(&self) -> usize {
        self.range.start
    }

    pub(crate) fn end(&self) -> usize {
        self.range.end
    }

    pub(crate) fn key(&self) -> &libfabric::mr::MappedMemoryRegionKey {
        &self.key
    }

    pub(crate) fn contains(&self, addr: &usize) -> bool {
        self.range.contains(addr)
    }
}

pub(crate) struct AllocInfo {
    _mem: memmap::MmapMut,
    mr: libfabric::mr::MemoryRegion,
    desc: libfabric::mr::MemoryRegionDesc,
    key: libfabric::mr::MemoryRegionKey,
    range: std::ops::Range<usize>,
    remote_allocs: HashMap<usize, RemoteAllocInfo>,
}

impl AllocInfo {
    pub(crate) fn new(mem: memmap::MmapMut, mr: libfabric::mr::MemoryRegion, remote_allocs: HashMap<usize, RemoteAllocInfo>) -> Result<Self, libfabric::error::Error> {
        let start =  mem.as_ptr() as usize;
        let end =   start + mem.len();
        let desc = mr.description();
        let key = mr.key()?;

        Ok(Self {
            _mem: mem,
            mr,
            desc,
            key,
            range: std::ops::Range {start, end},
            remote_allocs,
        })
    }

    pub(crate) fn start(&self) -> usize {
        self.range.start
    }
    
    #[allow(dead_code)]
    pub(crate) fn remote_start(&self, remote_id: &usize) -> usize {
        self.remote_allocs.get(remote_id)
            .expect(&format!("PE {} is not part of the sub allocation group", remote_id))
            .start()
    }

    #[allow(dead_code)]
    pub(crate) fn remote_end(&self, remote_id: &usize) -> usize {
        self.remote_allocs.get(remote_id)
            .expect(&format!("PE {} is not part of the sub allocation group", remote_id))
            .end()
    }

    #[allow(dead_code)]
    pub(crate) fn end(&self) -> usize {
        self.range.end
    }

    #[allow(dead_code)]
    pub(crate) fn key(&self) -> &libfabric::mr::MemoryRegionKey {
        &self.key
    }

    #[allow(dead_code)]
    pub(crate) fn remote_key(&self, remote_id: &usize) -> &libfabric::mr::MappedMemoryRegionKey {
        self.remote_allocs.get(remote_id)
            .expect(&format!("PE {} is not part of the sub allocation group", remote_id))
            .key()
    }

    pub(crate) fn contains(&self, addr: &usize) -> bool {
        self.range.contains(addr)
    }

    #[allow(dead_code)]
    pub(crate) fn remote_contains(&self, remote_id: &usize, addr: &usize) -> bool {
        self.remote_allocs.get(remote_id)
            .expect(&format!("PE {} is not part of the sub allocation group", remote_id))
            .contains(&addr)
    }

    pub(crate) fn remote_info(&self, remote_pe: &usize) -> Option<RemoteAllocInfo> {
        self.remote_allocs.get(remote_pe).cloned()
    }

    pub(crate) fn mr_desc(&self) -> libfabric::mr::MemoryRegionDesc {
        
        self.desc.clone()
    }

    #[allow(dead_code)]
    pub(crate) fn mr(&self) -> &libfabric::mr::MemoryRegion {
        
        &self.mr
    }
}