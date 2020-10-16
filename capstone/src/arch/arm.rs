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
        assert_eq!(core::mem::size_of::<Details>(), unsafe {
            sys::ep_helper__sizeof_cs_arm() as usize
        });

        assert_eq!(core::mem::align_of::<Details>(), unsafe {
            sys::ep_helper__alignof_cs_arm() as usize
        });
    }
}
