macro_rules! result {
    ($error:expr, $good:expr) => {
        match $error {
            $crate::sys::Error(0) => Ok($good),

            $crate::sys::Error(err) => {
                Err($crate::Error::from_c(err).unwrap_or($crate::Error::Bindings))
            }
        }
    };

    ($error:expr) => {
        result!($error, ())
    };
}

macro_rules! c_enum {
    (
        $(#[$enum_meta:meta])*
        $vis:vis enum $EnumName:ident: $Primitive:ident {
            $(
                $(#[$variant_meta:meta])*
                $Variant:ident $(= $Value:expr)?
            ),*
            $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[repr($Primitive)]
        $vis enum $EnumName {
            $(
                $(#[$variant_meta])*
                $Variant $(= $Value)?
            ),*
        }

        impl $EnumName {
            /// Converts this enum to its primitive value.
            #[allow(dead_code)]
            pub(crate) fn to_primitive(self) -> $Primitive {
                self as $Primitive
            }

            /// Converts a primitive value to this enum.
            #[allow(dead_code)]
            pub(crate) fn from_primitive(primitive: $Primitive) -> Option<Self> {
                match primitive {
                    $( _ if primitive == Self::$Variant as $Primitive => Some(Self::$Variant) ,)*
                    _ => None,
                }
            }

            /// Converts this to its C value.
            #[allow(dead_code)]
            pub(crate) fn to_c(self) -> libc::c_int {
                self as $Primitive as libc::c_int
            }

            /// Converts from a C value into this.
            #[allow(dead_code)]
            pub(crate) fn from_c(c: libc::c_int) -> Option<Self> {
                if let Ok(v) = <libc::c_int as core::convert::TryInto<$Primitive>>::try_into(c) {
                    Self::from_primitive(v)
                } else {
                    None
                }
            }
        }
    };
}

macro_rules! c_enum_big {
    (
        $(#[$enum_meta:meta])*
        $vis:vis enum $EnumName:ident: $Primitive:ident {
            @Start = $StartVariant:ident,
            @End   = $EndVariant:ident,
            $(
                $(#[$variant_meta:meta])*
                $Variant:ident $(= $Value:expr)?
            ),*
            $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[repr($Primitive)]
        $vis enum $EnumName {
            $(
                $(#[$variant_meta])*
                $Variant $(= $Value)?
            ),*
        }

        impl $EnumName {
            /// Converts this enum to its primitive value.
            #[allow(dead_code)]
            pub(crate) fn to_primitive(self) -> $Primitive {
                self as $Primitive
            }

            /// Converts a primitive value to this enum.
            #[allow(dead_code)]
            pub(crate) fn from_primitive(primitive: $Primitive) -> Option<Self> {
                if primitive < $EnumName::$StartVariant as $Primitive || primitive >= $EnumName::$EndVariant as $Primitive {
                    return None;
                }
                Some(unsafe { core::mem::transmute::<$Primitive, $EnumName>(primitive) })
            }

            /// Converts this to its C value.
            #[allow(dead_code)]
            pub(crate) fn to_c(self) -> libc::c_int {
                self as $Primitive as libc::c_int
            }

            /// Converts from a C value into this.
            #[allow(dead_code)]
            pub(crate) fn from_c(c: libc::c_int) -> Option<Self> {
                if let Ok(v) = <libc::c_int as core::convert::TryInto<$Primitive>>::try_into(c) {
                    Self::from_primitive(v)
                } else {
                    None
                }
            }
        }
    };
}
