/// Implements [trait@std::ops::Deref] and optionally [trait@std::ops::DerefMut] for a given type
/// and one if it's fields.
///
/// This is done through proc macros because when it's done through derive macros,
/// some LSPs and completion engines have trouble understanding that the type can
/// be dereferenced to another type.
///
/// # Examples
/// To implement Deref:
/// ```
/// struct SomeData {
///     data: String,
/// }
/// impl_deref!(SomeData, data, String);
/// ```
///
/// To implement both Deref and DerefMut:
/// ```
/// struct SomeData {
///     data: String,
/// }
/// impl_deref!(mut SomeData, data, String);
/// ```
///
/// # Panics
/// ```
/// struct SomeData {
///     data: String,
/// }
/// impl_deref!(SomeData, data, String);
///
/// fn do_something() {
///     let mut some_data = SomeData::default();
///     some_data += "yeet";
/// }
/// ```
///
#[macro_export]
macro_rules! impl_deref {
    // deref mut
    (mut $type_name:ty, $target_name:ident, $target_type_name:ty) => {
        impl std::ops::Deref for $type_name {
            type Target = $target_type_name;

            fn deref(&self) -> &Self::Target {
                &self.$target_name
            }
        }

        impl std::ops::DerefMut for $type_name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.$target_name
            }
        }
    };

    // deref
    ($type_name:ty, $target_name:ident, $target_type_name:ty) => {
        impl std::ops::Deref for $type_name {
            type Target = $target_type_name;

            fn deref(&self) -> &Self::Target {
                &self.$target_name
            }
        }
    };

    // deref mut single tuple struct
    (mut $type_name:ty, $target_type_name:ty) => {
        impl std::ops::Deref for $type_name {
            type Target = $target_type_name;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl std::ops::DerefMut for $type_name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };

    // deref single tuple struct
    ($type_name:ty, $target_type_name:ty) => {
        impl std::ops::Deref for $type_name {
            type Target = $target_type_name;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

/// Shorthand for writing smaller std::default::Default implementations.
#[macro_export]
macro_rules! impl_default {
    ($type_name:ty, $expression:expr) => {
       impl std::default::Default for $type_name {
        fn default() -> Self {
            $expression
        }
       }
    }
}

/// Shorthand for giving std::fmt::Display the same output as std::fmt::Debug.
#[macro_export]
macro_rules! impl_display_debug {
    ($type_name:ty) => {
        impl std::fmt::Display for $type_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", self)
            }
        }
    };
}

/// Shorthand for giving std::fmt::Display the same output as std::fmt::Debug prettified output.
#[macro_export]
macro_rules! impl_display_debug_pretty {
    ($type_name:ty) => {
        impl std::fmt::Display for $type_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:#?}", self)
            }
        }
    };
}
