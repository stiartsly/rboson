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

use std::time::SystemTime;

use boson_api::{Id, errors::Result};
use crate::contact::Contact;

/// A conversation between the local user and another party (person or channel).
///
pub trait ConversationOps: Send + Sync {
    /// The unique identifier of this conversation (same as the participant's `Id`).
    fn id(&self) -> &Id;

    /// The contact representing the other participant.
    fn contact(&self) -> &Contact;

    /// Whether this is a channel (group) conversation.
    fn is_channel(&self) -> bool;

    /// Short text preview of the most recent activity.
    fn preview(&self) -> Result<Option<&str>>;

    /// When this conversation was last updated.
    fn updated_at(&self) -> Option<SystemTime>;
}
