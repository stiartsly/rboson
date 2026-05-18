pub mod not_implemented;
pub mod argument_error;
pub mod signature_error;
pub mod permission_error;
pub mod protocol_error;
pub mod network_error;
pub mod crypto_error;
pub mod state_error;
pub mod io_error;
pub mod db_error;
pub mod before_valid_period;
pub mod expired_error;
pub mod malformed;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

pub use {
    not_implemented::NotImplementedError,
    argument_error::ArgumentError,
    signature_error::SignatureError,
    permission_error::PermissionError,
    protocol_error::ProtocolError,
    network_error::NetworkError,
    crypto_error::CryptoError,
    state_error::StateError,
    io_error::IOError,
    db_error::DBError,
    before_valid_period::BeforeValidPeriodError,
    expired_error::ExpiredError,
    malformed::MalformedError,
};
