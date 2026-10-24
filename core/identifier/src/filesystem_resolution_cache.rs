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

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use super::{Card, ResolutionCache, ResolutionResult};
use boson_api::{Id, Result};
pub struct FileSystemResolutionCache {
    dir: PathBuf,
    expiration: Duration,
}
impl FileSystemResolutionCache {
    pub fn new(path: &Path, expiration_secs: u64) -> Result<Self> {
        fs::create_dir_all(path)?;
        if !path.is_dir() {
            return Err("resolution cache path is not a directory".into());
        }
        Ok(Self {
            dir: path.to_path_buf(),
            expiration: Duration::from_secs(if expiration_secs == 0 {
                86400
            } else {
                expiration_secs
            }),
        })
    }
    pub(crate) fn file(&self, id: &Id) -> PathBuf {
        self.dir.join(id.to_string())
    }
}
impl ResolutionCache for FileSystemResolutionCache {
    fn put(&self, id: &Id, result: &ResolutionResult<Card>) -> Result<()> {
        let tmp = self.dir.join(format!("{}.tmp", id));
        fs::write(&tmp, serde_cbor::to_vec(result)?)?;
        fs::rename(tmp, self.file(id))?;
        Ok(())
    }
    fn get(&self, id: &Id) -> Result<Option<ResolutionResult<Card>>> {
        let path = self.file(id);
        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if SystemTime::now()
            .duration_since(meta.modified()?)
            .unwrap_or_default()
            > self.expiration
        {
            let _ = fs::remove_file(path);
            return Ok(None);
        }
        let result: ResolutionResult<Card> = serde_cbor::from_slice(&fs::read(path)?)?;
        if result.succeeded()
            && result
                .result
                .as_ref()
                .map(|c| !c.is_genuine())
                .unwrap_or(true)
        {
            let _ = fs::remove_file(self.file(id));
            return Ok(None);
        }
        Ok(Some(result))
    }
    fn evict_expired(&self) -> Result<()> {
        for e in fs::read_dir(&self.dir)? {
            let p = e?.path();
            if p.is_file()
                && SystemTime::now()
                    .duration_since(fs::metadata(&p)?.modified()?)
                    .unwrap_or_default()
                    > self.expiration
            {
                let _ = fs::remove_file(p);
            }
        }
        Ok(())
    }
    fn clear(&self) -> Result<()> {
        for e in fs::read_dir(&self.dir)? {
            let p = e?.path();
            if p.is_file() {
                fs::remove_file(p)?;
            }
        }
        Ok(())
    }
}
