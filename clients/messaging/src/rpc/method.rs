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

use serde::{Serialize, Deserialize};

use boson_api::{
    errors::ArgumentError,
    Error,
    Result,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub(crate) enum Method {
    #[serde(rename = "sl")]
    SessionList = 1,
    #[serde(rename = "sr")]
    SessionRevoke,
    #[serde(rename = "cm")]
    ContactMutate,
    #[serde(rename = "cs")]
    ContactSync,
    #[serde(rename = "cc")]
    ChannelCreate,
    #[serde(rename = "cd")]
    ChannelDelete,
    #[serde(rename = "cot")]
    ChannelOwnershipTransfer,
    #[serde(rename = "csr")]
    ChannelSessionKeyRotate,
    #[serde(rename = "ciu")]
    ChannelInfoUpdate,
    #[serde(rename = "cru")]
    ChannelMembersRoleUpdate,
    #[serde(rename = "cmb")]
    ChannelMembersBan,
    #[serde(rename = "cmu")]
    ChannelMembersUnban,
    #[serde(rename = "cmr")]
    ChannelMembersRemove,
    #[serde(rename = "cj")]
    ChannelJoin,
    #[serde(rename = "cl")]
    ChannelLeave,
    #[serde(rename = "ci")]
    ChannelInfo,
}

impl TryFrom<i32> for Method {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            1 => Ok(Method::SessionList),
            2 => Ok(Method::SessionRevoke),
            3 => Ok(Method::ContactMutate),
            4 => Ok(Method::ContactSync),
            5 => Ok(Method::ChannelCreate),
            6 => Ok(Method::ChannelDelete),
            7 => Ok(Method::ChannelOwnershipTransfer),
            8 => Ok(Method::ChannelSessionKeyRotate),
            9 => Ok(Method::ChannelInfoUpdate),
            10 => Ok(Method::ChannelMembersRoleUpdate),
            11 => Ok(Method::ChannelMembersBan),
            12 => Ok(Method::ChannelMembersUnban),
            13 => Ok(Method::ChannelMembersRemove),
            14 => Ok(Method::ChannelJoin),
            15 => Ok(Method::ChannelLeave),
            16 => Ok(Method::ChannelInfo),
            _ => Err(ArgumentError::new(format!("Invalid RPC method: {value}")).into()),
        }
    }
}
