use libfabric;
use std::cell::RefCell;

// struct ContextElement {
//     ctx: libfabric::Context,
//     id: usize,
// }

pub(crate) struct ContextBank {
    curr_ctx_id: usize,
    bank: std::collections::HashMap<usize, RefCell<libfabric::Context>>,
}

impl ContextBank {

    pub(crate) fn new() -> Self {
        Self {
            curr_ctx_id: 0,
            bank: std::collections::HashMap::new(),
        }
    }

    pub(crate) fn create(&mut self) -> usize {


        let res = RefCell::new(libfabric::Context::new());
        
        self.bank.insert(self.curr_ctx_id, res);
        let res = self.curr_ctx_id;
        self.curr_ctx_id += 1;

        res
    }

    pub(crate) fn get(&self, id: usize) -> Option<&RefCell<libfabric::Context>> {
        
        self.bank.get(&id)
    }

    #[allow(dead_code)]
    pub(crate) fn rm(&mut self, id: usize) -> Option<RefCell<libfabric::Context>>{
        
        self.bank.remove(&id)
    }
}