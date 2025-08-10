mod fp;
mod fq12;
mod fq2;
mod fq6;

use crate::bn::arith::U256;
use alloc::fmt::Debug;
use core::ops::{Add, Div, Mul, Neg, Sub};

pub use self::fp::{const_fq, Fq, Fr};
pub use self::fq12::Fq12;
pub use self::fq2::{fq2_nonresidue, Fq2};
pub use self::fq6::Fq6;

pub trait FieldElement:
    Sized
    + Copy
    + Clone
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Neg<Output = Self>
    + Div<Output = Self>
    + PartialEq
    + Eq
    + Debug
{
    fn zero() -> Self;
    fn one() -> Self;
    fn is_zero(&self) -> bool;
    fn squared(&self) -> Self {
        (*self) * (*self)
    }
    fn inverse(self) -> Option<Self>;
    fn pow<I: Into<U256>>(&self, by: I) -> Self {
        let mut res = Self::one();

        for i in by.into().bits() {
            res = res.squared();
            if i {
                res = *self * res;
            }
        }

        res
    }
}

pub trait Sqrt: Sized {
    fn sqrt(&self) -> Option<Self>;
}
