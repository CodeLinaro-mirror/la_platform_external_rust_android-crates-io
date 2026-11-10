// SPDX-FileCopyrightText: Copyright The arm-sysregs Contributors.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Types used by the system register crates.

use bitflags::bitflags;
use num_enum::{IntoPrimitive, TryFromPrimitive};

/// An AArch64 exception level.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ExceptionLevel {
    /// Exception level 0.
    El0 = 0,
    /// Exception level 1.
    El1 = 1,
    /// Exception level 2.
    El2 = 2,
    /// Exception level 3.
    El3 = 3,
}

/// Values for SPSEL.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum StackPointer {
    /// Use SP_EL0.
    El0 = 0,
    /// Use SP_EL1, SP_EL2 or SP_EL3 according to the current exception level.
    ElX = 1,
}

/// Allowed Shareability attributes.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum Shareability {
    /// Non-shareable.
    Non = 0b00,
    /// Outer-shareable.
    Outer = 0b10,
    /// Inner-shareable.
    Inner = 0b11,
}

/// Allowed Cacheability attributes.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum Cacheability {
    /// Normal memory, Non-cacheable.
    Non = 0b00,
    /// Normal memory, Write-Back Read-Allocate Write-Allocate Cacheable.
    WriteBackAllocate = 0b01,
    /// Normal memory, Write-Through Read-Allocate No Write-Allocate Cacheable.
    WriteThrough = 0b10,
    /// Normal memory, Write-Back Read-Allocate No Write-Allocate Cacheable.
    WriteBackNoAllocate = 0b11,
}

/// Cache type enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CacheType {
    /// No cache.
    NoCache = 0b000,
    /// Instruction cache only.
    InstructionOnly = 0b001,
    /// Data cache only.
    DataOnly = 0b010,
    /// Separate instruction and data caches.
    SeparateInstructionAndData = 0b011,
    /// Unified cache.
    Unified = 0b100,
}

/// Wrapper type for describing cache level in a human readable format, e.g. L3 cache = `CacheLevel(3)`
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheLevel(pub u8);

impl CacheLevel {
    /// Creates new instance.
    pub fn new(level: u8) -> Self {
        assert!((1..8).contains(&level));
        Self(level)
    }

    /// Returns the level value.
    pub fn level(&self) -> u8 {
        self.0
    }
}

impl From<CacheLevel> for u32 {
    fn from(value: CacheLevel) -> Self {
        (value.0 - 1).into()
    }
}

impl From<CacheLevel> for u64 {
    fn from(value: CacheLevel) -> Self {
        u32::from(value).into()
    }
}

/// Uniquely identifies a sysreg.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EsrIssSysregId(u32);

impl EsrIssSysregId {
    /// Creates new instance from encoding components.
    pub const fn new(op0: u32, op1: u32, crn: u32, crm: u32, op2: u32) -> Self {
        assert!(op0 == op0 & EsrIssSysreg::OP0_MASK);
        assert!(op1 == op1 & EsrIssSysreg::OP1_MASK);
        assert!(crn == crn & EsrIssSysreg::CRN_MASK);
        assert!(crm == crm & EsrIssSysreg::CRM_MASK);
        assert!(op2 == op2 & EsrIssSysreg::OP2_MASK);
        Self(
            op0 << EsrIssSysreg::OP0_SHIFT
                | op1 << EsrIssSysreg::OP1_SHIFT
                | crn << EsrIssSysreg::CRN_SHIFT
                | crm << EsrIssSysreg::CRM_SHIFT
                | op2 << EsrIssSysreg::OP2_SHIFT,
        )
    }
}

impl From<EsrIssSysreg> for EsrIssSysregId {
    fn from(value: EsrIssSysreg) -> Self {
        value.sysreg_id()
    }
}

/// Represents the transfer direction of trapped sysreg accesses.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EsrIssSysregDir {
    /// Write access, including MSR instructions.
    Write,
    /// Read access, including MRS instructions.
    Read,
}

bitflags! {
    /// ESR_ELx.ISS encoding for an exception from MSR, MRS, or System instruction execution in AArch64 state
    #[derive(Clone, Copy, Eq, Default, PartialEq)]
    #[repr(transparent)]
    pub struct EsrIssSysreg: u32 {
    }
}

impl EsrIssSysreg {
    /// Register number of XZR.
    pub const XZR_REG_NUM: u8 = 31;

    /// Offset of the `Op0` field.
    const OP0_SHIFT: u32 = 20;
    /// Mask for the `Op0` field.
    const OP0_MASK: u32 = 0b11;
    /// Offset of the `Op2` field.
    const OP2_SHIFT: u32 = 17;
    /// Mask for the `Op2` field.
    const OP2_MASK: u32 = 0b111;
    /// Offset of the `Op1` field.
    const OP1_SHIFT: u32 = 14;
    /// Mask for the `Op1` field.
    const OP1_MASK: u32 = 0b111;
    /// Offset of the `CRn` field.
    const CRN_SHIFT: u32 = 10;
    /// Mask for the `CRn` field.
    const CRN_MASK: u32 = 0b1111;
    /// Offset of the `Rt` field.
    const RT_SHIFT: u32 = 5;
    /// Mask for the `Rt` field.
    const RT_MASK: u32 = 0b11111;
    /// Offset of the `CRm` field.
    const CRM_SHIFT: u32 = 1;
    /// Mask for the `CRm` field.
    const CRM_MASK: u32 = 0b1111;
    /// Offset of the `Direction` field.
    const DIR_SHIFT: u32 = 0;
    /// Mask for the `Direction` field.
    const DIR_MASK: u32 = 0b1;

    /// Mask for the parts containing the sysreg ID: all fields except `Rt` and `DIR`.
    const SYSREG_MASK: u32 = (Self::OP0_MASK << Self::OP0_SHIFT)
        | (Self::OP2_MASK << Self::OP2_SHIFT)
        | (Self::OP1_MASK << Self::OP1_SHIFT)
        | (Self::CRN_MASK << Self::CRN_SHIFT)
        | (Self::CRM_MASK << Self::CRM_SHIFT);

    /// Returns the opcode.
    pub const fn sysreg_id(self) -> EsrIssSysregId {
        EsrIssSysregId(self.bits() & Self::SYSREG_MASK)
    }

    /// Returns the value of the `Rt` field.
    pub const fn rt(self) -> u8 {
        ((self.bits() >> Self::RT_SHIFT) & Self::RT_MASK) as u8
    }

    /// Returns the value of the `Direction` field.
    pub const fn dir(self) -> EsrIssSysregDir {
        if ((self.bits() >> Self::DIR_SHIFT) & Self::DIR_MASK) != 0 {
            EsrIssSysregDir::Read
        } else {
            EsrIssSysregDir::Write
        }
    }
}
