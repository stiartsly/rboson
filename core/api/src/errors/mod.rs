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

mod before_valid_period;
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

pub use boson_crypto::errors::{ArgumentError, CryptoError, Error, Result};

pub use {
    before_valid_period::BeforeValidPeriodError,
    db_error::DBError,
    expired_error::ExpiredError,
    io_error::IOError,
    malformed::MalformedError,
    network_error::NetworkError,
    not_implemented::NotImplemented,
    operation_error::OperationError,
    permission_error::PermissionError,
    protocol_error::ProtocolError,
    signature_error::SignatureError,
    state_error::StateError,
};
