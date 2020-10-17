macro_rules! result {
    ($error:expr, $good:expr) => {
        match $error {
            $crate::sys::Error(0) => Ok($good),

            $crate::sys::Error(err) => Err(<$crate::Error as core::convert::TryFrom<
                libc::c_int,
            >>::try_from(err)
            .unwrap_or($crate::Error::Bindings)),
        }
    };

    ($error:expr) => {
        result!($error, ())
    };
}

macro_rules! c_enum {
    (
        $(#[$enum_meta:meta])*
        $vis:vis enum $EnumName:ident $(: $($Primitive:path),*)? {
            $(
                $(#[$variant_meta:meta])*
                $Variant:ident $(= $Value:expr)?
            ),*
            $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[repr(C)]
        $vis enum $EnumName {
            $(
                $(#[$variant_meta])*
                $Variant $(= $Value)?
            ),*
        }

        impl core::convert::TryFrom<libc::c_int> for $EnumName {
            type Error = ();

            fn try_from(primitive: libc::c_int) -> Result<Self, Self::Error> {
                match primitive {
                    $( _ if primitive == Self::$Variant as libc::c_int => Ok(Self::$Variant) ,)*
                    _ => Err(()),
                }
            }
        }

        impl core::convert::From<$EnumName> for libc::c_int {
            fn from(e: $EnumName) -> libc::c_int{
                e as libc::c_int
            }
        }

        $($(
            impl core::convert::From<$EnumName> for $Primitive {
                fn from(e: $EnumName) -> $Primitive {
                    e as libc::c_int as $Primitive
                }
            }

            impl core::convert::TryFrom<$Primitive> for $EnumName {
                type Error = ();

                fn try_from(primitive: $Primitive) -> Result<Self, Self::Error> {
                    // FIXME: while this does guard against possible undefined behavior through bad
                    // values, as long as the C API does not have a bug it should be fine to
                    // use an `as libc::c_int` instead. I will leave this here for now though.
                    if let Ok(p) = <libc::c_int as core::convert::TryFrom<$Primitive>>::try_from(primitive) {
                        <$EnumName as core::convert::TryFrom<libc::c_int>>::try_from(p)
                    } else {
                        Err(())
                    }
                }
            }
        )*)?
    };
}

macro_rules! c_enum_big {
    (
        $(#[$enum_meta:meta])*
        $vis:vis enum $EnumName:ident $(: $($Primitive:path),*)? {
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
        #[repr(C)]
        $vis enum $EnumName {
            $(
                $(#[$variant_meta])*
                $Variant $(= $Value)?
            ),*
        }

        impl core::convert::TryFrom<libc::c_int> for $EnumName {
            type Error = ();

            fn try_from(primitive: libc::c_int) -> Result<Self, Self::Error> {
                if primitive < $EnumName::$StartVariant as libc::c_int || primitive >= $EnumName::$EndVariant as libc::c_int {
                    return Err(());
                }
                Ok(unsafe { core::mem::transmute::<libc::c_int, $EnumName>(primitive) })
            }
        }

        impl core::convert::From<$EnumName> for libc::c_int {
            fn from(e: $EnumName) -> libc::c_int {
                e as libc::c_int
            }
        }

        $($(
            impl core::convert::From<$EnumName> for $Primitive {
                fn from(e: $EnumName) -> $Primitive {
                    e as libc::c_int as $Primitive
                }
            }

            impl core::convert::TryFrom<$Primitive> for $EnumName {
                type Error = ();

                fn try_from(primitive: $Primitive) -> Result<Self, Self::Error> {
                    // FIXME: while this does guard against possible undefined behavior through bad
                    // values, as long as the C API does not have a bug it should be fine to
                    // use an `as libc::c_int` instead. I will leave this here for now though.
                    if let Ok(p) = <libc::c_int as core::convert::TryFrom<$Primitive>>::try_from(primitive) {
                        <$EnumName as core::convert::TryFrom<libc::c_int>>::try_from(p)
                    } else {
                        Err(())
                    }
                }
            }
        )*)?
    };
}
