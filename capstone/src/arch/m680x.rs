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
    fn m680x_size_and_alignment() {
        assert_eq!(core::mem::size_of::<Details>(), unsafe {
            sys::ep_helper__sizeof_cs_m680x() as usize
        });

        assert_eq!(core::mem::align_of::<Details>(), unsafe {
            sys::ep_helper__alignof_cs_m680x() as usize
        });
    }
}
