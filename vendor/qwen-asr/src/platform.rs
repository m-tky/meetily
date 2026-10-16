//! Windows-only file mapping and positional I/O. Model files must remain
//! immutable while mapped, just as with the upstream Unix mmap implementation.
use std::{fs::File, io, os::windows::fs::FileExt, path::Path};

pub(crate) struct ReadOnlyMapping {
    file: File,
    map: memmap2::Mmap,
}
impl ReadOnlyMapping {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        // SAFETY: read-only mapping, retained until all borrowed weight pointers
        // are dropped. The worker requires immutable model/sidecar files.
        let map = unsafe { memmap2::MmapOptions::new().map(&file)? };
        Ok(Self { file, map })
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.map
    }
    pub(crate) fn read_exact_at(&self, mut dst: &mut [u8], mut offset: u64) -> io::Result<()> {
        while !dst.is_empty() {
            match self.file.seek_read(dst, offset) {
                Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(n) => {
                    offset += n as u64;
                    dst = &mut dst[n..];
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

pub(crate) trait FileWriteAt {
    fn write_all_at(&self, bytes: &[u8], offset: u64) -> io::Result<()>;
}
impl FileWriteAt for File {
    fn write_all_at(&self, mut bytes: &[u8], mut offset: u64) -> io::Result<()> {
        while !bytes.is_empty() {
            match self.seek_write(bytes, offset) {
                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                Ok(n) => {
                    offset += n as u64;
                    bytes = &bytes[n..];
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}
