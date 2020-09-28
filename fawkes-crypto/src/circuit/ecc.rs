use crate::core::signal::Signal;
use crate::circuit::bitify::c_into_bits_le_strict;
use crate::circuit::mux::c_mux3;
use crate::circuit::num::{CNum};
use crate::circuit::bool::{CBool};
use crate::native::ecc::{JubJubParams, EdwardsPoint, EdwardsPointEx, MontgomeryPoint};
use ff_uint::{Num, PrimeField};
use crate::circuit::cs::RCS;


#[derive(Clone, Signal)]
#[Value="EdwardsPoint<Fr>"]
pub struct CEdwardsPoint<Fr: PrimeField> {
    pub x: CNum<Fr>,
    pub y: CNum<Fr>
}


#[derive(Clone, Signal)]
#[Value="MontgomeryPoint<Fr>"]
pub struct CMontgomeryPoint<Fr: PrimeField> {
    pub x: CNum<Fr>,
    pub y: CNum<Fr>
}


impl<Fr: PrimeField> CEdwardsPoint<Fr> {

    pub fn double<J:JubJubParams<Fr=Fr>>(&self, params: &J) -> Self{
        let v = &self.x * &self.y;
        let v2 = v.square();
        let u = (&self.x + &self.y).square();
        Self {
            x: (&v*Num::from(2)).div_unchecked(&(Num::ONE + params.edwards_d()*&v2)),
            y: (&u-&v*Num::from(2)).div_unchecked(&(Num::ONE -  params.edwards_d()*&v2))
        }
    }

    pub fn mul_by_cofactor<J:JubJubParams<Fr=Fr>>(&self, params: &J) -> Self {
        self.double(params).double(params).double(params)
    }



    pub fn add<J:JubJubParams<Fr=Fr>>(&self, p: &Self, params: &J) -> Self {
        let v1 = &self.x * &p.y;
        let v2 = &p.x * &self.y;
        let v12 = &v1 * &v2;
        let u = (&self.x + &self.y) * (&p.x + &p.y);
        Self {
            x: (&v1+&v2).div_unchecked(&(Num::ONE +  params.edwards_d()*&v12)),
            y: (&u-&v1-&v2).div_unchecked(&(Num::ONE -  params.edwards_d()*&v12))
        }
    }

    pub fn assert_in_curve<J:JubJubParams<Fr=Fr>>(&self, params: &J) {
        let x2 = self.x.square();
        let y2 = self.y.square();
        
        (params.edwards_d()*&x2*&y2).assert_eq(&(&y2-&x2-Num::ONE));
    }

    pub fn assert_in_subgroup<J:JubJubParams<Fr=Fr>>(&self, params: &J) {
        let preimage_value = self.get_value().map(|p| p.mul(Num::from(8).checked_inv().unwrap(), params));
        let preimage = self.derive_alloc::<Self>(preimage_value.as_ref()); 
        preimage.assert_in_curve(params);
        let preimage8 = preimage.mul_by_cofactor(params);

        (&self.x - &preimage8.x).assert_zero();
        (&self.y - &preimage8.y).assert_zero();
    }

    pub fn subgroup_decompress<J:JubJubParams<Fr=Fr>>(x:&CNum<Fr>, params: &J) -> Self {
        let preimage_value = x.get_value()
            .map(|x| EdwardsPoint::subgroup_decompress(x, params)
            .unwrap_or(params.edwards_g().clone()).mul(Num::from(8).checked_inv().unwrap(), params));
        let preimage = CEdwardsPoint::alloc(x.get_cs(), preimage_value.as_ref()); 
        preimage.assert_in_curve(params);
        let preimage8 = preimage.mul_by_cofactor(params);
        (x - &preimage8.x).assert_zero();
        preimage8
    }

    // assume nonzero subgroup point
    pub fn into_montgomery(&self) -> CMontgomeryPoint<Fr> {
        let x = (Num::ONE + &self.y).div_unchecked(&(Num::ONE - &self.y));
        let y = x.div_unchecked(&self.x);
        CMontgomeryPoint {x, y}
    }

    // assume subgroup point, bits
    pub fn mul<J:JubJubParams<Fr=Fr>>(&self, bits:&[CBool<Fr>], params: &J) -> Self {
        fn gen_table<Fr:PrimeField, J:JubJubParams<Fr=Fr>>(p: &EdwardsPointEx<Fr>, params: &J) -> Vec<Vec<Num<Fr>>> {
            let mut x_col = vec![];
            let mut y_col = vec![];
            let mut q = p.clone();
            for _ in 0..8 {
                let MontgomeryPoint{x, y} = q.into_montgomery().unwrap();
                x_col.push(x);
                y_col.push(y);
                q = q.add(&p, params);
            }
            vec![x_col, y_col]
        }
        let cs = self.get_cs();

        match self.as_const() {        
            Some(c_base) => {
                let c_base = c_base.into_extended();
                let mut base = c_base;
                if base.is_zero() {
                    self.derive_const(&EdwardsPoint::zero())
                } else {
                    let bits_len = bits.len();
                    let zeros_len = (2 * bits_len) % 3;
                    let zero_bits = vec![CBool::from_const(cs, &false); zeros_len];
                    let all_bits = [bits, &zero_bits].concat();

                    let all_bits_len = all_bits.len();
                    let nwindows = all_bits_len / 3;

                    let mut acc = EdwardsPoint{x:Num::ZERO, y:-Num::ONE}.into_extended();
                    
                    for _ in 0..nwindows {
                        acc = acc.add(&base, params);
                        base = base.double().double().double();
                    }

                    let mp = acc.negate().into_montgomery().unwrap();

                    let mut acc = CMontgomeryPoint::from_const(cs, &mp);
                    let mut base = c_base;

        
                    for i in 0..nwindows {
                        let table = gen_table(&base, params);
                        let res = c_mux3(&all_bits[3*i..3*(i+1)], &table);
                        let p = CMontgomeryPoint {x: res[0].clone(), y: res[1].clone()};
                        acc = acc.add(&p, params);
                        base = base.double().double().double();
                    }
                    
                    let res = acc.into_edwards();
                    CEdwardsPoint {x:-res.x, y:-res.y}
                }
            },
            _ => {
                let base_is_zero = self.x.is_zero();
                let dummy_point = CEdwardsPoint::from_const(cs, params.edwards_g());
                let base_point = dummy_point.switch(&base_is_zero, self);

                let mut base_point = base_point.into_montgomery();
        
                let mut exponents = vec![base_point.clone()];
        
                for _ in 1..bits.len() {
                    base_point = base_point.double(params);
                    exponents.push(base_point.clone());
                }

        
                let empty_acc = CMontgomeryPoint {x:CNum::from_const(cs, &Num::ZERO), y:CNum::from_const(cs, &Num::ZERO)};
                let mut acc = empty_acc.clone();
        
                for i in 0..bits.len() {
                    let inc_acc = acc.add(&exponents[i], params);
                    acc = inc_acc.switch(&bits[i], &acc);
                }
        
                acc = empty_acc.switch(&base_is_zero, &acc);
        
                let res = acc.into_edwards();
                CEdwardsPoint {x:-res.x, y:-res.y}
            }
        }
    }

    // assuming t!=-1
    pub fn from_scalar<J:JubJubParams<Fr=Fr>>(t:&CNum<Fr>, params: &J) -> Self {

        fn filter_even<Fr:PrimeField>(x:Num<Fr>) -> Num<Fr> {
            if x.is_even() {x} else {-x}
        }

        fn check_and_get_y<Fr:PrimeField, J:JubJubParams<Fr=Fr>>(x:&CNum<Fr>, params: &J) -> (CBool<Fr>, CNum<Fr>) {
            let g = (x.square()*(x+params.montgomery_a())+x) / params.montgomery_b();

            let preimage_value = g.get_value().map(|g| {
                match g.sqrt() {
                    Some(g_sqrt) => filter_even(g_sqrt),
                    _ => filter_even((g*params.montgomery_u()).sqrt().unwrap())
                }
            });

            let preimage = x.derive_alloc(preimage_value.as_ref());
            let preimage_bits = c_into_bits_le_strict(&preimage);
            preimage_bits[0].assert_const(&false);

            let preimage_square = preimage.square();

            let is_square = (&g-&preimage_square).is_zero();
            let isnot_square = (&g*params.montgomery_u() - &preimage_square).is_zero();

            (&is_square ^ &isnot_square).assert_const(&true);
            (is_square, preimage)
        }


        let t = t + Num::ONE;

        let t2g1 = t.square()*params.montgomery_u();
        

        let x3 = - Num::ONE/params.montgomery_a() * (&t2g1 + Num::ONE);
        let x2 = x3.div_unchecked(&t2g1);

        let (is_valid, y2) = check_and_get_y(&x2, params);        
        let (_, y3) = check_and_get_y(&x3, params);

        let x = x2.switch(&is_valid, &x3);
        let y = y2.switch(&is_valid, &y3);

        CMontgomeryPoint {x, y}.into_edwards().mul_by_cofactor(params)
    }
}


impl<Fr: PrimeField> CMontgomeryPoint<Fr> {
    // assume self != (0, 0)
    pub fn double<J:JubJubParams<Fr=Fr>>(&self, params: &J) -> Self {
        let x2 = self.x.square();
        let l = (Num::from(3)*&x2 + Num::from(2)*params.montgomery_a()*&self.x + Num::ONE).div_unchecked(&(Num::from(2)*params.montgomery_b() * &self.y));
        let b_l2 = params.montgomery_b()*&l.square();
        let a = params.montgomery_a();

        Self {
            x: &b_l2 - &a - Num::from(2) * &self.x,
            y: l*(Num::from(3) * &self.x + a - &b_l2) - &self.y
        }
    }

    // assume self != p
    pub fn add<J:JubJubParams<Fr=Fr>>(&self, p: &Self, params: &J) -> Self {
        let l = (&p.y - &self.y).div_unchecked(&(&p.x - &self.x));
        let b_l2 = params.montgomery_b()*&l.square();
        let a = params.montgomery_a();
        
        Self {
            x: &b_l2 - &a - &self.x - &p.x,
            y: l*(Num::from(2) * &self.x + &p.x + a - &b_l2) - &self.y
        }
    }

    // assume any nonzero point
    pub fn into_edwards(&self) -> CEdwardsPoint<Fr> {
        let y_is_zero = self.y.is_zero();
        CEdwardsPoint {
            x: self.x.div_unchecked(&(&self.y + y_is_zero.to_num())),
            y: (&self.x - Num::ONE).div_unchecked(&(&self.x + Num::ONE))
        }
    }
}

