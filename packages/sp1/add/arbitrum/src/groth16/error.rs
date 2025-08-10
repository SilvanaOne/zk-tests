use crate::bn::{CurveError, FieldError, GroupError};

#[derive(Debug)]
pub enum Error {
    // Input Errors
    InvalidWitness,
    InvalidXLength,
    InvalidData,
    InvalidPoint,

    // Conversion Errors
    FailedToGetFrFromRandomBytes,

    // External Library Errors
    Field(FieldError),
    Group(GroupError),
    Curve(CurveError),

    // SP1 Errors
    InvalidProgramVkeyHash,
}