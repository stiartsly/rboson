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

use serde::{
    de::Error as DeError,
    Deserialize,
    Deserializer,
    Serializer
};
use crate::channel::{Permission, Role};

pub(crate) mod role {
    use super::*;

    pub(crate) fn serialize<S>(value: &Role, se: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        se.serialize_i32(i32::from(*value))
    }

    pub(crate) fn deserialize<'de, D>(de: D) -> Result<Role, D::Error>
    where
        D: Deserializer<'de>,
    {
        Role::try_from(i32::deserialize(de)?).map_err(D::Error::custom)
    }
}

pub(crate) mod permission {
    use super::*;

    pub(crate) fn serialize<S>(value: &Permission, se: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        se.serialize_i32(i32::from(*value))
    }

    pub(crate) fn deserialize<'de, D>(de: D) -> Result<Permission, D::Error>
    where
        D: Deserializer<'de>,
    {
        Permission::try_from(i32::deserialize(de)?).map_err(D::Error::custom)
    }
}
