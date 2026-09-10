pub mod argument_error;
pub mod before_valid_period;
pub mod crypto_error;
pub mod db_error;
pub mod expired_error;
pub mod io_error;
pub mod malformed;
pub mod network_error;
pub mod not_implemented;
pub mod operation_error;
pub mod permission_error;
pub mod protocol_error;
pub mod signature_error;
pub mod state_error;

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;

pub use {
    argument_error::ArgumentError,
    before_valid_period::BeforeValidPeriodError,
    crypto_error::CryptoError,
    db_error::DBError,
    expired_error::ExpiredError,
    io_error::IOError,
    malformed::MalformedError,
    network_error::NetworkError,
    not_implemented::NotImplementedError,
    operation_error::OperationError,
    permission_error::PermissionError,
    protocol_error::ProtocolError,
    signature_error::SignatureError,
    state_error::StateError,
};
