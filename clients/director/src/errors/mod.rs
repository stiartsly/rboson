// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

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
