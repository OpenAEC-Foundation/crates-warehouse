//! GEF column quantity numbers (per CUR/NEN convention) → field names.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GefField {
    Length, Qc, Fs, Rf, U1, U2, U3,
    Inclination, InclNs, InclEw,
    Depth, Time, CorrectedQc, NetQc, PoreRatio,
    Speed, Temp, ElectricCond, FrictionTotal,
    Unknown(u32),
}

pub fn from_quantity(q: u32) -> GefField {
    match q {
        1 => GefField::Length,
        2 => GefField::Qc,
        3 => GefField::Fs,
        4 => GefField::Rf,
        5 => GefField::U1,
        6 => GefField::U2,
        7 => GefField::U3,
        8 => GefField::Inclination,
        9 => GefField::InclNs,
        10 => GefField::InclEw,
        11 => GefField::Depth,
        12 => GefField::Time,
        13 => GefField::CorrectedQc,
        14 => GefField::NetQc,
        15 => GefField::PoreRatio,
        20 => GefField::Speed,
        21 => GefField::Temp,
        23 => GefField::ElectricCond,
        39 => GefField::FrictionTotal,
        n => GefField::Unknown(n),
    }
}
