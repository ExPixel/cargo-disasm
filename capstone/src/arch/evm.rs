#[repr(C)]
#[derive(Clone, Copy)]
pub struct Details {
    x: u32,
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
