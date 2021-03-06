use super::generated::cs_evm;
use core::marker::PhantomData;

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Details<'c> {
    #[allow(dead_code)]
    inner: cs_evm,
    _phantom: PhantomData<&'c ()>,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sys;

    #[test]
    fn evm_size_and_alignment() {
        assert_eq!(
            core::mem::size_of::<Details>(),
            sys::get_test_val("sizeof(cs_evm)")
        );

        assert_eq!(
            core::mem::align_of::<Details>(),
            sys::get_test_val("alignof(cs_evm)")
        );
    }
}
