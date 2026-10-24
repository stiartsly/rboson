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

use boson::did::{ResolutionMetadata, ResolutionOptions, ResolutionResult, ResolutionStatus};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_options_default() {
        let options = ResolutionOptions::default();
        assert!(options.use_cache);
        assert_eq!(options.valid_ttl, 0);
    }

    #[test]
    fn resolution_result_success() {
        let metadata = ResolutionMetadata {
            created: None,
            updated: None,
            resolved: std::time::SystemTime::now(),
            deactivated: false,
            version: 1,
        };
        let result = ResolutionResult::success("value", Some(metadata));
        assert_eq!(result.status, ResolutionStatus::Success);
        assert_eq!(result.result, Some("value"));
        assert!(result.succeeded());
        assert!(!result.failed());
        assert!(result.metadata.is_some());
    }

    #[test]
    fn resolution_result_not_found() {
        let result = ResolutionResult::<()>::not_found();
        assert_eq!(result.status, ResolutionStatus::NotFound);
        assert!(result.result.is_none());
        assert!(result.failed());
    }

    #[test]
    fn resolution_result_invalid() {
        let result = ResolutionResult::<()>::invalid();
        assert_eq!(result.status, ResolutionStatus::Invalid);
        assert!(result.result.is_none());
        assert!(result.failed());
    }
}
