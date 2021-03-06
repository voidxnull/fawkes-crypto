use crate::{
    circuit::num::CNum, core::cs::ConstraintSystem, core::signal::Signal, native::num::Num,
};

#[derive(Debug, Clone)]
pub struct CBool<'a, CS: ConstraintSystem>(pub CNum<'a, CS>);

impl<'a, CS: ConstraintSystem> Signal<'a, CS> for CBool<'a, CS> {
    type Value = bool;

    fn get_cs(&self) -> &'a CS {
        self.0.get_cs()
    }

    fn inputize(&self) {
        self.0.inputize();
    }

    fn linearize_builder(&self, acc: &mut Vec<CNum<'a, CS>>) {
        self.0.linearize_builder(acc);
    }

    fn from_const(cs: &'a CS, value: &Self::Value) -> Self {
        CBool(CNum::from_const(cs, &Num::from(*value)))
    }

    fn get_value(&self) -> Option<Self::Value> {
        self.0.get_value().map(|n| n.into_bool())
    }

    fn as_const(&self) -> Option<Self::Value> {
        self.0.as_const().map(|v| v.into_bool())
    }

    fn alloc(cs: &'a CS, value: Option<&Self::Value>) -> Self {
        CBool(CNum::alloc(cs, value.map(|v| Num::from(*v)).as_ref()))
    }

    fn switch(&self, bit: &CBool<'a, CS>, if_else: &Self) -> Self {
        CBool(self.0.switch(bit, &if_else.0))
    }

    fn assert_const(&self, value: &Self::Value) {
        self.0.assert_const(&Num::from(*value));
    }

    fn assert_eq(&self, other: &Self) {
        self.0.assert_eq(&other.0);
    }

    fn is_eq(&self, other: &Self) -> CBool<'a, CS> {
        self.0.is_eq(&other.0)
    }
}

impl<'a, CS: ConstraintSystem> CBool<'a, CS> {
    #[inline]
    pub fn if_else<T: Signal<'a, CS>>(&self, if_true: &T, if_false: &T) -> T {
        if_true.switch(self, if_false)
    }

    pub fn into_num(&self) -> CNum<'a, CS> {
        self.0.clone()
    }

    #[inline]
    pub fn assert(&self) {
        self.0.assert_bit();
    }

    #[inline]
    pub fn assert_false(&self) {
        self.assert_const(&false);
    }

    #[inline]
    pub fn assert_true(&self) {
        self.assert_const(&true);
    }

    #[inline]
    pub fn c_true(cs: &'a CS) -> Self {
        Self::from_const(cs, &true)
    }

    #[inline]
    pub fn c_false(cs: &'a CS) -> Self {
        Self::from_const(cs, &false)
    }
}
