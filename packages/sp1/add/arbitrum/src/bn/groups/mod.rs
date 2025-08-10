use crate::bn::arith::U256;
use crate::bn::fields::{const_fq, fq2_nonresidue, FieldElement, Fq, Fq12, Fq2, Fr, Sqrt};
#[allow(unused_imports)]
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Display;
#[cfg(target_os = "zkvm")]
use core::mem::transmute;
use core::ops::AddAssign;
use core::{
    fmt,
    ops::{Add, Mul, Neg, Sub},
};

#[cfg(target_os = "zkvm")]
use sp1_lib::{syscall_bn254_add, syscall_bn254_double};
// This is the NAF version of ate_loop_count. Entries are all mod 4, so 3 = -1
// n.b. ate_loop_count = 0x19d797039be763ba8
//                     = 11001110101111001011100000011100110111110011101100011101110101000
//       (naf version) = 11010003030003010300300000100301003000030100030300100030030101000
// We skip the first element (1) as we would need to skip over it in the main loop
const ATE_LOOP_COUNT_NAF: [u8; 64] = [
    1, 0, 1, 0, 0, 0, 3, 0, 3, 0, 0, 0, 3, 0, 1, 0, 3, 0, 0, 3, 0, 0, 0, 0, 0, 1, 0, 0, 3, 0, 1, 0,
    0, 3, 0, 0, 0, 0, 3, 0, 1, 0, 0, 0, 3, 0, 3, 0, 0, 1, 0, 0, 0, 3, 0, 0, 3, 0, 1, 0, 1, 0, 0, 0,
];

pub trait GroupElement:
    Sized
    + Copy
    + Clone
    + PartialEq
    + Eq
    + fmt::Debug
    + Add<Output = Self>
    + Sub<Output = Self>
    + Neg<Output = Self>
    + Mul<Fr, Output = Self>
{
    type Base: FieldElement;
    
    fn zero() -> Self;
    fn one() -> Self;
    fn coeff_b() -> Self::Base;
    
    #[inline(always)]
    fn add_b(elem: Self::Base) -> Self::Base {
        if Self::coeff_b().is_zero() {
            elem
        } else {
            elem + Self::coeff_b()
        }
    }
}

pub trait GroupParams: Sized {
    type Base: FieldElement + Sqrt + PartialOrd;

    fn one() -> G<Self>;
    fn coeff_b() -> Self::Base;
    fn check_order() -> bool {
        false
    }
    
    #[inline(always)]
    fn add_b(elem: Self::Base) -> Self::Base {
        if Self::coeff_b().is_zero() {
            elem
        } else {
            elem + Self::coeff_b()
        }
    }
}

#[repr(C)]
pub struct G<P: GroupParams> {
    x: P::Base,
    y: P::Base,
    z: P::Base,
}

impl<P: GroupParams> G<P> {
    pub fn new(x: P::Base, y: P::Base, z: P::Base) -> Self {
        G { x, y, z }
    }

    pub fn x(&self) -> &P::Base {
        &self.x
    }

    pub fn x_mut(&mut self) -> &mut P::Base {
        &mut self.x
    }

    pub fn y(&self) -> &P::Base {
        &self.y
    }

    pub fn y_mut(&mut self) -> &mut P::Base {
        &mut self.y
    }

    pub fn z(&self) -> &P::Base {
        &self.z
    }

    pub fn z_mut(&mut self) -> &mut P::Base {
        &mut self.z
    }
}

#[derive(Debug, Default)]
pub struct AffineG<P: GroupParams> {
    x: P::Base,
    y: P::Base,
}

#[derive(Debug)]
pub enum Error {
    NotOnCurve,
    NotInSubgroup,
    InvalidInputLength,
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotOnCurve => write!(f, "Point is not on curve"),
            Error::NotInSubgroup => write!(f, "Point is not in subgroup"),
            Error::InvalidInputLength => write!(f, "Invalid input length"),
        }
    }
}
impl<P: GroupParams> AffineG<P> {
    pub fn new(x: P::Base, y: P::Base) -> Result<Self, Error> {
        let lhs = y.squared();
        let rhs = (x.squared() * x) + P::coeff_b();
        if lhs == rhs {
            if P::check_order() {
                let p: G<P> = G {
                    x,
                    y,
                    z: P::Base::one(),
                };

                if (p * (-Fr::one())) + p != G::zero() {
                    return Err(Error::NotInSubgroup);
                }
            }

            Ok(AffineG { x, y })
        } else {
            Err(Error::NotOnCurve)
        }
    }

    pub fn new_unchecked(x: P::Base, y: P::Base) -> Self {
        AffineG { x, y }
    }

    pub fn zero() -> Self {
        AffineG {
            x: P::Base::zero(),
            y: P::Base::one(),
        }
    }

    pub fn x(&self) -> &P::Base {
        &self.x
    }

    pub fn x_mut(&mut self) -> &mut P::Base {
        &mut self.x
    }

    pub fn y(&self) -> &P::Base {
        &self.y
    }

    pub fn y_mut(&mut self) -> &mut P::Base {
        &mut self.y
    }

    pub fn one() -> Self {
        let p: G<P> = G::one();
        AffineG::new(p.x / p.z, p.y / p.z).unwrap()
    }

    /// Returns the two possible y-coordinates corresponding to the given x-coordinate.
    /// The corresponding points are not guaranteed to be in the prime-order subgroup,
    /// but are guaranteed to be on the curve. That is, this method returns `None`
    /// if the x-coordinate corresponds to a non-curve point.
    ///
    /// The results are sorted by lexicographical order.
    /// This means that, if `P::BaseField: PrimeField`, the results are sorted as integers.
    pub fn get_ys_from_x_unchecked(x: P::Base) -> Option<(P::Base, P::Base)> {
        // Compute the curve equation x^3 + Ax + B.
        let x3_plus_b = P::add_b(x.squared() * x);
        let y = x3_plus_b.sqrt()?;
        let neg_y = -y;
        match y < neg_y {
            true => Some((y, neg_y)),
            false => Some((neg_y, y)),
        }
    }

    pub fn to_jacobian(self) -> G<P> {
        G {
            x: self.x,
            y: self.y,
            z: P::Base::one(),
        }
    }
}

impl<P: GroupParams> PartialEq for AffineG<P> {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl<P: GroupParams> Eq for AffineG<P> {}

impl<P: GroupParams> fmt::Debug for G<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "G({:?}, {:?}, {:?})", self.x, self.y, self.z)
    }
}

impl<P: GroupParams> Clone for G<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: GroupParams> Copy for G<P> {}

impl<P: GroupParams> Clone for AffineG<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: GroupParams> Copy for AffineG<P> {}

impl AffineG1 {
    pub fn double(&mut self) -> Self {
        #[cfg(target_os = "zkvm")]
        {
            let mut out = *self;
            unsafe { syscall_bn254_double(transmute(&mut out)) };
            out
        }
        #[cfg(not(target_os = "zkvm"))]
        {
            let p: G1 = (*self).to_jacobian();
            (p + p)
                .to_affine()
                .expect("Unable to convert G1 to AffineG1")
        }
    }
}

impl Add<AffineG1> for AffineG1 {
    type Output = AffineG1;

    // We only need the mutability for the zkvm case.
    #[allow(unused_mut)]
    fn add(mut self, other: AffineG1) -> AffineG1 {
        #[cfg(target_os = "zkvm")]
        {
            let mut out = self;
            if self == other {
                return self.double();
            }
            unsafe { syscall_bn254_add(transmute(&mut out), transmute(&other)) };
            out
        }
        #[cfg(not(target_os = "zkvm"))]
        {
            let p: G1 = self.to_jacobian();
            let q: G1 = other.to_jacobian();
            (p + q)
                .to_affine()
                .expect("Unable to convert G1 to AffineG1")
        }
    }
}

impl Sub<AffineG1> for AffineG1 {
    type Output = AffineG1;

    fn sub(self, other: AffineG1) -> AffineG1 {
        self + (-other)
    }
}

impl Mul<Fr> for AffineG1 {
    type Output = AffineG1;

    fn mul(self, other: Fr) -> AffineG1 {
        let mut res: Option<AffineG1> = None;
        let mut found_one = false;

        for i in U256::from(other).bits() {
            if found_one {
                res = res.map(|mut p| p.double());
            }

            #[allow(clippy::suspicious_arithmetic_impl)]
            if i {
                found_one = true;
                res = res.map(|p| p + self).or(Some(self));
            }
        }

        res.unwrap()
    }
}

impl<P: GroupParams> PartialEq for G<P> {
    fn eq(&self, other: &Self) -> bool {
        if self.is_zero() {
            return other.is_zero();
        }

        if other.is_zero() {
            return false;
        }

        let z1_squared = self.z.squared();
        let z2_squared = other.z.squared();

        if self.x * z2_squared != other.x * z1_squared {
            return false;
        }

        let z1_cubed = self.z * z1_squared;
        let z2_cubed = other.z * z2_squared;

        if self.y * z2_cubed != other.y * z1_cubed {
            return false;
        }

        true
    }
}
impl<P: GroupParams> Eq for G<P> {}

impl<P: GroupParams> G<P> {
    pub fn is_zero(&self) -> bool {
        self.z.is_zero()
    }
    
    pub fn double(&self) -> Self {
        let a = self.x.squared();
        let b = self.y.squared();
        let c = b.squared();
        let mut d = (self.x + b).squared() - a - c;
        d = d + d;
        let e = a + a + a;
        let f = e.squared();
        let x3 = f - (d + d);
        let mut eight_c = c + c;
        eight_c = eight_c + eight_c;
        eight_c = eight_c + eight_c;
        let y1z1 = self.y * self.z;

        G {
            x: x3,
            y: e * (d - x3) - eight_c,
            z: y1z1 + y1z1,
        }
    }
    
    pub fn to_affine(&self) -> Option<AffineG<P>> {
        if self.z.is_zero() {
            None
        } else if self.z == P::Base::one() {
            Some(AffineG {
                x: self.x,
                y: self.y,
            })
        } else {
            let zinv = self.z.inverse().unwrap();
            let zinv_squared = zinv.squared();

            Some(AffineG {
                x: self.x * zinv_squared,
                y: self.y * (zinv * zinv_squared),
            })
        }
    }
}

impl<P: GroupParams> GroupElement for G<P> {
    type Base = P::Base;
    
    fn zero() -> Self {
        G {
            x: P::Base::zero(),
            y: P::Base::one(),
            z: P::Base::zero(),
        }
    }

    fn one() -> Self {
        P::one()
    }

    fn coeff_b() -> Self::Base {
        P::coeff_b()
    }
}

impl<P: GroupParams> Mul<Fr> for G<P> {
    type Output = G<P>;

    fn mul(self, other: Fr) -> G<P> {
        let mut res = G::zero();
        let mut found_one = false;

        for i in U256::from(other).bits() {
            if found_one {
                res = res.double();
            }

            #[allow(clippy::suspicious_arithmetic_impl)]
            if i {
                found_one = true;
                res = res + self;
            }
        }

        res
    }
}

impl<P: GroupParams> Add<G<P>> for G<P> {
    type Output = G<P>;

    fn add(self, other: G<P>) -> G<P> {
        if self.is_zero() {
            return other;
        }

        if other.is_zero() {
            return self;
        }

        let z1_squared = self.z.squared();
        let z2_squared = other.z.squared();
        let u1 = self.x * z2_squared;
        let u2 = other.x * z1_squared;
        let z1_cubed = self.z * z1_squared;
        let z2_cubed = other.z * z2_squared;
        let s1 = self.y * z2_cubed;
        let s2 = other.y * z1_cubed;

        if u1 == u2 && s1 == s2 {
            self.double()
        } else {
            let h = u2 - u1;
            let s2_minus_s1 = s2 - s1;
            let i = (h + h).squared();
            let j = h * i;
            let r = s2_minus_s1 + s2_minus_s1;
            let v = u1 * i;
            let s1_j = s1 * j;
            let x3 = r.squared() - j - (v + v);

            G {
                x: x3,
                y: r * (v - x3) - (s1_j + s1_j),
                z: ((self.z + other.z).squared() - z1_squared - z2_squared) * h,
            }
        }
    }
}
impl<P: GroupParams> AddAssign for G<P> {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl<P: GroupParams> AddAssign<&G<P>> for G<P> {
    fn add_assign(&mut self, rhs: &G<P>) {
        *self += *rhs;
    }
}

impl<P: GroupParams> Neg for G<P> {
    type Output = G<P>;

    fn neg(self) -> G<P> {
        if self.is_zero() {
            self
        } else {
            G {
                x: self.x,
                y: -self.y,
                z: self.z,
            }
        }
    }
}

impl<P: GroupParams> Neg for AffineG<P> {
    type Output = AffineG<P>;

    fn neg(self) -> AffineG<P> {
        AffineG {
            x: self.x,
            y: -self.y,
        }
    }
}

impl<P: GroupParams> Sub<G<P>> for G<P> {
    type Output = G<P>;

    fn sub(self, other: G<P>) -> G<P> {
        self + (-other)
    }
}

#[derive(Debug)]
pub struct G1Params;

impl GroupParams for G1Params {
    type Base = Fq;

    fn one() -> G<Self> {
        G {
            x: Fq::one(),
            y: const_fq([2, 0, 0, 0]),
            z: Fq::one(),
        }
    }

    fn coeff_b() -> Fq {
        const_fq([3, 0, 0, 0])
    }
}

pub type G1 = G<G1Params>;

pub type AffineG1 = AffineG<G1Params>;

#[derive(Debug)]
pub struct G2Params;

impl GroupParams for G2Params {
    type Base = Fq2;

    fn one() -> G<Self> {
        G {
            x: Fq2::new(
                const_fq([
                    0x46debd5cd992f6ed,
                    0x674322d4f75edadd,
                    0x426a00665e5c4479,
                    0x1800deef121f1e76,
                ]),
                const_fq([
                    0x97e485b7aef312c2,
                    0xf1aa493335a9e712,
                    0x7260bfb731fb5d25,
                    0x198e9393920d483a,
                ]),
            ),
            y: Fq2::new(
                const_fq([
                    0x4ce6cc0166fa7daa,
                    0xe3d1e7690c43d37b,
                    0x4aab71808dcb408f,
                    0x12c85ea5db8c6deb,
                ]),
                const_fq([
                    0x55acdadcd122975b,
                    0xbc4b313370b38ef3,
                    0xec9e99ad690c3395,
                    0x90689d0585ff075,
                ]),
            ),
            z: Fq2::one(),
        }
    }

    fn coeff_b() -> Fq2 {
        Fq2::new(
            const_fq([
                0x3267e6dc24a138e5,
                0xb5b4c5e559dbefa3,
                0x81be18991be06ac3,
                0x2b149d40ceb8aaae,
            ]),
            const_fq([
                0xe4a2bd0685c315d2,
                0xa74fa084e52d1852,
                0xcd2cafadeed8fdf4,
                0x9713b03af0fed4,
            ]),
        )
    }

    fn check_order() -> bool {
        true
    }
}

pub type G2 = G<G2Params>;

pub type AffineG2 = AffineG<G2Params>;

#[inline]
fn twist() -> Fq2 {
    Fq2::new(
        const_fq([9, 0, 0, 0]),
        const_fq([1, 0, 0, 0]),
    )
}

#[inline]
fn two_inv() -> Fq {
    const_fq([
        0x9e10460b6c3e7ea4,
        0xcbc0b548b438e546,
        0xdc2822db40c0ac2e,
        0x183227397098d014,
    ])
}

#[inline]
fn twist_mul_by_q_x() -> Fq2 {
    Fq2::new(
        const_fq([
            0x99e39557176f553d,
            0xb78cc310c2c3330c,
            0x4c0bec3cf559b143,
            0x2fb347984f7911f7,
        ]),
        const_fq([
            0x1665d51c640fcba2,
            0x32ae2a1d0b7c9dce,
            0x4ba4cc8bd75a0794,
            0x16c9e55061ebae20,
        ]),
    )
}

#[inline]
fn twist_mul_by_q_y() -> Fq2 {
    Fq2::new(
        const_fq([
            0xdc54014671a0135a,
            0xdbaae0eda9c95998,
            0xdc5ec698b6e2f9b9,
            0x63cf305489af5dc,
        ]),
        const_fq([
            0x82d37f632623b0e3,
            0x21807dc98fa25bd2,
            0x704b5a7ec796f2b,
            0x7c03cbcac41049a,
        ]),
    )
}

#[derive(PartialEq, Eq, Debug)]
pub struct EllCoeffs {
    pub ell_0: Fq2,
    pub ell_vw: Fq2,
    pub ell_vv: Fq2,
}

#[derive(PartialEq, Eq, Debug)]
pub struct G2Precomp {
    pub q: AffineG<G2Params>,
    pub coeffs: Vec<EllCoeffs>,
}

impl G2Precomp {
}

pub fn miller_loop_batch(g2_precomputes: &Vec<G2Precomp>, g1_vec: &Vec<AffineG<G1Params>>) -> Fq12 {
    let mut f = Fq12::one();

    let mut idx = 0;

    for i in ATE_LOOP_COUNT_NAF.iter() {
        f = f.squared();
        for (g2_precompute, g1) in g2_precomputes.iter().zip(g1_vec.iter()) {
            let c = &g2_precompute.coeffs[idx];
            f = f.mul_by_024(c.ell_0, c.ell_vw.scale(g1.y), c.ell_vv.scale(g1.x));
        }
        idx += 1;
        if *i != 0 {
            for (g2_precompute, g1) in g2_precomputes.iter().zip(g1_vec.iter()) {
                let c = &g2_precompute.coeffs[idx];
                f = f.mul_by_024(c.ell_0, c.ell_vw.scale(g1.y), c.ell_vv.scale(g1.x));
            }
            idx += 1;
        }
    }

    for (g2_precompute, g1) in g2_precomputes.iter().zip(g1_vec.iter()) {
        let c = &g2_precompute.coeffs[idx];
        f = f.mul_by_024(c.ell_0, c.ell_vw.scale(g1.y), c.ell_vv.scale(g1.x));
    }
    idx += 1;
    for (g2_precompute, g1) in g2_precomputes.iter().zip(g1_vec.iter()) {
        let c = &g2_precompute.coeffs[idx];
        f = f.mul_by_024(c.ell_0, c.ell_vw.scale(g1.y), c.ell_vv.scale(g1.x));
    }
    f
}


impl AffineG<G2Params> {
    fn mul_by_q(&self) -> Self {
        AffineG {
            x: twist_mul_by_q_x() * self.x.frobenius_map(1),
            y: twist_mul_by_q_y() * self.y.frobenius_map(1),
        }
    }

    pub fn precompute(&self) -> G2Precomp {
        let mut r = self.to_jacobian();

        let mut coeffs = Vec::with_capacity(102);

        let q_neg = self.neg();
        for i in ATE_LOOP_COUNT_NAF.iter() {
            coeffs.push(r.doubling_step_for_flipped_miller_loop());

            if *i == 1 {
                coeffs.push(r.mixed_addition_step_for_flipped_miller_loop(self));
            }
            if *i == 3 {
                coeffs.push(r.mixed_addition_step_for_flipped_miller_loop(&q_neg));
            }
        }
        let q1 = self.mul_by_q();
        let q2 = -(q1.mul_by_q());

        coeffs.push(r.mixed_addition_step_for_flipped_miller_loop(&q1));
        coeffs.push(r.mixed_addition_step_for_flipped_miller_loop(&q2));

        G2Precomp { q: *self, coeffs }
    }
}

impl G1 {
    pub fn msm_variable_base(points: &[G1], scalars: &[Fr]) -> G1 {
        points
            .iter()
            .zip(scalars)
            .map(|(&p, &s)| p * s)
            .fold(G1::zero(), |acc, p| acc + p)
    }
}

impl AffineG1 {
    pub fn msm_variable_base(points: &[AffineG1], scalars: &[Fr]) -> AffineG1 {
        points
            .iter()
            .zip(scalars)
            .map(|(&p, &s)| p * s)
            .fold(None, |acc: Option<AffineG1>, p| {
                acc.map(|acc| acc + p).or(Some(p))
            })
            .unwrap()
    }
}

impl G2 {
    fn mixed_addition_step_for_flipped_miller_loop(
        &mut self,
        base: &AffineG<G2Params>,
    ) -> EllCoeffs {
        let d = self.x - self.z * base.x;
        let e = self.y - self.z * base.y;
        let f = d.squared();
        let g = e.squared();
        let h = d * f;
        let i = self.x * f;
        let j = self.z * g + h - (i + i);

        self.x = d * j;
        self.y = e * (i - j) - h * self.y;
        self.z = self.z * h;

        EllCoeffs {
            ell_0: twist() * (e * base.x - d * base.y),
            ell_vv: e.neg(),
            ell_vw: d,
        }
    }

    fn doubling_step_for_flipped_miller_loop(&mut self) -> EllCoeffs {
        let a = (self.x * self.y).scale(two_inv());
        let b = self.y.squared();
        let c = self.z.squared();
        let d = c + c + c;
        let e = G2Params::coeff_b() * d;
        let f = e + e + e;
        let g = (b + f).scale(two_inv());
        let h = (self.y + self.z).squared() - (b + c);
        let i = e - b;
        let j = self.x.squared();
        let e_sq = e.squared();

        self.x = a * (b - f);
        self.y = g.squared() - (e_sq + e_sq + e_sq);
        self.z = b * h;

        EllCoeffs {
            ell_0: twist() * i,
            ell_vw: h.neg(),
            ell_vv: j + j + j,
        }
    }
}



pub fn pairing_batch(ps: &[G1], qs: &[G2]) -> Fq12 {
    let mut p_affines: Vec<AffineG<G1Params>> = Vec::new();
    let mut q_precomputes: Vec<G2Precomp> = Vec::new();
    for (p, q) in ps.iter().zip(qs.iter()) {
        let p_affine = p.to_affine();
        let q_affine = q.to_affine();
        let exists = match (p_affine, q_affine) {
            (None, _) | (_, None) => false,
            (Some(_p_affine), Some(_q_affine)) => true,
        };

        if exists {
            p_affines.push(p.to_affine().unwrap());
            q_precomputes.push(q.to_affine().unwrap().precompute());
        }
    }
    if q_precomputes.is_empty() {
        return Fq12::one();
    }
    miller_loop_batch(&q_precomputes, &p_affines)
        .final_exponentiation()
        .expect("miller loop cannot produce zero")
}
