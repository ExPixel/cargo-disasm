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
    fn tms320c64x_size_and_alignment() {
        assert_eq!(
            core::mem::size_of::<Details>(),
            sys::get_test_val("sizeof(cs_tms320c64x)")
        );

        assert_eq!(
            core::mem::align_of::<Details>(),
            sys::get_test_val("alignof(cs_tms320c64x)")
        );
    }
}
