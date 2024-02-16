use std::cell::RefCell;
use std::rc::Rc;


pub(crate) struct ContextBank {
    curr_ctx_id: usize,
    bank: std::collections::HashMap<usize, Rc<RefCell<libfabric::Context>>>,
}

impl ContextBank {

    pub(crate) fn new() -> Self {
        Self {
            curr_ctx_id: 0,
            bank: std::collections::HashMap::new(),
        }
    }

    pub(crate) fn create(&mut self) -> Rc<RefCell<libfabric::Context>> {


        let res = Rc::new(RefCell::new(libfabric::Context::new()));
        
        self.bank.insert(self.curr_ctx_id, res);
        // let res = self.curr_ctx_id;
        
        self.curr_ctx_id += 1;
        self.bank.get(&(self.curr_ctx_id-1)).unwrap().clone()
    }

}