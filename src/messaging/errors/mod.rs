
pub mod auth;
pub mod encoding;
pub mod notfound;
pub mod protocol;

pub use {
    auth::AuthenticationError,
    encoding::EncodingError,
    notfound::NotFoundError,
    protocol::ProtocolError,
};
