use ff_uint::{Num, PrimeField};
use crate::circuit::{
    general::Variable,
    plonk::{cs::CS, bool::CBool},
    general::traits::{signal::Signal, num::SignalNum}
};
use std::cell::RefCell;
use std::rc::Rc;

use std::ops::{Add, Sub, Mul, Neg, Div, AddAssign, SubAssign, MulAssign, DivAssign};


#[derive(Clone, Debug)]
pub struct CNum<Fr:PrimeField> {
    pub value:Option<Num<Fr>>,
    // a*x + b
    pub lc: (Num<Fr>, Variable, Num<Fr>),
    pub cs: Rc<RefCell<CS<Fr>>>
}

impl<Fr:PrimeField> SignalNum for CNum<Fr> {
    type Bool = CBool<Fr>;
}

impl<Fr:PrimeField> Signal for CNum<Fr> {
    type Value = Num<Fr>;
    type CS = Rc<RefCell<CS<Fr>>>;

    fn as_const(&self) -> Option<Self::Value> {
        let lc = self.lc;
        if lc.0 == Num::ZERO {
            Some(lc.2)
        } else {
            None
        }
    }

    fn inputize(&self) {
        let var: Self = self.derive_alloc(self.get_value().as_ref());
        CS::enforce_pub(&var);
    }

    fn get_value(&self) -> Option<Self::Value> {
        self.value
    }

    fn from_const(cs:&Self::CS, value: &Self::Value) -> Self {
        let value = value.clone();
        Self {
            value: Some(value),
            lc: (Num::ZERO, Variable(0), value),
            cs:cs.clone()
        }
    }

    fn get_cs(&self) -> &Self::CS {
        &self.cs
    }

    fn alloc(cs:&Self::CS, value:Option<&Self::Value>) -> Self {
        let mut rcs = cs.borrow_mut();
        let v = Variable(rcs.n_vars);
        rcs.n_vars+=1;
        Self {value:value.cloned(), lc:(Num::ONE, v, Num::ZERO), cs:cs.clone()}
    }

    fn assert_const(&self, value: &Self::Value) {
        CS::enforce_add(self, &self.derive_const(&Num::ZERO), &self.derive_const(value))
    }
    
}


impl<Fr:PrimeField> CNum<Fr> {
    pub fn capacity(&self) -> usize {
        if self.lc.0==Num::ZERO {
            0
        } else {
            1
        }
    }

    pub fn assert_zero(&self) {
        self.assert_const(&Num::ZERO) 
    }


    pub fn assert_nonzero(&self) {
        match self.as_const() {
            Some(v) => {
                assert!(v!=Num::ZERO);
            },
            _ => {
                let inv_value = self.get_value().map(|v| if v.is_zero() {Num::ONE} else {v.checked_inv().unwrap()});
                let inv_signal = self.derive_alloc(inv_value.as_ref());
                CS::enforce_mul(self, &inv_signal, &self.derive_const(&Num::ONE));
            }
        }
    }

    pub fn assert_bit(&self) {
        CS::enforce_mul(self, &(self-Num::ONE), &self.derive_const(&Num::ZERO));
    }

    pub fn to_bool(&self) -> CBool<Fr> {
        CBool::new(self)
    }

    pub fn to_bool_unchecked(&self) -> CBool<Fr> {
        CBool::new_unchecked(self)
    }

    pub fn from_bool(b:CBool<Fr>) -> Self {
        b.to_num()
    }

    pub fn inv(&self) -> Self {
        match self.as_const() {
            Some(v) => {
                self.derive_const(&v.checked_inv().expect("Division by zero"))
            }
            _ => {
                self.assert_nonzero();
                let inv_value = self.get_value().map(|v| v.checked_inv().expect("Division by zero"));
                let inv_signal = self.derive_alloc(inv_value.as_ref());
                CS::enforce_mul(self, &inv_signal, &self.derive_const(&Num::ONE));
                inv_signal
            }
        }
    }

}


impl<Fr:PrimeField> std::ops::Neg for CNum<Fr> {
    type Output = CNum<Fr>;

    #[inline]
    fn neg(mut self) -> Self::Output {
        self.lc.0 = -self.lc.0;
        self.lc.2 = -self.lc.2;
        self
    }
}

forward_unop_ex!(impl<Fr:PrimeField> Neg for CNum<Fr>, neg);





impl<'l, Fr:PrimeField> AddAssign<&'l CNum<Fr>> for CNum<Fr> {
    #[inline]
    fn add_assign(&mut self, other: &'l CNum<Fr>)  {
        let cs = self.cs.clone();
        *self = if let Some(c) = self.as_const() {
            let value = other.value.map(|v| v+c);
            let mut lc = other.lc;
            lc.2+=c;
            Self {value, lc, cs}
        } else if let Some(c) = other.as_const() {
            let value = self.value.map(|v| v+c);
            let mut lc = self.lc;
            lc.2+=c;
            Self {value, lc, cs}
        } else if self.lc.1 == other.lc.1 {
            Self {
                value: self.value.map(|a| other.value.map(|b| a+b)).flatten(),
                lc: (self.lc.0+other.lc.0, self.lc.1, self.lc.2+other.lc.2),
                cs
            }
        } else {
            let value = self.value.map(|a| other.value.map(|b| a+b)).flatten();
            let var:Self = self.derive_alloc(value.as_ref());
            CS::enforce_add(self, other, &var);
            var
        }
    }
}

impl<'l, Fr:PrimeField> AddAssign<&'l Num<Fr>> for CNum<Fr> {
    #[inline]
    fn add_assign(&mut self, other: &'l Num<Fr>)  {
        *self += self.derive_const::<Self>(other)
    }
}


impl<'l, Fr:PrimeField> SubAssign<&'l CNum<Fr>> for CNum<Fr> {

    #[inline]
    fn sub_assign(&mut self, other: &'l CNum<Fr>)  {
        self.add_assign(&-other)
    }
}

impl<'l, Fr:PrimeField> SubAssign<&'l Num<Fr>> for CNum<Fr> {
    #[inline]
    fn sub_assign(&mut self, other: &'l Num<Fr>)  {
        *self -= self.derive_const::<Self>(other)
    }
}


impl<'l, Fr:PrimeField> MulAssign<&'l Num<Fr>> for CNum<Fr> {
    #[inline]
    fn mul_assign(&mut self, other: &'l Num<Fr>)  {
        self.lc.0*=other;
        self.lc.2*=other;
    }
}

impl<'l, Fr:PrimeField> DivAssign<&'l Num<Fr>> for CNum<Fr> {
    #[inline]
    fn div_assign(&mut self, other: &'l Num<Fr>)  {
        let inv = other.checked_inv().expect("Division by zero");
        self.mul_assign(&inv);
    }
}


impl<'l, Fr:PrimeField> MulAssign<&'l CNum<Fr>> for CNum<Fr> {
    #[inline]
    fn mul_assign(&mut self, other: &'l CNum<Fr>)  {
        let cs = self.cs.clone();
        *self = if let Some(c) = self.as_const() {
            let mut lc = other.lc;
            lc.0*=c;
            lc.2*=c;
            Self {value: other.value.map(|v| v*c), lc, cs}
        } else if let Some(c) = other.as_const() {
            let mut lc = self.lc;
            lc.0*=c;
            lc.2*=c;
            Self {value: self.value.map(|v| v*c), lc, cs}
        } else {
            let value = self.value.map(|a| other.value.map(|b| a*b)).flatten();
            let var = self.derive_alloc(value.as_ref());
            CS::enforce_mul(self, other, &var);
            var
        }
    }
}


impl<'l, Fr:PrimeField> DivAssign<&'l CNum<Fr>> for CNum<Fr> {
    #[inline]
    fn div_assign(&mut self, other: &'l CNum<Fr>)  {
        match (self.as_const(), other.as_const()) {
            (Some(a), _) => {*self = a*other.inv(); },
            (_, Some(b)) => {*self /= b},
            _ => {
                let value = self.value.map(|a| other.value.map(|b| a/b)).flatten();
                let var = self.derive_alloc(value.as_ref());
                other.assert_nonzero();
                CS::enforce_mul(&var, other, self);
                *self = var;
            }
        }
    }
}



forward_val_assign_ex!(impl<Fr:PrimeField> AddAssign<CNum<Fr>> for CNum<Fr>, add_assign);
forward_val_assign_ex!(impl<Fr:PrimeField> AddAssign<Num<Fr>> for CNum<Fr>, add_assign);

forward_val_assign_ex!(impl<Fr:PrimeField> SubAssign<CNum<Fr>> for CNum<Fr>, sub_assign);
forward_val_assign_ex!(impl<Fr:PrimeField> SubAssign<Num<Fr>> for CNum<Fr>, sub_assign);

forward_val_assign_ex!(impl<Fr:PrimeField> MulAssign<CNum<Fr>> for CNum<Fr>, mul_assign);
forward_val_assign_ex!(impl<Fr:PrimeField> MulAssign<Num<Fr>> for CNum<Fr>, mul_assign);

forward_val_assign_ex!(impl<Fr:PrimeField> DivAssign<CNum<Fr>> for CNum<Fr>, div_assign);
forward_val_assign_ex!(impl<Fr:PrimeField> DivAssign<Num<Fr>> for CNum<Fr>, div_assign);


impl<'l, Fr:PrimeField> Add<&'l CNum<Fr>> for CNum<Fr> {
    type Output = CNum<Fr>;

    #[inline]
    fn add(mut self, other: &'l CNum<Fr>) -> Self::Output  {
        self += other;
        self
    }
}

impl<'l, Fr:PrimeField> Add<&'l Num<Fr>> for CNum<Fr> {
    type Output = CNum<Fr>;

    #[inline]
    fn add(mut self, other: &'l Num<Fr>) -> Self::Output  {
        self += other;
        self
    }
}


impl<'l, Fr:PrimeField> Sub<&'l CNum<Fr>> for CNum<Fr> {
    type Output = CNum<Fr>;

    #[inline]
    fn sub(mut self, other: &'l CNum<Fr>) -> Self::Output  {
        self -= other;
        self
    }
}


impl<'l, Fr:PrimeField> Sub<&'l Num<Fr>> for CNum<Fr> {
    type Output = CNum<Fr>;

    #[inline]
    fn sub(mut self, other: &'l Num<Fr>) -> Self::Output  {
        self -= other;
        self
    }
}

impl<'l, Fr:PrimeField> Sub<&'l CNum<Fr>> for Num<Fr> {
    type Output = CNum<Fr>;

    #[inline]
    fn sub(self, other: &'l CNum<Fr>) -> Self::Output  {
        -other+self
    }
}


impl<'l, Fr:PrimeField> Mul<&'l Num<Fr>> for CNum<Fr> {
    type Output = CNum<Fr>;

    #[inline]
    fn mul(mut self, other: &'l Num<Fr>) -> Self::Output  {
        self *= other;
        self
    }
}

impl<'l, Fr:PrimeField> Mul<&'l CNum<Fr>> for CNum<Fr> {
    type Output = CNum<Fr>;

    #[inline]
    fn mul(mut self, other: &'l CNum<Fr>) -> Self::Output  {
        self *= other;
        self
    }
}


impl<'l, Fr:PrimeField> Div<&'l CNum<Fr>> for CNum<Fr> {
    type Output = CNum<Fr>;

    #[inline]
    fn div(mut self, other: &'l CNum<Fr>) -> Self::Output  {
        self /= other;
        self
    }
}


impl<'l, Fr:PrimeField> Div<&'l Num<Fr>> for CNum<Fr> {
    type Output = CNum<Fr>;

    #[inline]
    fn div(mut self, other: &'l Num<Fr>) -> Self::Output  {
        self /= other;
        self
    }
}

impl<'l, Fr:PrimeField> Div<&'l CNum<Fr>> for Num<Fr> {
    type Output = CNum<Fr>;

    #[inline]
    fn div(self, other: &'l CNum<Fr>) -> Self::Output  {
        other.inv()*self
    }
}




forward_all_binop_to_val_ref_ex!(impl<Fr:PrimeField> Sub<CNum<Fr>> for CNum<Fr>, sub -> CNum<Fr>);
forward_all_binop_to_val_ref_ex!(impl<Fr:PrimeField> Sub<CNum<Fr>> for Num<Fr>, sub -> CNum<Fr>);
forward_all_binop_to_val_ref_ex!(impl<Fr:PrimeField> Sub<Num<Fr>> for CNum<Fr>, sub -> CNum<Fr>);

forward_all_binop_to_val_ref_commutative_ex!(impl<Fr:PrimeField> Add for CNum<Fr>, add);
forward_all_binop_to_val_ref_ex!(impl<Fr:PrimeField> Add<Num<Fr>> for CNum<Fr>, add -> CNum<Fr>);
swap_commutative!(impl<Fr:PrimeField> Add<Num<Fr>> for CNum<Fr>, add);

forward_all_binop_to_val_ref_commutative_ex!(impl<Fr:PrimeField> Mul for CNum<Fr>, mul);
forward_all_binop_to_val_ref_ex!(impl<Fr:PrimeField> Mul<Num<Fr>> for CNum<Fr>, mul -> CNum<Fr>);
swap_commutative!(impl<Fr:PrimeField> Mul<Num<Fr>> for CNum<Fr>, mul);

forward_all_binop_to_val_ref_ex!(impl<Fr:PrimeField> Div<CNum<Fr>> for CNum<Fr>, div -> CNum<Fr>);
forward_all_binop_to_val_ref_ex!(impl<Fr:PrimeField> Div<Num<Fr>> for CNum<Fr>, div -> CNum<Fr>);
forward_all_binop_to_val_ref_ex!(impl<Fr:PrimeField> Div<CNum<Fr>> for Num<Fr>, div -> CNum<Fr>);
