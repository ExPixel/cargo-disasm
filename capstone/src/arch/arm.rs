#[repr(C)]
#[derive(Clone, Copy)]
pub struct Details {
    placeholder: [u8; 1768],
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::sys;

    #[test]
    fn arm_size_and_alignment() {
        assert_eq!(
            core::mem::size_of::<Details>(),
            sys::get_test_val("sizeof(cs_arm)")
        );

        assert_eq!(
            core::mem::align_of::<Details>(),
            sys::get_test_val("alignof(cs_arm)")
        );
    }
}
