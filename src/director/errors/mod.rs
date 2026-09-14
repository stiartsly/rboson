use std::error::Error;

pub mod conflict_error;
pub mod director_server_error;
pub mod forbidden_error;
pub mod invalidrequest_error;
pub mod notfound_error;
pub mod passphraserequired_error;
pub mod ratelimit_error;
pub mod registrationdisabled_error;
pub mod servicebusy_error;
pub mod unauthorized_error;

pub trait DirectorError: Error {
    fn status(&self) -> u16;
}

pub use conflict_error::ConflictError;
pub use director_server_error::ServerError;
pub use forbidden_error::ForbiddenError;
pub use invalidrequest_error::InvalidRequestError;
pub use notfound_error::NotFoundError;
pub use passphraserequired_error::PassphraseRequiredError;
pub use ratelimit_error::RateLimitError;
pub use registrationdisabled_error::RegistrationDisabledError;
pub use servicebusy_error::ServiceBusyError;
pub use unauthorized_error::UnauthorizedError;
