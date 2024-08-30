// use debug_print::debug_println;
// use libfabric::{async_::{av::{AddressVector, AddressVectorBuilder}, cq::{CompletionQueue, CompletionQueueBuilder, AsyncCompletionQueueImpl, AsyncReadCq}, ep::{Endpoint, EndpointBuilder}, eq::{AsyncReadEq}}, infocapsoptions::Caps, info::{InfoEntry, InfoCapsImpl}, cq::Completion, cntr::{CounterBuilder, Counter, WaitCntr, ReadCntr},  mr::{MemoryRegionKey, MemoryRegion, MemoryRegionBuilder}, error::Error, eq::ReadEq, ep::ActiveEndpoint, domain::Domain};




// fn need_mr_reg<I: Caps>(info: &InfoEntry<I>) -> bool {

//     if info.get_caps().is_rma() || info.get_caps().is_atomic() {
//         true
//     }
//     else {
//         info.get_domain_attr().get_mr_mode().is_local()
//     }
// }

// fn rma_write_target_allowed(caps: &InfoCapsImpl) -> bool {
//     if caps.is_rma() || caps.is_atomic() {
//         if caps.is_remote_write() {
//             return true
//         }
//         else {
//             return ! (caps.is_read() || caps.is_write() || caps.is_remote_write());
//         }
//     }

//     false
// }

// fn rma_read_target_allowed(caps: &InfoCapsImpl) -> bool {
//     if caps.is_rma() || caps.is_atomic() {
//         if caps.is_remote_read() {
//             return true
//         }
//         else {
//             return ! (caps.is_read() || caps.is_write() || caps.is_remote_write());
//         }
//     }

//     false
// }

// fn check_mr_local_flag<I: Caps>(info: &InfoEntry<I>) -> bool {
//     info.get_mode().is_local_mr() || info.get_domain_attr().get_mr_mode().is_local()
// }

// pub fn info_to_mr_attr<'a, I: Caps>(info: &InfoEntry<I>, domain: &Domain, buff: &'a [u8]) -> MemoryRegionBuilder<'a, u8>{

//     let mut mr_region =  MemoryRegionBuilder::new(buff);

//     if check_mr_local_flag(info) {
//         if info.get_caps().is_msg() || info.get_caps().is_tagged() {
//             let mut temp = info.get_caps().is_send();
//             if temp {
//                 mr_region = mr_region.access_send();
//             }
//             temp |= info.get_caps().is_recv();
//             if temp {
//                 mr_region = mr_region.access_recv();
//             }
//             if !temp {
//                 mr_region = mr_region.access_send().access_recv();
//             }
//         }
//     }
//     else if info.get_caps().is_rma() || info.get_caps().is_atomic() {
//         if rma_read_target_allowed(info.get_caps()) {
//             mr_region = mr_region.access_remote_read();
//         }
//         if rma_write_target_allowed(info.get_caps()) {
//             mr_region = mr_region.access_remote_write();
//         }
//     }

//     mr_region
// }

// macro_rules!  post{
//     ($post_fn:ident, $ep:expr, $( $x:expr),* ) => {
//         loop {
//             let ret = $ep.$post_fn($($x,)*).await;
//             if ret.is_ok() {
//                 break;
//             }
//             else if let Err(ref err) = ret {
//                 if !matches!(err.kind, libfabric::error::ErrorKind::TryAgain) {
//                     match &err.kind {
//                         ErrorInQueue(e) => {
//                             match e {
//                                 libfabric::error::QueueError::Completion(entry) => {
//                                     panic!("Completion erro {}", entry.error())
//                                 }
//                                 _ => todo!()
//                             }
//                         },
//                         _ => {ret.unwrap();},
//                     }
//                     panic!("Unexpected error in post_rma ");
//                 }

//             }
//         }
//     };
// }

// pub(crate) use post;

// use crate::rofi::CntrOptDefault;
