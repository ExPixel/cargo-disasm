#include <capstone/capstone.h>
#include <capstone/platform.h>
#include <string.h>
#include <stddef.h>


#ifndef offsetof
#define ep_offset_of(type, field) ((size_t) &(((type*)0)->field))
#else
#define ep_offset_of(type, field) offsetof(type, field)
#endif
#define alignof(type) ep_offset_of(struct { char __a; type __b; }, __b)

struct ep_helper__entry {
    const char* name;
    size_t value;
};

struct ep_helper__entry ep_helper__entries[] = {
    { "sizeof(cs_insn)", sizeof(cs_insn) },
    { "alignof(cs_insn)", alignof(cs_insn) },

    { "sizeof(cs_detail)", sizeof(cs_detail) },
    { "alignof(cs_detail)", alignof(cs_detail) },

    { "sizeof(cs_x86)", sizeof(cs_x86) },
    { "alignof(cs_x86)", alignof(cs_x86) },

    { "sizeof(cs_arm64)", sizeof(cs_arm64) },
    { "alignof(cs_arm64)", alignof(cs_arm64) },

    { "sizeof(cs_arm)", sizeof(cs_arm) },
    { "alignof(cs_arm)", alignof(cs_arm) },

    { "sizeof(cs_m68k)", sizeof(cs_m68k) },
    { "alignof(cs_m68k)", alignof(cs_m68k) },

    { "sizeof(cs_mips)", sizeof(cs_mips) },
    { "alignof(cs_mips)", alignof(cs_mips) },

    { "sizeof(cs_ppc)", sizeof(cs_ppc) },
    { "alignof(cs_ppc)", alignof(cs_ppc) },

    { "sizeof(cs_sparc)", sizeof(cs_sparc) },
    { "alignof(cs_sparc)", alignof(cs_sparc) },

    { "sizeof(cs_sysz)", sizeof(cs_sysz) },
    { "alignof(cs_sysz)", alignof(cs_sysz) },

    { "sizeof(cs_xcore)", sizeof(cs_xcore) },
    { "alignof(cs_xcore)", alignof(cs_xcore) },

    { "sizeof(cs_tms320c64x)", sizeof(cs_tms320c64x) },
    { "alignof(cs_tms320c64x)", alignof(cs_tms320c64x) },

    { "sizeof(cs_m680x)", sizeof(cs_m680x) },
    { "alignof(cs_m680x)", alignof(cs_m680x) },

    { "sizeof(cs_evm)", sizeof(cs_evm) },
    { "alignof(cs_evm)", alignof(cs_evm) },

    { "sizeof(cs_mos65xx)", sizeof(cs_mos65xx) },
    { "alignof(cs_mos65xx)", alignof(cs_mos65xx) },

    { "X86_REG_ENDING", (size_t)X86_REG_ENDING },
    { "X86_INS_ENDING", (size_t)X86_INS_ENDING },
    { "X86_GRP_ENDING", (size_t)X86_GRP_ENDING },
};

CAPSTONE_EXPORT
size_t CAPSTONE_API ep_helper__get_value(const char* value_name, size_t value_name_len) {
    int entry_count = sizeof(ep_helper__entries)/sizeof(struct ep_helper__entry);
    for (int idx = 0; idx < entry_count; idx++) {
        struct ep_helper__entry* entry = &ep_helper__entries[idx];
        if (strncmp(entry->name, value_name, value_name_len) == 0)
            return entry->value;
    }
    return 0;
}
