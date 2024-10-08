use crate::async_::ep::AsyncTxEp;
use crate::comm::rma::{ConnectedWriteEp, ReadEpImpl, WriteEp, WriteEpImpl};
use crate::conn_ep::ConnectedEp;
use crate::connless_ep::ConnlessEp;
use crate::ep::{Connected, Connectionless, EndpointImplBase, EpMrReq};
use crate::infocapsoptions::RmaCap;
use crate::msg::{
    MsgRma, MsgRmaConnected, MsgRmaConnectedMr, MsgRmaConnectedMut, MsgRmaConnectedMutMr, MsgRmaMr,
    MsgRmaMut, MsgRmaMutMr,
};
use crate::utils::MsgType;
use crate::{
    async_::{cq::AsyncReadCq, eq::AsyncReadEq, AsyncCtx},
    cq::SingleCompletion,
    enums::{ReadMsgOptions, WriteMsgOptions},
    ep::EndpointBase,
    infocapsoptions::{ReadMod, WriteMod},
    mr::{DataDescriptor, MappedMemoryRegionKey},
    MappedAddress,
};

pub(crate) trait AsyncReadEpImpl: AsyncTxEp + ReadEpImpl {
    async unsafe fn read_async_impl<T>(
        &self,
        buf: &mut [T],
        desc: &mut impl DataDescriptor,
        src_mapped_addr: Option<&MappedAddress>,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        user_ctx: Option<*mut std::ffi::c_void>,
    ) -> Result<SingleCompletion, crate::error::Error> {
        let mut async_ctx = AsyncCtx { user_ctx };
        self.read_impl(
            buf,
            desc,
            src_mapped_addr,
            mem_addr,
            mapped_key,
            Some(&mut async_ctx as *mut AsyncCtx as *mut std::ffi::c_void),
        )?;
        let cq = self.retrieve_tx_cq();
        cq.wait_for_ctx_async(&mut async_ctx).await
    }

    async unsafe fn readv_async_impl<'a>(
        &self,
        iov: &[crate::iovec::IoVecMut<'a>],
        desc: &mut [impl DataDescriptor],
        src_mapped_addr: Option<&MappedAddress>,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        user_ctx: Option<*mut std::ffi::c_void>,
    ) -> Result<SingleCompletion, crate::error::Error> {
        let mut async_ctx = AsyncCtx { user_ctx };
        self.readv_impl(
            iov,
            desc,
            src_mapped_addr,
            mem_addr,
            mapped_key,
            Some(&mut async_ctx as *mut AsyncCtx as *mut std::ffi::c_void),
        )?;
        let cq = self.retrieve_tx_cq();
        cq.wait_for_ctx_async(&mut async_ctx).await
    }

    async unsafe fn readmsg_async_impl<'a>(
        &self,
        mut msg: MsgType<
            &mut crate::msg::MsgRmaMut<'a>,
            &mut crate::msg::MsgRmaConnectedMut<'a>,
            &mut crate::msg::MsgRmaMutMr<'a>,
            &mut crate::msg::MsgRmaConnectedMutMr<'a>,
        >,
        options: ReadMsgOptions,
    ) -> Result<SingleCompletion, crate::error::Error> {
        let real_user_ctx = match msg {
            MsgType::ConnectionlessMsg(ref mut msg) => msg.get_mut().context,
            MsgType::ConnectedMsg(ref mut msg) => msg.get_mut().context,
            MsgType::ConnectionlessMrMsg(ref mut msg) => msg.get_mut().context,
            MsgType::ConnectedMrMsg(ref mut msg) => msg.get_mut().context,
        };

        let mut async_ctx = AsyncCtx {
            user_ctx: if real_user_ctx.is_null() {
                None
            } else {
                Some(real_user_ctx)
            },
        };

        let imm_msg = match msg {
            MsgType::ConnectionlessMsg(ref mut msg) => {
                msg.get_mut().context = (&mut async_ctx as *mut AsyncCtx).cast();
                MsgType::<&MsgRmaMut, &MsgRmaConnectedMut, &MsgRmaMutMr, &MsgRmaConnectedMutMr>::ConnectionlessMsg(msg)
            }
            MsgType::ConnectedMsg(ref mut msg) => {
                msg.get_mut().context = (&mut async_ctx as *mut AsyncCtx).cast();
                MsgType::<&MsgRmaMut, &MsgRmaConnectedMut, &MsgRmaMutMr, &MsgRmaConnectedMutMr>::ConnectedMsg(msg)
            }
            MsgType::ConnectedMrMsg(ref mut msg) => {
                msg.get_mut().context = (&mut async_ctx as *mut AsyncCtx).cast();
                MsgType::<&MsgRmaMut, &MsgRmaConnectedMut, &MsgRmaMutMr, &MsgRmaConnectedMutMr>::ConnectedMrMsg(msg)
            }
            MsgType::ConnectionlessMrMsg(ref mut msg) => {
                msg.get_mut().context = (&mut async_ctx as *mut AsyncCtx).cast();
                MsgType::<&MsgRmaMut, &MsgRmaConnectedMut, &MsgRmaMutMr, &MsgRmaConnectedMutMr>::ConnectionlessMrMsg(msg)
            }
        };
        let err = self.readmsg_impl(imm_msg, options);

        if let Err(err) = err {
            match msg {
                MsgType::ConnectionlessMsg(msg) => msg.get_mut().context = real_user_ctx,
                MsgType::ConnectedMsg(msg) => msg.get_mut().context = real_user_ctx,
                MsgType::ConnectionlessMrMsg(msg) => msg.get_mut().context = real_user_ctx,
                MsgType::ConnectedMrMsg(msg) => msg.get_mut().context = real_user_ctx,
            }
            return Err(err);
        }

        let cq = self.retrieve_tx_cq();
        let res = cq.wait_for_ctx_async(&mut async_ctx).await;
        match msg {
            MsgType::ConnectionlessMsg(ref mut msg) => msg.get_mut().context = real_user_ctx,
            MsgType::ConnectedMsg(ref mut msg) => msg.get_mut().context = real_user_ctx,
            MsgType::ConnectionlessMrMsg(ref mut msg) => msg.get_mut().context = real_user_ctx,
            MsgType::ConnectedMrMsg(ref mut msg) => msg.get_mut().context = real_user_ctx,
        }
        res
    }
}

pub trait AsyncReadEp {
    /// Async version of [crate::comm::rma::ReadEp::read_from]
    /// # Safety
    /// See [crate::comm::rma::ReadEp::read_from]
    unsafe fn read_from_async<T0>(
        &self,
        buf: &mut [T0],
        desc: &mut impl DataDescriptor,
        src_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::ReadEp::read_from_with_context]
    /// # Safety
    /// See [crate::comm::rma::ReadEp::read_from_with_context]
    unsafe fn read_from_with_context_async<T, T0>(
        &self,
        buf: &mut [T],
        desc: &mut impl DataDescriptor,
        src_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::ReadEp::readv_from]
    /// # Safety
    /// See [crate::comm::rma::ReadEp::readv_from]
    unsafe fn readv_from_async<'a, T>(
        &self,
        iov: &[crate::iovec::IoVecMut<'a>],
        desc: &mut [impl DataDescriptor],
        src_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>; //[TODO]

    /// Async version of [crate::comm::rma::ReadEp::readv_from_with_context]
    /// # Safety
    /// See [crate::comm::rma::ReadEp::readv_from_with_context]
    unsafe fn readv_from_with_context_async<'a, T, T0>(
        &self,
        iov: &[crate::iovec::IoVecMut<'a>],
        desc: &mut [impl DataDescriptor],
        src_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>; //[TODO]

    unsafe fn readmsg_from_async(
        &self,
        msg: &mut crate::msg::MsgRmaMut,
        options: ReadMsgOptions,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;
}

pub trait ConnectedAsyncReadEp {
    /// Async version of [crate::comm::rma::ReadEp::read]
    /// # Safety
    /// See [crate::comm::rma::ReadEp::read]
    unsafe fn read_async<T0>(
        &self,
        buf: &mut [T0],
        desc: &mut impl DataDescriptor,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::ReadEp::read_with_context]
    /// # Safety
    /// See [crate::comm::rma::ReadEp::read_with_context]
    unsafe fn read_with_context_async<T, T0>(
        &self,
        buf: &mut [T],
        desc: &mut impl DataDescriptor,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::ReadEp::readv]
    /// # Safety
    /// See [crate::comm::rma::ReadEp::readv]
    unsafe fn readv_async<'a, T>(
        &self,
        iov: &[crate::iovec::IoVecMut<'a>],
        desc: &mut [impl DataDescriptor],
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::ReadEp::readv_with_context]
    /// # Safety
    /// See [crate::comm::rma::ReadEp::readv_with_context]
    unsafe fn readv_with_context_async<'a, T, T0>(
        &self,
        iov: &[crate::iovec::IoVecMut<'a>],
        desc: &mut [impl DataDescriptor],
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>; //[TODO]

    /// Async version of [crate::comm::rma::ReadEp::readmsg]
    /// # Safety
    /// See [crate::comm::rma::ReadEp::readmsg]
    unsafe fn readmsg_async(
        &self,
        msg: &mut crate::msg::MsgRmaConnectedMut,
        options: ReadMsgOptions,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;
}

// impl<E: ReadMod, EQ: ?Sized + AsyncReadEq,  CQ: AsyncReadCq  + ? Sized> EndpointBase<E> {
impl<EP: RmaCap + ReadMod, EQ: ?Sized + AsyncReadEq, CQ: AsyncReadCq + ?Sized> AsyncReadEpImpl
    for EndpointImplBase<EP, EQ, CQ>
{
}

impl<E: AsyncReadEpImpl, MRREQ: EpMrReq> AsyncReadEpImpl for EndpointBase<E, Connected, MRREQ> {}
impl<E: AsyncReadEpImpl, MRREQ: EpMrReq> AsyncReadEpImpl
    for EndpointBase<E, Connectionless, MRREQ>
{
}

impl<EP: AsyncReadEpImpl> AsyncReadEp for EP {
    async unsafe fn read_from_async<T0>(
        &self,
        buf: &mut [T0],
        desc: &mut impl DataDescriptor,
        src_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.read_async_impl(buf, desc, Some(src_addr), mem_addr, mapped_key, None)
            .await
    }

    async unsafe fn read_from_with_context_async<T, T0>(
        &self,
        buf: &mut [T],
        desc: &mut impl DataDescriptor,
        src_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.read_async_impl(
            buf,
            desc,
            Some(src_addr),
            mem_addr,
            mapped_key,
            Some((context as *mut T0).cast()),
        )
        .await
    }

    async unsafe fn readv_from_async<'a, T>(
        &self,
        iov: &[crate::iovec::IoVecMut<'a>],
        desc: &mut [impl DataDescriptor],
        src_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> Result<SingleCompletion, crate::error::Error> {
        //[TODO]
        self.readv_async_impl(iov, desc, Some(src_addr), mem_addr, mapped_key, None)
            .await
    }

    async unsafe fn readv_from_with_context_async<'a, T, T0>(
        &self,
        iov: &[crate::iovec::IoVecMut<'a>],
        desc: &mut [impl DataDescriptor],
        src_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> Result<SingleCompletion, crate::error::Error> {
        //[TODO]
        self.readv_async_impl(
            iov,
            desc,
            Some(src_addr),
            mem_addr,
            mapped_key,
            Some((context as *mut T0).cast()),
        )
        .await
    }

    async unsafe fn readmsg_from_async<'a>(
        &self,
        msg: &mut crate::msg::MsgRmaMut<'a>,
        options: ReadMsgOptions,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.readmsg_async_impl(MsgType::ConnectionlessMsg(msg), options)
            .await
    }
}

impl<EP: AsyncReadEpImpl> ConnectedAsyncReadEp for EP {
    async unsafe fn read_async<T0>(
        &self,
        buf: &mut [T0],
        desc: &mut impl DataDescriptor,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.read_async_impl(buf, desc, None, mem_addr, mapped_key, None)
            .await
    }

    async unsafe fn read_with_context_async<T, T0>(
        &self,
        buf: &mut [T],
        desc: &mut impl DataDescriptor,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.read_async_impl(
            buf,
            desc,
            None,
            mem_addr,
            mapped_key,
            Some((context as *mut T0).cast()),
        )
        .await
    }

    async unsafe fn readv_async<'a, T>(
        &self,
        iov: &[crate::iovec::IoVecMut<'a>],
        desc: &mut [impl DataDescriptor],
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.readv_async_impl(iov, desc, None, mem_addr, mapped_key, None)
            .await
    }

    async unsafe fn readv_with_context_async<'a, T, T0>(
        &self,
        iov: &[crate::iovec::IoVecMut<'a>],
        desc: &mut [impl DataDescriptor],
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> Result<SingleCompletion, crate::error::Error> {
        //[TODO]
        self.readv_async_impl(
            iov,
            desc,
            None,
            mem_addr,
            mapped_key,
            Some((context as *mut T0).cast()),
        )
        .await
    }

    async unsafe fn readmsg_async<'a>(
        &self,
        msg: &mut crate::msg::MsgRmaConnectedMut<'a>,
        options: ReadMsgOptions,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.readmsg_async_impl(MsgType::ConnectedMsg(msg), options)
            .await
    }
}

pub(crate) trait AsyncWriteEpImpl: AsyncTxEp + WriteEpImpl {
    async unsafe fn write_async_impl<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        dest_mapped_addr: Option<&MappedAddress>,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        user_ctx: Option<*mut std::ffi::c_void>,
    ) -> Result<SingleCompletion, crate::error::Error> {
        let mut async_ctx = AsyncCtx { user_ctx };
        self.write_impl(
            buf,
            desc,
            dest_mapped_addr,
            mem_addr,
            mapped_key,
            Some(&mut async_ctx as *mut AsyncCtx as *mut std::ffi::c_void),
        )?;
        let cq = self.retrieve_tx_cq();
        cq.wait_for_ctx_async(&mut async_ctx).await
    }

    async unsafe fn writev_async_impl<'a>(
        &self,
        iov: &[crate::iovec::IoVec<'a>],
        desc: &mut [impl DataDescriptor],
        dest_mapped_addr: Option<&MappedAddress>,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        user_ctx: Option<*mut std::ffi::c_void>,
    ) -> Result<SingleCompletion, crate::error::Error> {
        let mut async_ctx = AsyncCtx { user_ctx };
        self.writev_impl(
            iov,
            desc,
            dest_mapped_addr,
            mem_addr,
            mapped_key,
            Some(&mut async_ctx as *mut AsyncCtx as *mut std::ffi::c_void),
        )?;
        let cq = self.retrieve_tx_cq();
        cq.wait_for_ctx_async(&mut async_ctx).await
    }

    #[allow(clippy::too_many_arguments)]
    async unsafe fn writedata_async_impl<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        data: u64,
        dest_mapped_addr: Option<&MappedAddress>,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        user_ctx: Option<*mut std::ffi::c_void>,
    ) -> Result<SingleCompletion, crate::error::Error> {
        let mut async_ctx = AsyncCtx { user_ctx };
        self.writedata_impl(
            buf,
            desc,
            data,
            dest_mapped_addr,
            mem_addr,
            mapped_key,
            Some(&mut async_ctx as *mut AsyncCtx as *mut std::ffi::c_void),
        )?;
        let cq = self.retrieve_tx_cq();
        cq.wait_for_ctx_async(&mut async_ctx).await
    }

    async unsafe fn writemsg_async_impl<'a>(
        &self,
        mut msg: MsgType<
            &mut crate::msg::MsgRma<'a>,
            &mut crate::msg::MsgRmaConnected<'a>,
            &mut crate::msg::MsgRmaMr<'a>,
            &mut crate::msg::MsgRmaConnectedMr<'a>,
        >,
        options: WriteMsgOptions,
    ) -> Result<SingleCompletion, crate::error::Error> {
        let real_user_ctx = match msg {
            MsgType::ConnectionlessMsg(ref mut msg) => msg.get_mut().context,
            MsgType::ConnectedMsg(ref mut msg) => msg.get_mut().context,
            MsgType::ConnectionlessMrMsg(ref mut msg) => msg.get_mut().context,
            MsgType::ConnectedMrMsg(ref mut msg) => msg.get_mut().context,
        };

        let mut async_ctx = AsyncCtx {
            user_ctx: if real_user_ctx.is_null() {
                None
            } else {
                Some(real_user_ctx)
            },
        };

        let imm_msg = match msg {
            MsgType::ConnectionlessMsg(ref mut msg) => {
                msg.get_mut().context = (&mut async_ctx as *mut AsyncCtx).cast();
                MsgType::<&MsgRma, &MsgRmaConnected, &MsgRmaMr, &MsgRmaConnectedMr>::ConnectionlessMsg(msg)
            }
            MsgType::ConnectedMsg(ref mut msg) => {
                msg.get_mut().context = (&mut async_ctx as *mut AsyncCtx).cast();
                MsgType::<&MsgRma, &MsgRmaConnected, &MsgRmaMr, &MsgRmaConnectedMr>::ConnectedMsg(
                    msg,
                )
            }
            MsgType::ConnectionlessMrMsg(ref mut msg) => {
                msg.get_mut().context = (&mut async_ctx as *mut AsyncCtx).cast();
                MsgType::<&MsgRma, &MsgRmaConnected, &MsgRmaMr, &MsgRmaConnectedMr>::ConnectionlessMrMsg(msg)
            }
            MsgType::ConnectedMrMsg(ref mut msg) => {
                msg.get_mut().context = (&mut async_ctx as *mut AsyncCtx).cast();
                MsgType::<&MsgRma, &MsgRmaConnected, &MsgRmaMr, &MsgRmaConnectedMr>::ConnectedMrMsg(
                    msg,
                )
            }
        };

        if let Err(err) = self.writemsg_impl(imm_msg, options) {
            match msg {
                MsgType::ConnectionlessMsg(msg) => msg.get_mut().context = real_user_ctx,
                MsgType::ConnectedMsg(msg) => msg.get_mut().context = real_user_ctx,
                MsgType::ConnectionlessMrMsg(msg) => msg.get_mut().context = real_user_ctx,
                MsgType::ConnectedMrMsg(msg) => msg.get_mut().context = real_user_ctx,
            }
            return Err(err);
        }
        let cq = self.retrieve_tx_cq();
        let res = cq.wait_for_ctx_async(&mut async_ctx).await;
        match msg {
            MsgType::ConnectionlessMsg(ref mut msg) => msg.get_mut().context = real_user_ctx,
            MsgType::ConnectedMsg(ref mut msg) => msg.get_mut().context = real_user_ctx,
            MsgType::ConnectionlessMrMsg(ref mut msg) => msg.get_mut().context = real_user_ctx,
            MsgType::ConnectedMrMsg(ref mut msg) => msg.get_mut().context = real_user_ctx,
        }
        res
    }
}

pub trait AsyncWriteEp: WriteEp {
    /// Async version of [crate::comm::rma::WriteEp::write_to]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::write_to]
    unsafe fn write_to_async<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        dest_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::WriteEp::write_to_with_context]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::write_to_with_context]
    unsafe fn write_to_with_context_async<T, T0>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        dest_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::WriteEp::writev_to]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::writev_to]
    unsafe fn writev_to_async<'a>(
        &self,
        iov: &[crate::iovec::IoVec<'a>],
        desc: &mut [impl DataDescriptor],
        dest_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::WriteEp::writev_to_with_context]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::writev_to_with_context]
    unsafe fn writev_to_with_context_async<'a, T0>(
        &self,
        iov: &[crate::iovec::IoVec<'a>],
        desc: &mut [impl DataDescriptor],
        dest_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::WriteEp::writemsg_to]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::writemsg_to]
    unsafe fn writemsg_to_async(
        &self,
        msg: &mut crate::msg::MsgRma,
        options: WriteMsgOptions,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::WriteEp::writedata_to]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::writedata_to]
    unsafe fn writedata_to_async<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        data: u64,
        dest_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::WriteEp::writedata_to_with_context]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::writedata_to_with_context]
    #[allow(clippy::too_many_arguments)]
    unsafe fn writedata_to_with_context_async<T, T0>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        data: u64,
        dest_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;
}

pub trait ConnectedAsyncWriteEp: ConnectedWriteEp {
    /// Async version of [crate::comm::rma::WriteEp::write]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::write]
    unsafe fn write_async<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::WriteEp::write_with_context]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::write_with_context]
    unsafe fn write_with_context_async<T, T0>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::WriteEp::writev]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::writev]
    unsafe fn writev_async<'a>(
        &self,
        iov: &[crate::iovec::IoVec<'a>],
        desc: &mut [impl DataDescriptor],
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::WriteEp::writev_with_context]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::writev_with_context]
    unsafe fn writev_with_context_async<'a, T0>(
        &self,
        iov: &[crate::iovec::IoVec<'a>],
        desc: &mut [impl DataDescriptor],
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::WriteEp::writemsg]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::writemsg]
    unsafe fn writemsg_async(
        &self,
        msg: &mut crate::msg::MsgRmaConnected,
        options: WriteMsgOptions,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::WriteEp::writedata]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::writedata]
    unsafe fn writedata_async<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        data: u64,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;

    /// Async version of [crate::comm::rma::WriteEp::writedata_with_context]
    /// # Safety
    /// See [crate::comm::rma::WriteEp::writedata_with_context]
    unsafe fn writedata_with_context_async<T, T0>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        data: u64,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> impl std::future::Future<Output = Result<SingleCompletion, crate::error::Error>>;
}

// impl<E: WriteMod, EQ: ?Sized + AsyncReadEq,  CQ: AsyncReadCq  + ? Sized> EndpointBase<E> {
impl<EP: RmaCap + WriteMod, EQ: ?Sized + AsyncReadEq, CQ: AsyncReadCq + ?Sized> AsyncWriteEpImpl
    for EndpointImplBase<EP, EQ, CQ>
{
}

impl<E: AsyncWriteEpImpl, MRREQ: EpMrReq> AsyncWriteEpImpl for EndpointBase<E, Connected, MRREQ> {
    async unsafe fn write_async_impl<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        dest_mapped_addr: Option<&MappedAddress>,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        user_ctx: Option<*mut std::ffi::c_void>,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.inner
            .write_async_impl(buf, desc, dest_mapped_addr, mem_addr, mapped_key, user_ctx)
            .await
    }

    async unsafe fn writev_async_impl<'a>(
        &self,
        iov: &[crate::iovec::IoVec<'a>],
        desc: &mut [impl DataDescriptor],
        dest_mapped_addr: Option<&MappedAddress>,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        user_ctx: Option<*mut std::ffi::c_void>,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.inner
            .writev_async_impl(iov, desc, dest_mapped_addr, mem_addr, mapped_key, user_ctx)
            .await
    }

    async unsafe fn writedata_async_impl<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        data: u64,
        dest_mapped_addr: Option<&MappedAddress>,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        user_ctx: Option<*mut std::ffi::c_void>,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.inner
            .writedata_async_impl(
                buf,
                desc,
                data,
                dest_mapped_addr,
                mem_addr,
                mapped_key,
                user_ctx,
            )
            .await
    }

    async unsafe fn writemsg_async_impl<'a>(
        &self,
        msg: MsgType<
            &mut crate::msg::MsgRma<'a>,
            &mut crate::msg::MsgRmaConnected<'a>,
            &mut crate::msg::MsgRmaMr<'a>,
            &mut crate::msg::MsgRmaConnectedMr<'a>,
        >,
        options: WriteMsgOptions,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.inner.writemsg_async_impl(msg, options).await
    }
}

impl<E: AsyncWriteEpImpl, MRREQ: EpMrReq> AsyncWriteEpImpl
    for EndpointBase<E, Connectionless, MRREQ>
{
    async unsafe fn write_async_impl<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        dest_mapped_addr: Option<&MappedAddress>,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        user_ctx: Option<*mut std::ffi::c_void>,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.inner
            .write_async_impl(buf, desc, dest_mapped_addr, mem_addr, mapped_key, user_ctx)
            .await
    }

    async unsafe fn writev_async_impl<'a>(
        &self,
        iov: &[crate::iovec::IoVec<'a>],
        desc: &mut [impl DataDescriptor],
        dest_mapped_addr: Option<&MappedAddress>,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        user_ctx: Option<*mut std::ffi::c_void>,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.inner
            .writev_async_impl(iov, desc, dest_mapped_addr, mem_addr, mapped_key, user_ctx)
            .await
    }

    async unsafe fn writedata_async_impl<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        data: u64,
        dest_mapped_addr: Option<&MappedAddress>,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        user_ctx: Option<*mut std::ffi::c_void>,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.inner
            .writedata_async_impl(
                buf,
                desc,
                data,
                dest_mapped_addr,
                mem_addr,
                mapped_key,
                user_ctx,
            )
            .await
    }

    async unsafe fn writemsg_async_impl<'a>(
        &self,
        msg: MsgType<
            &mut crate::msg::MsgRma<'a>,
            &mut crate::msg::MsgRmaConnected<'a>,
            &mut crate::msg::MsgRmaMr<'a>,
            &mut crate::msg::MsgRmaConnectedMr<'a>,
        >,
        options: WriteMsgOptions,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.inner.writemsg_async_impl(msg, options).await
    }
}

impl<EP: AsyncWriteEpImpl + ConnlessEp> AsyncWriteEp for EP {
    #[inline]
    async unsafe fn write_to_async<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        dest_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.write_async_impl(buf, desc, Some(dest_addr), mem_addr, mapped_key, None)
            .await
    }

    #[inline]
    async unsafe fn write_to_with_context_async<T, T0>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        dest_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.write_async_impl(
            buf,
            desc,
            Some(dest_addr),
            mem_addr,
            mapped_key,
            Some((context as *mut T0).cast()),
        )
        .await
    }

    #[inline]
    async unsafe fn writev_to_async<'a>(
        &self,
        iov: &[crate::iovec::IoVec<'a>],
        desc: &mut [impl DataDescriptor],
        dest_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.writev_async_impl(iov, desc, Some(dest_addr), mem_addr, mapped_key, None)
            .await
    }

    #[inline]
    async unsafe fn writev_to_with_context_async<'a, T0>(
        &self,
        iov: &[crate::iovec::IoVec<'a>],
        desc: &mut [impl DataDescriptor],
        dest_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.writev_async_impl(
            iov,
            desc,
            Some(dest_addr),
            mem_addr,
            mapped_key,
            Some((context as *mut T0).cast()),
        )
        .await
    }

    #[inline]
    async unsafe fn writemsg_to_async<'a>(
        &self,
        msg: &mut crate::msg::MsgRma<'a>,
        options: WriteMsgOptions,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.writemsg_async_impl(MsgType::ConnectionlessMsg(msg), options)
            .await
    }

    #[inline]
    async unsafe fn writedata_to_async<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        data: u64,
        dest_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.writedata_async_impl(buf, desc, data, Some(dest_addr), mem_addr, mapped_key, None)
            .await
    }

    #[inline]
    async unsafe fn writedata_to_with_context_async<T, T0>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        data: u64,
        dest_addr: &MappedAddress,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.writedata_async_impl(
            buf,
            desc,
            data,
            Some(dest_addr),
            mem_addr,
            mapped_key,
            Some((context as *mut T0).cast()),
        )
        .await
    }
}

impl<EP: AsyncWriteEpImpl + ConnectedEp> ConnectedAsyncWriteEp for EP {
    #[inline]
    async unsafe fn write_async<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.write_async_impl(buf, desc, None, mem_addr, mapped_key, None)
            .await
    }

    #[inline]
    async unsafe fn write_with_context_async<T, T0>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.write_async_impl(
            buf,
            desc,
            None,
            mem_addr,
            mapped_key,
            Some((context as *mut T0).cast()),
        )
        .await
    }

    #[inline]
    async unsafe fn writev_async<'a>(
        &self,
        iov: &[crate::iovec::IoVec<'a>],
        desc: &mut [impl DataDescriptor],
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> Result<SingleCompletion, crate::error::Error> {
        //[TODO]
        self.writev_async_impl(iov, desc, None, mem_addr, mapped_key, None)
            .await
    }

    #[inline]
    async unsafe fn writev_with_context_async<'a, T0>(
        &self,
        iov: &[crate::iovec::IoVec<'a>],
        desc: &mut [impl DataDescriptor],
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> Result<SingleCompletion, crate::error::Error> {
        //[TODO]
        self.writev_async_impl(
            iov,
            desc,
            None,
            mem_addr,
            mapped_key,
            Some((context as *mut T0).cast()),
        )
        .await
    }

    #[inline]
    async unsafe fn writemsg_async<'a>(
        &self,
        msg: &mut crate::msg::MsgRmaConnected<'a>,
        options: WriteMsgOptions,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.writemsg_async_impl(MsgType::ConnectedMsg(msg), options)
            .await
    }

    #[inline]
    async unsafe fn writedata_async<T>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        data: u64,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.writedata_async_impl(buf, desc, data, None, mem_addr, mapped_key, None)
            .await
    }

    #[inline]
    async unsafe fn writedata_with_context_async<T, T0>(
        &self,
        buf: &[T],
        desc: &mut impl DataDescriptor,
        data: u64,
        mem_addr: u64,
        mapped_key: &MappedMemoryRegionKey,
        context: &mut T0,
    ) -> Result<SingleCompletion, crate::error::Error> {
        self.writedata_async_impl(
            buf,
            desc,
            data,
            None,
            mem_addr,
            mapped_key,
            Some((context as *mut T0).cast()),
        )
        .await
    }
}

pub trait AsyncReadWriteEp: AsyncReadEp + AsyncWriteEp {}
impl<EP: AsyncReadEp + AsyncWriteEp> AsyncReadWriteEp for EP {}

// impl AsyncWriteEpImpl for TransmitContext {}
// impl AsyncWriteEpImpl for TransmitContextImpl {}
// impl AsyncReadEpImpl for ReceiveContext {}
// impl AsyncReadEpImpl for ReceiveContextImpl {}
