use std::marker::PhantomData;

use crate::{error, MyRc};
pub(crate) type RawFid = *mut libfabric_sys::fid;

#[derive(Hash, Clone, Copy)]
pub struct Fid(pub(crate) RawFid);

impl PartialEq for Fid {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Fid {}

impl AsRawFid for Fid {
    fn as_raw_fid(&self) -> RawFid {
        self.0
    }
}

pub(crate) struct OwnedTypedFid<FID: AsRawFid> {
    typed_fid: FID,
}

impl<FID: AsRawFid + AsRawTypedFid> OwnedTypedFid<FID> {
    pub(crate) fn from(typed_fid: FID) -> Self {
        Self { typed_fid }
    }
}

pub struct BorrowedFid<'a> {
    fid: RawFid,
    phantom: PhantomData<&'a OwnedTypedFid<RawFid>>,
}

impl BorrowedFid<'_> {
    #[inline]
    pub const unsafe fn borrow_raw(fid: RawFid) -> Self {
        Self {
            fid,
            phantom: PhantomData,
        }
    }
}

pub struct BorrowedTypedFid<'a, FID: AsRawFid + Copy> {
    typed_fid: FID,
    phantom: PhantomData<&'a OwnedTypedFid<FID>>,
}

impl<FID: AsRawFid + Copy> BorrowedTypedFid<'_, FID> {
    #[inline]
    pub const unsafe fn borrow_raw(typed_fid: FID) -> Self {
        Self {
            typed_fid,
            phantom: PhantomData,
        }
    }
}

impl<'a, FID: AsRawFid + Copy> AsRawTypedFid for BorrowedTypedFid<'a, FID> {
    type Output = FID;
    #[inline]
    fn as_raw_typed_fid(&self) -> Self::Output {
        self.typed_fid
    }
}

impl<'a, FID: AsRawFid + Copy> AsRawFid for BorrowedTypedFid<'a, FID> {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        self.typed_fid.as_raw_fid()
    }
}

impl<FID: AsRawFid> Drop for OwnedTypedFid<FID> {
    #[inline]
    fn drop(&mut self) {
        let err = unsafe { libfabric_sys::inlined_fi_close(self.typed_fid.as_raw_fid()) };
        if err != 0 {
            panic!(
                "{}",
                error::Error::from_err_code((-err).try_into().unwrap())
            );
        }
    }
}

impl<T: AsRawFid> AsRawFid for MyRc<T> {
    fn as_raw_fid(&self) -> RawFid {
        (**self).as_raw_fid()
    }
}

impl<FID: AsRawFid + Copy> AsRawTypedFid for OwnedTypedFid<FID> {
    type Output = FID;

    fn as_raw_typed_fid(&self) -> Self::Output {
        self.typed_fid
    }
}

impl<FID: AsRawFid> AsRawFid for OwnedTypedFid<FID> {
    fn as_raw_fid(&self) -> RawFid {
        self.typed_fid.as_raw_fid()
    }
}

impl<FID: AsRawFid> AsFid for OwnedTypedFid<FID> {
    fn as_fid(&self) -> BorrowedFid<'_> {
        unsafe { BorrowedFid::borrow_raw(self.as_raw_fid()) }
    }
}

impl<FID: AsRawFid + Copy> AsTypedFid<FID> for OwnedTypedFid<FID> {
    fn as_typed_fid(&self) -> BorrowedTypedFid<'_, FID> {
        unsafe { BorrowedTypedFid::borrow_raw(self.as_raw_typed_fid()) }
    }
}

pub trait AsFid {
    fn as_fid(&self) -> BorrowedFid;
}

pub trait AsRawFid {
    fn as_raw_fid(&self) -> RawFid;
}

pub trait AsTypedFid<FID: AsRawFid + Copy> {
    fn as_typed_fid(&self) -> BorrowedTypedFid<FID>;
}

pub trait AsRawTypedFid {
    type Output: AsRawFid;

    fn as_raw_typed_fid(&self) -> Self::Output;
}

impl AsRawFid for RawFid {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        *self
    }
}

impl<'a> AsRawFid for BorrowedFid<'a> {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        self.fid
    }
}

pub(crate) type DomainRawFid = *mut libfabric_sys::fid_domain;

pub(crate) type OwnedDomainFid = OwnedTypedFid<DomainRawFid>;

impl AsRawTypedFid for DomainRawFid {
    type Output = DomainRawFid;

    fn as_raw_typed_fid(&self) -> Self::Output {
        *self
    }
}

impl AsRawFid for DomainRawFid {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        unsafe { &mut (**self).fid }
    }
}

pub(crate) type AvRawFid = *mut libfabric_sys::fid_av;

impl AsRawTypedFid for AvRawFid {
    type Output = AvRawFid;

    #[inline]
    fn as_raw_typed_fid(&self) -> Self::Output {
        *self
    }
}

impl AsRawFid for AvRawFid {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        unsafe { &mut (**self).fid }
    }
}

pub(crate) type OwnedAVFid = OwnedTypedFid<AvRawFid>;

pub(crate) type AVSetRawFid = *mut libfabric_sys::fid_av_set;

impl AsRawTypedFid for AVSetRawFid {
    type Output = AVSetRawFid;

    #[inline]
    fn as_raw_typed_fid(&self) -> Self::Output {
        *self
    }
}

impl AsRawFid for AVSetRawFid {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        unsafe { &mut (**self).fid }
    }
}

pub(crate) type OwnedAVSetFid = OwnedTypedFid<AVSetRawFid>;

pub(crate) type CntrRawFid = *mut libfabric_sys::fid_cntr;

impl AsRawTypedFid for CntrRawFid {
    type Output = CntrRawFid;

    #[inline]
    fn as_raw_typed_fid(&self) -> Self::Output {
        *self
    }
}

impl AsRawFid for CntrRawFid {
    fn as_raw_fid(&self) -> RawFid {
        unsafe { &mut (**self).fid }
    }
}

pub(crate) type OwnedCntrFid = OwnedTypedFid<CntrRawFid>;

pub(crate) type CqRawFid = *mut libfabric_sys::fid_cq;

impl AsRawTypedFid for CqRawFid {
    type Output = CqRawFid;

    #[inline]
    fn as_raw_typed_fid(&self) -> Self::Output {
        *self
    }
}

impl AsRawFid for CqRawFid {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        unsafe { &mut (**self).fid }
    }
}

pub(crate) type OwnedCqFid = OwnedTypedFid<CqRawFid>;

pub(crate) type FabricRawFid = *mut libfabric_sys::fid_fabric;

impl AsRawTypedFid for FabricRawFid {
    type Output = FabricRawFid;

    #[inline]
    fn as_raw_typed_fid(&self) -> Self::Output {
        *self
    }
}

impl AsRawFid for FabricRawFid {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        unsafe { &mut (**self).fid }
    }
}

pub(crate) type OwnedFabricFid = OwnedTypedFid<FabricRawFid>;

pub(crate) type MrRawFid = *mut libfabric_sys::fid_mr;

impl AsRawTypedFid for MrRawFid {
    type Output = MrRawFid;

    #[inline]
    fn as_raw_typed_fid(&self) -> Self::Output {
        *self
    }
}

impl AsRawFid for MrRawFid {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        unsafe { &mut (**self).fid }
    }
}

pub(crate) type OwnedMrFid = OwnedTypedFid<MrRawFid>;

pub(crate) type EqRawFid = *mut libfabric_sys::fid_eq;

impl AsRawTypedFid for EqRawFid {
    type Output = EqRawFid;

    #[inline]
    fn as_raw_typed_fid(&self) -> Self::Output {
        *self
    }
}

impl AsRawFid for EqRawFid {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        unsafe { &mut (**self).fid }
    }
}

pub(crate) type OwnedEqFid = OwnedTypedFid<EqRawFid>;

pub(crate) type WaitRawFid = *mut libfabric_sys::fid_wait;

impl AsRawTypedFid for WaitRawFid {
    type Output = WaitRawFid;

    #[inline]
    fn as_raw_typed_fid(&self) -> Self::Output {
        *self
    }
}

impl AsRawFid for WaitRawFid {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        unsafe { &mut (**self).fid }
    }
}

pub(crate) type OwnedWaitFid = OwnedTypedFid<WaitRawFid>;

pub(crate) type EpRawFid = *mut libfabric_sys::fid_ep;

impl AsRawTypedFid for EpRawFid {
    type Output = EpRawFid;

    #[inline]
    fn as_raw_typed_fid(&self) -> Self::Output {
        *self
    }
}

impl AsRawFid for EpRawFid {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        unsafe { &mut (**self).fid }
    }
}

pub(crate) type OwnedEpFid = OwnedTypedFid<EpRawFid>;

pub(crate) type PepRawFid = *mut libfabric_sys::fid_pep;

impl AsRawTypedFid for PepRawFid {
    type Output = PepRawFid;

    #[inline]
    fn as_raw_typed_fid(&self) -> Self::Output {
        *self
    }
}

impl AsRawFid for PepRawFid {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        unsafe { &mut (**self).fid }
    }
}

pub(crate) type OwnedPepFid = OwnedTypedFid<PepRawFid>;

pub(crate) type McRawFid = *mut libfabric_sys::fid_mc;

impl AsRawTypedFid for McRawFid {
    type Output = McRawFid;

    #[inline]
    fn as_raw_typed_fid(&self) -> Self::Output {
        *self
    }
}

impl AsRawFid for McRawFid {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        unsafe { &mut (**self).fid }
    }
}

pub(crate) type OwnedMcFid = OwnedTypedFid<McRawFid>;

pub(crate) type PollRawFid = *mut libfabric_sys::fid_poll;

impl AsRawTypedFid for PollRawFid {
    type Output = PollRawFid;

    #[inline]
    fn as_raw_typed_fid(&self) -> Self::Output {
        *self
    }
}

impl AsRawFid for PollRawFid {
    #[inline]
    fn as_raw_fid(&self) -> RawFid {
        unsafe { &mut (**self).fid }
    }
}

pub(crate) type OwnedPollFid = OwnedTypedFid<PollRawFid>;
