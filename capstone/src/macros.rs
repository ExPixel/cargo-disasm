macro_rules! result {
    ($error:expr, $good:expr) => {
        match $error {
            $crate::sys::Error(0) => Ok($good),

            $crate::sys::Error(err) => {
                Err(<u8 as core::convert::TryFrom<libc::c_int>>::try_from(err)
                    .map_err(|_| ())
                    .and_then(<$crate::Error as core::convert::TryFrom<u8>>::try_from)
                    .unwrap_or($crate::Error::Bindings))
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
        $vis:vis enum $EnumName:ident:$Primitive:ident $(+ $ExtraPrimitive:path)* {
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

        impl core::convert::TryFrom<$Primitive> for $EnumName {
            type Error = ();

            fn try_from(primitive: $Primitive) -> Result<Self, Self::Error> {
                match primitive {
                    $( _ if primitive == Self::$Variant as $Primitive => Ok(Self::$Variant) ,)*
                    _ => Err(()),
                }
            }
        }

        impl core::convert::From<$EnumName> for $Primitive {
            fn from(e: $EnumName) -> $Primitive {
                e as $Primitive
            }
        }

        $(
            impl core::convert::From<$EnumName> for $ExtraPrimitive {
                fn from(e: $EnumName) -> $ExtraPrimitive {
                    e as $Primitive as $ExtraPrimitive
                }
            }

            impl core::convert::TryFrom<$ExtraPrimitive> for $EnumName {
                type Error = ();

                fn try_from(primitive: $ExtraPrimitive) -> Result<Self, Self::Error> {
                    // FIXME: while this does guard against possible undefined behavior through bad
                    // values, as long as the C API does not have a bug it should be fine to
                    // use an `as $Primitive` instead. I will leave this here for now though.
                    if let Ok(p) = <$Primitive as core::convert::TryFrom<$ExtraPrimitive>>::try_from(primitive) {
                        <$EnumName as core::convert::TryFrom<$Primitive>>::try_from(p)
                    } else {
                        Err(())
                    }
                }
            }
        )*
    };
}

macro_rules! c_enum_big {
    (
        $(#[$enum_meta:meta])*
        $vis:vis enum $EnumName:ident:$Primitive:ident $(+ $ExtraPrimitive:path)* {
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

        impl core::convert::TryFrom<$Primitive> for $EnumName {
            type Error = ();

            fn try_from(primitive: $Primitive) -> Result<Self, Self::Error> {
                if primitive < $EnumName::$StartVariant as $Primitive || primitive >= $EnumName::$EndVariant as $Primitive {
                    return Err(());
                }
                Ok(unsafe { core::mem::transmute::<$Primitive, $EnumName>(primitive) })
            }
        }

        impl core::convert::From<$EnumName> for $Primitive {
            fn from(e: $EnumName) -> $Primitive {
                e as $Primitive
            }
        }

        $(
            impl core::convert::From<$EnumName> for $ExtraPrimitive {
                fn from(e: $EnumName) -> $ExtraPrimitive {
                    e as $Primitive as $ExtraPrimitive
                }
            }

            impl core::convert::TryFrom<$ExtraPrimitive> for $EnumName {
                type Error = ();

                fn try_from(primitive: $ExtraPrimitive) -> Result<Self, Self::Error> {
                    // FIXME: while this does guard against possible undefined behavior through bad
                    // values, as long as the C API does not have a bug it should be fine to
                    // use an `as $Primitive` instead. I will leave this here for now though.
                    if let Ok(p) = <$Primitive as core::convert::TryFrom<$ExtraPrimitive>>::try_from(primitive) {
                        <$EnumName as core::convert::TryFrom<$Primitive>>::try_from(p)
                    } else {
                        Err(())
                    }
                }
            }
        )*
    };
}
