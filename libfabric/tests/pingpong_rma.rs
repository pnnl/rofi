pub mod common; // Public to supress lint warnings (unused function)
use common::IP;

use libfabric::info::{Info, Version};
use prefix::{call, define_test, HintsCaps};

#[cfg(any(feature = "use-async-std", feature = "use-tokio"))]
pub mod async_;
pub mod sync_; // Public to supress lint warnings (unused function) // Public to supress lint warnings (unused function)

use sync_ as prefix;

define_test!(pp_server_rma, async_pp_server_rma, {
    let mut gl_ctx = prefix::TestsGlobalCtx::new();

    let info = Info::new(&Version {
        major: 1,
        minor: 19,
    })
    .enter_hints()
        .mode(libfabric::enums::Mode::new().context())
        .enter_ep_attr()
            .type_(libfabric::enums::EndpointType::Rdm)
        .leave_ep_attr()
        .enter_domain_attr()
            .threading(libfabric::enums::Threading::Domain)
            .mr_mode(
                libfabric::enums::MrMode::new()
                    .prov_key()
                    .allocated()
                    .virt_addr()
                    .local()
                    .endpoint()
                    .raw(),
            )
            .resource_mgmt(libfabric::enums::ResourceMgmt::Enabled)
        .leave_domain_attr()
        .enter_tx_attr()
            .traffic_class(libfabric::enums::TrafficClass::LowLatency)
        .leave_tx_attr()
        .addr_format(libfabric::enums::AddressFormat::Unspec);

    let hintscaps = if true {
        HintsCaps::Msg(info.caps(libfabric::infocapsoptions::InfoCaps::new().msg().rma()))
    } else {
        HintsCaps::Tagged(info.caps(libfabric::infocapsoptions::InfoCaps::new().tagged().rma()))
    };

    let (infocap, ep, domain, cq_type, tx_cntr, rx_cntr, mut mr, _av, mut mr_desc) = call!(
        prefix::ft_init_fabric,
        hintscaps,
        &mut gl_ctx,
        "".to_owned(),
        "9222".to_owned(),
        true
    );

    match infocap {
        prefix::InfoWithCaps::Msg(entry) => {
            let remote = call!(
                prefix::ft_exchange_keys,
                &entry,
                &mut gl_ctx,
                mr.as_mut().unwrap(),
                &cq_type,
                &tx_cntr,
                &rx_cntr,
                &domain,
                &ep,
                &mut mr_desc
            );

            let test_sizes = gl_ctx.test_sizes.clone();
            for msg_size in test_sizes {
                call!(
                    prefix::pingpong_rma,
                    &entry,
                    &mut gl_ctx,
                    &cq_type,
                    &tx_cntr,
                    &rx_cntr,
                    &ep,
                    &mut mr_desc,
                    prefix::RmaOp::RMA_WRITE,
                    &remote,
                    100,
                    10,
                    msg_size,
                    true
                );
            }

            call!(
                prefix::ft_finalize,
                &entry,
                &mut gl_ctx,
                &ep,
                &cq_type,
                &tx_cntr,
                &rx_cntr,
                &mut mr_desc
            );
        }
        prefix::InfoWithCaps::Tagged(entry) => {
            let remote = call!(
                prefix::ft_exchange_keys,
                &entry,
                &mut gl_ctx,
                mr.as_mut().unwrap(),
                &cq_type,
                &tx_cntr,
                &rx_cntr,
                &domain,
                &ep,
                &mut mr_desc
            );

            let test_sizes = gl_ctx.test_sizes.clone();
            for msg_size in test_sizes {
                call!(
                    prefix::pingpong_rma,
                    &entry,
                    &mut gl_ctx,
                    &cq_type,
                    &tx_cntr,
                    &rx_cntr,
                    &ep,
                    &mut mr_desc,
                    prefix::RmaOp::RMA_WRITE,
                    &remote,
                    100,
                    10,
                    msg_size,
                    true
                );
            }

            call!(
                prefix::ft_finalize,
                &entry,
                &mut gl_ctx,
                &ep,
                &cq_type,
                &tx_cntr,
                &rx_cntr,
                &mut mr_desc
            );
        }
    }
});

define_test!(pp_client_rma, async_pp_client_rma, {
    let hostname = std::process::Command::new("hostname")
        .output()
        .expect("Failed to execute hostname")
        .stdout;
    let hostname = String::from_utf8(hostname[2..].to_vec()).unwrap();
    let ip = "172.17.110.".to_string() + &hostname;
    let mut gl_ctx = prefix::TestsGlobalCtx::new();

    let info = Info::new(&Version {
        major: 1,
        minor: 19,
    })
    .enter_hints()
        .mode(libfabric::enums::Mode::new().context())
        .enter_ep_attr()
            .type_(libfabric::enums::EndpointType::Rdm)
        .leave_ep_attr()
        .enter_domain_attr()
            .threading(libfabric::enums::Threading::Domain)
            .mr_mode(
                libfabric::enums::MrMode::new()
                    .prov_key()
                    .allocated()
                    .virt_addr()
                    .local()
                    .endpoint()
                    .raw(),
            )
            .resource_mgmt(libfabric::enums::ResourceMgmt::Enabled)
        .leave_domain_attr()
        .enter_tx_attr()
            .traffic_class(libfabric::enums::TrafficClass::LowLatency)
        .leave_tx_attr()
        .addr_format(libfabric::enums::AddressFormat::Unspec);

    let hintscaps = if true {
        HintsCaps::Msg(info.caps(libfabric::infocapsoptions::InfoCaps::new().msg().rma()))
    } else {
        HintsCaps::Tagged(info.caps(libfabric::infocapsoptions::InfoCaps::new().tagged().rma()))
    };

    let (infocap, ep, domain, cq_type, tx_cntr, rx_cntr, mut mr, _av, mut mr_desc) = call!(
        prefix::ft_init_fabric,
        hintscaps,
        &mut gl_ctx,
        ip.strip_suffix("\n").unwrap_or(&ip).to_owned(),
        "9222".to_owned(),
        false
    );

    match infocap {
        prefix::InfoWithCaps::Msg(entry) => {
            let remote = call!(
                prefix::ft_exchange_keys,
                &entry,
                &mut gl_ctx,
                mr.as_mut().unwrap(),
                &cq_type,
                &tx_cntr,
                &rx_cntr,
                &domain,
                &ep,
                &mut mr_desc
            );

            let test_sizes = gl_ctx.test_sizes.clone();
            for msg_size in test_sizes {
                call!(
                    prefix::pingpong_rma,
                    &entry,
                    &mut gl_ctx,
                    &cq_type,
                    &tx_cntr,
                    &rx_cntr,
                    &ep,
                    &mut mr_desc,
                    prefix::RmaOp::RMA_WRITE,
                    &remote,
                    100,
                    10,
                    msg_size,
                    false
                );
            }

            call!(
                prefix::ft_finalize,
                &entry,
                &mut gl_ctx,
                &ep,
                &cq_type,
                &tx_cntr,
                &rx_cntr,
                &mut mr_desc
            );
        }
        prefix::InfoWithCaps::Tagged(entry) => {
            let remote = call!(
                prefix::ft_exchange_keys,
                &entry,
                &mut gl_ctx,
                mr.as_mut().unwrap(),
                &cq_type,
                &tx_cntr,
                &rx_cntr,
                &domain,
                &ep,
                &mut mr_desc
            );
            let test_sizes = gl_ctx.test_sizes.clone();
            for msg_size in test_sizes {
                call!(
                    prefix::pingpong_rma,
                    &entry,
                    &mut gl_ctx,
                    &cq_type,
                    &tx_cntr,
                    &rx_cntr,
                    &ep,
                    &mut mr_desc,
                    prefix::RmaOp::RMA_WRITE,
                    &remote,
                    100,
                    10,
                    msg_size,
                    false
                );
            }

            call!(
                prefix::ft_finalize,
                &entry,
                &mut gl_ctx,
                &ep,
                &cq_type,
                &tx_cntr,
                &rx_cntr,
                &mut mr_desc
            );
            // drop(domain);
        }
    }
});
