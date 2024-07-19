pub mod sync_; // Public to supress lint warnings (unused function)
#[cfg(any(feature="use-async-std", feature="use-tokio"))]
pub mod async_; // Public to supress lint warnings (unused function)
pub mod common; // Public to supress lint warnings (unused function)
use common::IP;

use sync_ as prefix;
use prefix::{HintsCaps, define_test, call};




// To run the following tests do:
// 1. export FI_LOG_LEVEL="info" . 
// 2. Run the server (e.g. cargo test pp_server_msg -- --ignored --nocapture) 
//    There will be a large number of info printed. What we need is the last line with: listening on: fi_sockaddr_in:// <ip:port>
// 3. Copy the ip, port of the previous step
// 4. On the client (e.g. pp_client_msg) change  ft_client_connect node(<ip>) and service(<port>) to service and port of the copied ones
// 5. Run client (e.g. cargo test pp_client_msg -- --ignored --nocapture) 

define_test!(pp_server_rdm_tagged, asyn_pp_server_rdm_tagged, {
    let mut gl_ctx = prefix::TestsGlobalCtx::new();

    let mut ep_attr = libfabric::ep::EndpointAttr::new();
        ep_attr.ep_type(libfabric::enums::EndpointType::Rdm);

    let mut dom_attr = libfabric::domain::DomainAttr::new();
        dom_attr
        .threading(libfabric::enums::Threading::Domain)
        .mr_mode(libfabric::enums::MrMode::new().prov_key().allocated().virt_addr().local().endpoint().raw());
    
    let mut tx_attr = libfabric::xcontext::TxAttr::new();
        tx_attr.tclass(libfabric::enums::TClass::LowLatency);



    let hintscaps = if false {
        HintsCaps::Msg(
            libfabric::info::InfoHints::new()
            .mode(libfabric::enums::Mode::new().context())
            .ep_attr(ep_attr)
            .caps(
                libfabric::infocapsoptions::InfoCaps::new()
                .msg()
            )
            .domain_attr(dom_attr)
            .tx_attr(tx_attr)
            .addr_format(libfabric::enums::AddressFormat::Unspec)
        )
    }
    else {
        HintsCaps::Tagged(
            libfabric::info::InfoHints::new()
            .mode(libfabric::enums::Mode::new().context())
            .ep_attr(ep_attr)
            .caps(
                libfabric::infocapsoptions::InfoCaps::new()
                .tagged()
            )
            .domain_attr(dom_attr)
            .tx_attr(tx_attr)
            .addr_format(libfabric::enums::AddressFormat::Unspec)
        )
    };

    let (info, ep, _domain, tx_cq, rx_cq, tx_cntr, rx_cntr, _mr, _av, mut mr_desc) = 
        call!(prefix::ft_init_fabric,hintscaps, &mut gl_ctx, "".to_owned(), "9222".to_owned(), true);
    
    match info {
        prefix::InfoWithCaps::Msg(info) => {
            let entries = info.get();

            if entries.is_empty() {
                panic!("No entires in fi_info");
            }

            let test_sizes = gl_ctx.test_sizes.clone();
            let inject_size = entries[0].get_tx_attr().get_inject_size();
            for msg_size in test_sizes {
                call!(prefix::pingpong,inject_size, &mut gl_ctx, &tx_cq, &rx_cq, &tx_cntr, &rx_cntr, &ep, &mut mr_desc, 100, 10, msg_size, true);
            }

            call!(prefix::ft_finalize,&entries[0], &mut gl_ctx, &ep, &tx_cq, &rx_cq, &tx_cntr, &rx_cntr, &mut mr_desc);
            
        }
        prefix::InfoWithCaps::Tagged(info) => {
            let entries = info.get();

            if entries.is_empty() {
                panic!("No entires in fi_info");
            }

            let test_sizes = gl_ctx.test_sizes.clone();
            let inject_size = entries[0].get_tx_attr().get_inject_size();
            for msg_size in test_sizes {
                call!(prefix::pingpong,inject_size, &mut gl_ctx, &tx_cq, &rx_cq, &tx_cntr, &rx_cntr, &ep, &mut mr_desc, 100, 10, msg_size, true);
            }

            call!(prefix::ft_finalize,&entries[0], &mut gl_ctx, &ep, &tx_cq, &rx_cq, &tx_cntr, &rx_cntr, &mut mr_desc);
        }
    }
});



define_test!(pp_client_rdm_tagged, async_pp_client_rdm_tagged, {
    let mut gl_ctx = prefix::TestsGlobalCtx::new();

    let mut ep_attr = libfabric::ep::EndpointAttr::new();
        ep_attr.ep_type(libfabric::enums::EndpointType::Rdm);

    let mut dom_attr = libfabric::domain::DomainAttr::new();
        dom_attr
        .threading(libfabric::enums::Threading::Domain)
        .mr_mode(libfabric::enums::MrMode::new().prov_key().allocated().virt_addr().local().endpoint().raw());
    

    let mut tx_attr = libfabric::xcontext::TxAttr::new();
        tx_attr.tclass(libfabric::enums::TClass::LowLatency);
    
    let hintscaps = if false {
            HintsCaps::Msg(
                libfabric::info::InfoHints::new()
                .mode(libfabric::enums::Mode::new().context())
                .ep_attr(ep_attr)
                .caps(
                    libfabric::infocapsoptions::InfoCaps::new()
                    .msg()
                )
                .domain_attr(dom_attr)
                .tx_attr(tx_attr)
                .addr_format(libfabric::enums::AddressFormat::Unspec)
            )
        }
        else {
            HintsCaps::Tagged(
                libfabric::info::InfoHints::new()
                .mode(libfabric::enums::Mode::new().context())
                .ep_attr(ep_attr)
                .caps(
                    libfabric::infocapsoptions::InfoCaps::new()
                    .tagged()
                )
                .domain_attr(dom_attr)
                .tx_attr(tx_attr)
                .addr_format(libfabric::enums::AddressFormat::Unspec)
            )
        };

    let (info, ep, _domain, tx_cq, rx_cq, tx_cntr, rx_cntr, _mr, _av, mut mr_desc) = 
        call!(prefix::ft_init_fabric,hintscaps, &mut gl_ctx, IP.to_owned(), "9222".to_owned(), false);

    match info {
        prefix::InfoWithCaps::Msg(info) => {
            let entries = info.get();

            if entries.is_empty() {
                panic!("No entires in fi_info");
            }

            let test_sizes = gl_ctx.test_sizes.clone();
            let inject_size = entries[0].get_tx_attr().get_inject_size();
            for msg_size in test_sizes {
                call!(prefix::pingpong,inject_size, &mut gl_ctx, &tx_cq, &rx_cq, &tx_cntr, &rx_cntr, &ep, &mut mr_desc, 100, 10, msg_size, false);
            }

            call!(prefix::ft_finalize,&entries[0], &mut gl_ctx, &ep, &tx_cq, &rx_cq, &tx_cntr, &rx_cntr, &mut mr_desc);
            
        }
        prefix::InfoWithCaps::Tagged(info) => {
            let entries = info.get();

            if entries.is_empty() {
                panic!("No entires in fi_info");
            }

            let test_sizes = gl_ctx.test_sizes.clone();
            let inject_size = entries[0].get_tx_attr().get_inject_size();
            for msg_size in test_sizes {
                call!(prefix::pingpong,inject_size, &mut gl_ctx, &tx_cq, &rx_cq, &tx_cntr, &rx_cntr, &ep, &mut mr_desc, 100, 10, msg_size, false);
            }

            call!(prefix::ft_finalize,&entries[0], &mut gl_ctx, &ep, &tx_cq, &rx_cq, &tx_cntr, &rx_cntr, &mut mr_desc);
        }
    }
});