
/// Define an error type wrapper e.g. `Foo` around a
/// `Box<FooKind>`. Implements the `std::error::Error`,
/// `std::ops::Deref` and `From` traits on `Foo` to make the
/// underlying `FooKind` transparently accessible. Thanks to the
/// `From` forwarding, `thiserror`'s `#[from]` syntax still
/// works. Also implements `Debug`. `FooKind` has to be defined
/// separately.
#[macro_export]
macro_rules! def_boxed_error {
    ($wrappername:ident, $kindname:ident) => {

        #[derive(Debug)]
        pub struct $wrappername(Box<$kindname>);

        impl std::ops::Deref for $wrappername {
            type Target = $kindname;

            fn deref(&self) -> &Self::Target {
                &*self.0
            }
        }

        impl<E> From<E> for $wrappername where $kindname: From<E> {
            fn from(err: E) -> Self {
                $wrappername(Box::new($kindname::from(err)))
            }
        }

        impl std::error::Error for $wrappername {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                self.0.source()
            }

            // Commented out unstable features:

            // fn type_id(&self, _: private::Internal) -> std::any::TypeId
            // where
            //     Self: 'static,
            // {
            //     std::any::TypeId::of::<Self>()
            // }

            // fn backtrace(&self) -> Option<&std::backtrace::Backtrace> {
            //     None
            // }

            fn description(&self) -> &str {
                "description() is deprecated; use Display"
            }

            fn cause(&self) -> Option<&dyn std::error::Error> {
                self.source()
            }
        }

        impl std::fmt::Display for $wrappername {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // Don't use the fmt method call as it can be
                // ambiguous (not sure why, but it's what it is)
                std::fmt::Display::fmt(&*self.0, f)
            }
        }
    }
}


/// Defines both an error type and its box wrapper as per
/// `def_boxed_error`. Implicitly derives `thiserror::Error` on the
/// inner type. This macro should make the code look less cluttered.
#[macro_export]
macro_rules! def_boxed_thiserror {
    ($wrappername:ident, $key1:tt $kindname:ident {$($body:tt)*}) => {
        crate::_def_boxed_thiserror!($wrappername, $kindname, $key1 $kindname {
            $($body)*
        });
    };
    ($wrappername:ident, $key1:tt $key2:tt $kindname:ident {$($body:tt)*}) => {
        crate::_def_boxed_thiserror!($wrappername, $kindname, $key1 $key2 $kindname {
            $($body)*
        });
    };
    ($wrappername:ident, $key1:tt $key2:tt $key3:tt $kindname:ident {$($body:tt)*}) => {
        crate::_def_boxed_thiserror!($wrappername, $kindname, $key1 $key2 $key3 $kindname {
            $($body)*
        });
    };
    ($wrappername:ident, $key1:tt $key2:tt $key3:tt $key4:tt $kindname:ident {$($body:tt)*}) => {
        crate::_def_boxed_thiserror!($wrappername, $kindname, $key1 $key2 $key3 $key4 $kindname {
            $($body)*
        });
    }
}

#[macro_export]
macro_rules! _def_boxed_thiserror {
    (
        $wrappername:ident,
        $kindname:ident,
        $($innerdef:tt)*
    ) => {
        #[derive(thiserror::Error, Debug)]
        $($innerdef)*
        
        crate::def_boxed_error!($wrappername, $kindname);
    }
}
