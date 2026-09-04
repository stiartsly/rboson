use std::{error::Error, fmt};

macro_rules! error_type {
    ($name:ident) => {
        #[derive(Debug)]
        pub struct $name(pub String);
        impl $name {
            pub fn new(message: impl Into<String>) -> Self {
                Self(message.into())
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
        impl Error for $name {}
    };
}

error_type!(RegistryError);
error_type!(ResolverError);
error_type!(ResolutionCacheError);
