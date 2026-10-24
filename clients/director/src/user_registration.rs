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

#[derive(Clone, Debug, Default)]
pub struct UserRegistration {
    name: Option<String>,
    email: Option<String>,
    bio: Option<String>,
    passphrase: Option<String>,
    device_name: Option<String>,
    app_name: Option<String>,
}

impl UserRegistration {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_name(mut self, value: impl Into<String>) -> Self {
        self.name = Some(value.into());
        self
    }
    pub fn with_email(mut self, value: impl Into<String>) -> Self {
        self.email = Some(value.into());
        self
    }
    pub fn with_bio(mut self, value: impl Into<String>) -> Self {
        self.bio = Some(value.into());
        self
    }
    pub fn with_passphrase(mut self, value: impl Into<String>) -> Self {
        self.passphrase = Some(value.into());
        self
    }

    pub fn with_initial_device(mut self, name: impl Into<String>, app: impl Into<String>) -> Self {
        self.device_name = Some(name.into());
        self.app_name = Some(app.into());
        self
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }

    pub fn bio(&self) -> Option<&str> {
        self.bio.as_deref()
    }

    pub fn passphrase(&self) -> Option<&str> {
        self.passphrase.as_deref()
    }

    pub fn device_name(&self) -> Option<&str> {
        self.device_name.as_deref()
    }

    pub fn app_name(&self) -> Option<&str> {
        self.app_name.as_deref()
    }

    pub fn has_initial_device(&self) -> bool {
        self.device_name.is_some()
    }
}
