use bellman::{
    SynthesisError,
    ConstraintSystem
};

use bellman::pairing::{
    Engine
};



use super::signal::Signal;
use crate::ecc::{JubJubParams};

#[derive(Clone)]
pub struct EdwardsPoint<E:Engine> {
    pub x: Signal<E>,
    pub y: Signal<E>
}

#[derive(Clone)]
pub struct MontgomeryPoint<E:Engine> {
    pub x: Signal<E>,
    pub y: Signal<E>
}



impl<E:Engine> EdwardsPoint<E> {
    pub fn alloc<CS:ConstraintSystem<E>>(mut cs:CS, p: Option<crate::ecc::EdwardsPoint<E>>) -> Result<Self, SynthesisError> {
        let (x_value, y_value) = match p {
            Some(p) => {
                let (x,y) = p.into_xy();
                (Some(x), Some(y))
            },
            None => (None, None)
        };

        let x = Signal::alloc(cs.namespace(|| "alloc x"), x_value)?;
        let y = Signal::alloc(cs.namespace(|| "alloc y"), y_value)?;
        Ok(Self {x, y})
    }


    pub fn double<CS:ConstraintSystem<E>, J:JubJubParams<E>>(&self, mut cs:CS, params: &J) -> Result<Self, SynthesisError>{
        let v = self.x.multiply(cs.namespace(|| "xy"), &self.y)?;
        let v2 = v.square(cs.namespace(|| "x^2 y^2"))?;
        let u = (&self.x+&self.y).square(cs.namespace(|| "(x+y)^2"))?;
        let new_x = (&v+&v).divide(cs.namespace(|| "compute point.x"), &(Signal::one() +  params.edwards_d()*&v2))?;
        let new_y = (&u-&v-&v).divide(cs.namespace(|| "compute point.x"), &(Signal::one() -  params.edwards_d()*&v2))?;
        Ok(Self {x: new_x, y: new_y})
    }

    pub fn mul_cofactor<CS:ConstraintSystem<E>, J:JubJubParams<E>>(&self, mut cs:CS, params: &J) -> Result<Self, SynthesisError>{
        let p2 = self.double(cs.namespace(|| "2*self"), params)?;
        let p4 = p2.double(cs.namespace(|| "4*self"), params)?;
        let p8 = p4.double(cs.namespace(|| "8*self"), params)?;
        Ok(p8)
    }



    pub fn add<CS:ConstraintSystem<E>, J:JubJubParams<E>>(&self, mut cs:CS, p: &Self, params: &J) -> Result<Self, SynthesisError> {
        let v1 = self.x.multiply(cs.namespace(|| "x1y2"), &p.y)?;
        let v2 = p.x.multiply(cs.namespace(|| "x2y1"), &self.y)?;
        let v12 = v1.multiply(cs.namespace(|| "x1y2x2y1"), &v2)?;
        let u = (&self.x+&self.y).multiply(cs.namespace(|| "(x1+y1)*(x2+y2)"), &(&p.x+&p.y))?;
        let new_x = (&v1+&v2).divide(cs.namespace(|| "compute point.x"), &(Signal::one() +  params.edwards_d()*&v12))?;
        let new_y = (&u-&v1-&v2).divide(cs.namespace(|| "compute point.x"), &(Signal::one() -  params.edwards_d()*&v12))?;
        Ok(Self {x: new_x, y: new_y})
    }

    pub fn assert_in_curve<CS:ConstraintSystem<E>, J:JubJubParams<E>>(&self, mut cs:CS, params: &J) -> Result<(), SynthesisError> {
        let x2 = self.x.square(cs.namespace(|| "x^2"))?;
        let y2 = self.y.square(cs.namespace(|| "y^2"))?;
        cs.enforce(|| "point should be on curve", |_| y2.lc(), |zero| zero + CS::one() - (params.edwards_d().into_inner(), &y2.lc()), |zero| zero + CS::one() + &x2.lc());
        Ok(())
    }

    pub fn assert_in_subgroup<CS:ConstraintSystem<E>, J:JubJubParams<E>>(&self, mut cs:CS, params: &J) -> Result<(), SynthesisError> {
        let preimage_value = match (self.x.get_value(), self.y.get_value()) {
            (Some(x), Some(y)) => {
                let p = crate::ecc::EdwardsPoint::from_xy_unchecked(x, y);
                Some(p.mul(params.edwards_inv_cofactor().into_repr(), params))
            },
            _ => None
        };

        let preimage = EdwardsPoint::alloc(cs.namespace(|| "alloc preimage point"), preimage_value)?;
        let preimage8 = preimage.mul_cofactor(cs.namespace(|| "8*preimage"), params)?;

        (&self.x - &preimage8.x).assert_zero(cs.namespace(|| "assert x equality"))?;
        (&self.y - &preimage8.y).assert_zero(cs.namespace(|| "assert y equality"))?;
        
        Ok(())
    }

    // assume nonzero subgroup point
    pub fn into_montgomery<CS:ConstraintSystem<E>>(&self, mut cs:CS) -> Result<MontgomeryPoint<E>, SynthesisError> {
        let x = (&Signal::one() + &self.y).divide(cs.namespace(|| "compute montgomery x"), &(Signal::one() - &self.y))?;
        let y = x.divide(cs.namespace(|| "compute montgomery y"), &self.x)?;
        Ok(MontgomeryPoint {x, y})
    }

    pub fn switch<CS:ConstraintSystem<E>>(&self, mut cs:CS, bit:&Signal<E>, if_else:&Self) -> Result<Self, SynthesisError> {
        let x = self.x.switch(cs.namespace(|| "switch x"), bit, &if_else.x)?;
        let y = self.y.switch(cs.namespace(|| "switch y"), bit, &if_else.y)?;
        Ok(Self {x, y})
    }

    // assume subgroup point, bits
    pub fn multiply<CS:ConstraintSystem<E>, J:JubJubParams<E>>(&self, mut cs:CS, bits:&[Signal<E>], params: &J) -> Result<Self, SynthesisError> {
        match (&self.x, &self.y) {
            (&Signal::Constant(x), &Signal::Constant(y)) => {
                //TODO implement constant exponentiation
                Err(SynthesisError::AssignmentMissing)
            },
            _ => {
                let base_is_zero = self.x.is_zero(cs.namespace(|| "check is base zero"))?;

                let g8 = params.edwards_g8();
                let dummy_point = EdwardsPoint {x: Signal::Constant(g8.x), y: Signal::Constant(g8.y)};
        
                let base_point = dummy_point.switch(cs.namespace(|| "optional switch point to dummy"), &base_is_zero, self)?;
                let mut base_point = base_point.into_montgomery(cs.namespace(|| "convert point to montgomery"))?;
        
                let mut exponents = vec![base_point.clone()];
        
                for i in 1..bits.len() {
                    base_point = base_point.double(cs.namespace(|| format!("{}th doubling", i)), params)?;
                    exponents.push(base_point.clone());
                }
        
                let empty_acc = MontgomeryPoint {x:Signal::zero(), y:Signal::zero()};
                let mut acc = empty_acc.clone();
        
                for i in 0..bits.len() {
                    let inc_acc = acc.add(cs.namespace(|| format!("{}th addition", i)), &exponents[i], params)?;
                    acc = inc_acc.switch(cs.namespace(|| format!("{}th switch", i)), &bits[i], &acc)?;
                }
        
                acc = empty_acc.switch(cs.namespace(|| "optional switch acc to empty"), &base_is_zero, &acc)?;
        
                let res = acc.into_edwards(cs.namespace(|| "convert point to edwards"))?;
        
                // reduce the accumulator to (0, -1) point in edwards representation
                Ok(EdwardsPoint {x:-res.x, y:-res.y})
            }
        }
    }
}


impl<E:Engine> MontgomeryPoint<E> {
    pub fn alloc<CS:ConstraintSystem<E>>(mut cs:CS, p: Option<crate::ecc::EdwardsPoint<E>>) -> Result<Self, SynthesisError> {
        let (x_value, y_value) = match p {
            Some(p) => {
                let (x,y) = p.into_xy();
                (Some(x), Some(y))
            },
            None => (None, None)
        };

        let x = Signal::alloc(cs.namespace(|| "alloc x"), x_value)?;
        let y = Signal::alloc(cs.namespace(|| "alloc y"), y_value)?;
        Ok(Self {x, y})
    }

    // assume self != (0, 0)
    pub fn double<CS:ConstraintSystem<E>, J:JubJubParams<E>>(&self, mut cs:CS, params: &J) -> Result<Self, SynthesisError> {
        let x2 = self.x.square(cs.namespace(|| "compute x^2"))?;
        let ax = params.montgomery_a().clone() * &self.x;
        let by = params.montgomery_b().clone() * &self.y;

        let l = (&x2 + &x2 + &x2 + &ax + &ax + &Signal::one()).divide(cs.namespace(|| "compute (3 x^2 + 2 a x + 1)/(2 b y)"), &(&by + &by))?;
        
        let b_l2 = params.montgomery_b().clone()*&l.square(cs.namespace(|| "compute l^2"))?;
        let a = Signal::Constant(params.montgomery_a().clone());
    
        let x = &b_l2 - &a - &self.x - &self.x;
        let y = l.multiply(cs.namespace(|| "compute (3 x + A - B*l^2)*l"), &(&self.x + &self.x + &self.x + &a - &b_l2))? - &self.y;

        Ok(Self {x, y})
    }

    // assume self != p
    pub fn add<CS:ConstraintSystem<E>, J:JubJubParams<E>>(&self, mut cs:CS, p: &Self, params: &J) -> Result<Self, SynthesisError> {
        let l = (&p.y - &self.y).divide(cs.namespace(|| "compute l"), &(&p.x - &self.x))?;
        let b_l2 = params.montgomery_b().clone()*&l.square(cs.namespace(|| "compute l^2"))?;
        let a = Signal::Constant(params.montgomery_a().clone());
    
        let x = &b_l2 - &a - &self.x - &p.x;
        let y = l.multiply(cs.namespace(|| "compute (2 x1 + x2 + A - B*l^2)*l"), &(&self.x + &self.x + &self.x + &a - &b_l2))? - &self.y;

        Ok(Self {x, y})
    }

    // assume any nonzero point
    pub fn into_edwards<CS:ConstraintSystem<E>>(&self, mut cs:CS) -> Result<EdwardsPoint<E>, SynthesisError> {
        let y_is_zero = self.y.is_zero(cs.namespace(|| "check (0, 0) point"))?;
        let x = self.x.divide(cs.namespace(|| "compute edwards x"), &(&self.y+&y_is_zero))?;
        let y = (&self.y - &Signal::one()).divide(cs.namespace(|| "compute edwards y"), &(&self.y+&Signal::one()))?;
        Ok(EdwardsPoint {x, y})
    }

    pub fn switch<CS:ConstraintSystem<E>>(&self, mut cs:CS, bit:&Signal<E>, if_else:&Self) -> Result<Self, SynthesisError> {
        let x = self.x.switch(cs.namespace(|| "switch x"), bit, &if_else.x)?;
        let y = self.y.switch(cs.namespace(|| "switch y"), bit, &if_else.y)?;
        Ok(Self {x, y})
    }
}
