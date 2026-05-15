//! Fixed 25-column order used by BRO CPT data arrays.
//!
//! Reference: BRO IMBRO/A CPT_O / CPT_O_DP standard.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BroField {
    Length,
    Depth,
    ElapsedTime,
    Qc,
    CorrectedQc,
    NetQc,
    MagX,
    MagY,
    MagZ,
    MagTotal,
    ElectricCond,
    InclEw,
    InclNs,
    InclX,
    InclY,
    Inclination,
    MagInclination,
    MagDeclination,
    Fs,
    PoreRatio,
    Temp,
    U1,
    U2,
    U3,
    Rf,
}

pub const ORDER: [BroField; 25] = [
    BroField::Length,
    BroField::Depth,
    BroField::ElapsedTime,
    BroField::Qc,
    BroField::CorrectedQc,
    BroField::NetQc,
    BroField::MagX,
    BroField::MagY,
    BroField::MagZ,
    BroField::MagTotal,
    BroField::ElectricCond,
    BroField::InclEw,
    BroField::InclNs,
    BroField::InclX,
    BroField::InclY,
    BroField::Inclination,
    BroField::MagInclination,
    BroField::MagDeclination,
    BroField::Fs,
    BroField::PoreRatio,
    BroField::Temp,
    BroField::U1,
    BroField::U2,
    BroField::U3,
    BroField::Rf,
];

pub const VOID_VALUE: f64 = -999_999.0;
