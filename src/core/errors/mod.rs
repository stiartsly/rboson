mod argument_error;
mod before_valid_period;
mod crypto_error;
mod db_error;
mod expired_error;
mod io_error;
mod malformed;
mod network_error;
mod not_implemented;
mod operation_error;
mod permission_error;
mod protocol_error;
mod signature_error;
mod state_error;

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
