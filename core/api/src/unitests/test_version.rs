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

use crate::version;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_def_version() {
        let ver = version::ver();
        let ver_str = version::format_version(ver);
        assert_eq!(ver_str, "Meerkat/1");
    }

    #[test]
    fn test_mk_version() {
        let ver = version::build("MK", 5).unwrap();
        let ver_str = version::format_version(ver);
        assert_eq!(ver_str, "Meerkat/5");
    }

    #[test]
    fn test_or_version() {
        let ver = version::build("OR", 8).unwrap();
        let ver_str = version::format_version(ver);
        assert_eq!(ver_str, "Orca/8");
    }

    #[test]
    fn test_na_version() {
        let ver_str = version::format_version(0);
        assert_eq!(ver_str, "N/A");
    }

    #[test]
    fn builds_and_formats_versions() {
        let version = version::build("MK", 5).unwrap();
        assert_eq!(version::format_version(version), "Meerkat/5");

        let version = version::build("OR", 8).unwrap();
        assert_eq!(version::format_version(version), "Orca/8");
        assert_eq!(version::format_version(0), version::VERSION_NOT_AVAILABLE);
    }

    #[test]
    fn rejects_short_names() {
        assert!(version::build("M", 1).is_err());
    }

    #[test]
    fn sanitizes_non_printable_names() {
        let version = (b'\n' as i32) << 24 | (b'\x1b' as i32) << 16 | 3;
        assert_eq!(version::format_version(version), "../3");
    }
}
