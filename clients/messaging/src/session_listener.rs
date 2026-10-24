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

use crate::SessionInfo;

/// A listener interface for monitoring the creation of new sessions.
/// Classes that are interested in being notified about new sessions
/// should implement this interface and register with a session management
/// mechanism.
pub trait SessionListener: Send + Sync {

    /// Called when a new session is created.
    /// This method notifies the implementing class of newly established sessions,
    /// providing session details for further processing or tracking.
    ///
    /// # Arguments
    ///
    /// * `session_info` - An object representing the information related to the new session,
    ///                    including its unique identifier, online status, and timestamp of the last activity.
    fn on_new_session(&self, session_info: &SessionInfo);
}
