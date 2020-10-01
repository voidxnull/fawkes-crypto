use ff_uint::{construct_primefield_params, Num};
use crate::engines::U256;
use crate::native::ecc::{EdwardsPoint, JubJubParams};
use crate::constants::SEED_EDWARDS_G;

construct_primefield_params! {
    pub struct _Fq(super::U256);

    impl PrimeFieldParams for _Fq {
        type Inner = super::U256;
        const MODULUS: &'static str = "21888242871839275222246405745257275088696311157297823662689037894645226208583";
        const GENERATOR: &'static str = "2";
   }
}


construct_primefield_params! {
    pub struct _Fr(super::U256);

    impl PrimeFieldParams for _Fr {
        type Inner = super::U256;
        const MODULUS: &'static str = "21888242871839275222246405745257275088548364400416034343698204186575808495617";
        const GENERATOR: &'static str = "7";
   }
}


construct_primefield_params! {
    pub struct _Fs(super::U256);

    impl PrimeFieldParams for _Fs {
        type Inner = super::U256;
        const MODULUS: &'static str = "2736030358979909402780800718157159386076813972158567259200215660948447373041";
        const GENERATOR: &'static str = "7";
   }
}

pub type Fq = _Fq;
pub type Fr = _Fr;
pub type Fs = _Fs;



#[derive(Clone)]
pub struct JubJubBN256 {
    edwards_g: EdwardsPoint<Fr>,
    edwards_d: Num<Fr>,
    montgomery_a: Num<Fr>,
    montgomery_b: Num<Fr>,
    montgomery_u: Num<Fr>,
}


impl JubJubBN256 {
    pub fn new() -> Self {
        let edwards_d = -Num::from(168696)/Num::from(168700);

        let montgomery_a = Num::from(2)*(Num::ONE-edwards_d)/(Num::ONE+edwards_d);
        let montgomery_b = -Num::from(4)/(Num::ONE+edwards_d);
        
        // value of montgomery polynomial for x=montgomery_b (has no square root in Fr)
        let montgomery_u= Num::from(337401);

        let edwards_g = EdwardsPoint::from_scalar_raw(Num::from_seed(SEED_EDWARDS_G), montgomery_a, montgomery_b, montgomery_u);
        Self {
            edwards_g,
            edwards_d,
            montgomery_a,
            montgomery_b,
            montgomery_u
        }
    }
}



impl JubJubParams for JubJubBN256 {
    type Fr = Fr;
    type Fs = Fs;

    fn edwards_g(&self) -> &EdwardsPoint<Fr> {
        &self.edwards_g
    }


    fn edwards_d(&self) -> Num<Fr> {
        self.edwards_d
    }


    fn montgomery_a(&self) -> Num<Fr> {
        self.montgomery_a
    }

    fn montgomery_b(&self) -> Num<Fr> {
        self.montgomery_b
    }

    fn montgomery_u(&self) -> Num<Fr> {
        self.montgomery_u
    }
}