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

pub const HEADER_NAME: &str = "Content-Type";

pub const TEXT: &str = "text/plain";
pub const BINARY: &str = "application/octet-stream";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    TextPlain,
    ApplicationJson,
    ApplicationCbor,
    ApplicationOctetStream,
    ImageJpeg,
    ImagePng,
    ImageWebp,
    AudioAac,
    AudioMp3,
    AudioWebm,
    VideoMp4,
    VideoWebm,
}

impl ContentType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TextPlain => "text/plain",
            Self::ApplicationJson => "application/json",
            Self::ApplicationCbor => "application/cbor",
            Self::ApplicationOctetStream => "application/octet-stream",
            Self::ImageJpeg => "image/jpeg",
            Self::ImagePng => "image/png",
            Self::ImageWebp => "image/webp",
            Self::AudioAac => "audio/aac",
            Self::AudioMp3 => "audio/mpeg",
            Self::AudioWebm => "audio/webm",
            Self::VideoMp4 => "video/mp4",
            Self::VideoWebm => "video/webm",
        }
    }
}
