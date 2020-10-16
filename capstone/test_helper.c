#include <capstone/capstone.h>
#include <capstone/platform.h>
#include <stddef.h>

#ifndef offsetof
#define ep_offset_of(type, field) ((size_t) &(((type*)0)->field))
#else
#define ep_offset_of(type, field) offsetof(type, field)
#endif


#define ep_align_of(type) ep_offset_of(struct { char __a; type __b; }, __b)

#define ep_helper__define_size_helper(type)             \
    CAPSTONE_EXPORT                                     \
    size_t CAPSTONE_API ep_helper__sizeof_##type() {    \
        return sizeof(type);                            \
    }                                                   \
                                                        \
    CAPSTONE_EXPORT                                     \
    size_t CAPSTONE_API ep_helper__alignof_##type() {   \
        return ep_align_of(type);                       \
    }                                                   \

ep_helper__define_size_helper(cs_insn)
ep_helper__define_size_helper(cs_detail)

ep_helper__define_size_helper(cs_x86)
ep_helper__define_size_helper(cs_arm64)
ep_helper__define_size_helper(cs_arm)
ep_helper__define_size_helper(cs_m68k)
ep_helper__define_size_helper(cs_mips)
ep_helper__define_size_helper(cs_ppc)
ep_helper__define_size_helper(cs_sparc)
ep_helper__define_size_helper(cs_sysz)
ep_helper__define_size_helper(cs_xcore)
ep_helper__define_size_helper(cs_tms320c64x)
ep_helper__define_size_helper(cs_m680x)
ep_helper__define_size_helper(cs_evm)
ep_helper__define_size_helper(cs_mos65xx)
