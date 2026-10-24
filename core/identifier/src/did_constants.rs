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

pub(crate) const DID_SUFFIXED_SCHEME: &str = "did:";

pub const DID_SCHEME: &str = "did";
pub const DID_METHOD: &str = "boson";

pub const W3C_DID_CONTEXT: &str = "https://www.w3.org/ns/did/v1.1";
pub const BOSON_DID_CONTEXT: &str = "https://bosonnetwork.io/ns/did/v1";
pub const W3C_VC_CONTEXT: &str = "https://www.w3.org/ns/credentials/v2";
pub const BOSON_VC_CONTEXT: &str = "https://bosonnetwork.io/ns/credentials/v1";
pub const W3C_ED25519_CONTEXT: &str = "https://w3id.org/security/suites/ed25519-2020/v1";

pub const DEFAULT_VC_TYPE: &str = "VerifiableCredential";
pub const DEFAULT_VP_TYPE: &str = "VerifiablePresentation";

//#[allow(unused)]
pub const DEFAULT_VERIFICATION_METHOD_FRAGMENT: &str = "default";

//#[allow(unused)]
pub const BOSON_ID_FORMAT_W3C: &str = "boson.id.format.w3c-did";
