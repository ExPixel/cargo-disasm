use anyhow::Context as _;
use memmap::{Mmap, MmapOptions};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

pub struct SourceLoader {
    // FIXME implement this
    // /// A map of paths that to not exist to their corresponding
    // /// existing paths. (e.g. an absolute Windows path to a path on a Linux OS).
    // path_mapper: HashMap<PathBuf, PathBuf>,
    cache: HashMap<PathBuf, Option<LineCache>>,
}

impl SourceLoader {
    pub fn new() -> SourceLoader {
        SourceLoader {
            // path_mapper: HashMap::new(),
            cache: HashMap::new(),
        }
    }

    pub fn load_lines<'p, I>(&mut self, lines: I, output: &mut Vec<Box<str>>) -> anyhow::Result<()>
    where
        I: Iterator<Item = (&'p Path, u32)>,
    {
        use std::collections::hash_map::Entry;
        for (path, line) in lines {
            let cache = match self.cache.entry(path.into()) {
                Entry::Occupied(o) => o.into_mut(),
                Entry::Vacant(v) => {
                    if !path.exists() {
                        v.insert(None)
                    } else {
                        v.insert(Some(
                            LineCache::new(path).context("error loading line cache")?,
                        ))
                    }
                }
            };

            if let Some(line_str) = cache.as_mut().and_then(|cache| cache.line(line)) {
                output.push(line_str.into());
            }
        }
        Ok(())
    }
}

struct LineCache {
    /// This is the ending offset of each line.
    offsets: Vec<u32>,
    /// The file mapped to memory.
    mapping: Mmap,
    /// The index that we are currently at.
    current: usize,
}

impl LineCache {
    pub fn new(path: &Path) -> anyhow::Result<LineCache> {
        unsafe {
            MmapOptions::new()
                .map(
                    &File::open(path).with_context(|| {
                        format!("failed to open source file `{}`", path.display())
                    })?,
                )
                .map(|mapping| LineCache {
                    offsets: Vec::new(),
                    mapping,
                    current: 0,
                })
                .map_err(|err| err.into())
        }
    }

    pub fn line(&mut self, mut index: u32) -> Option<Cow<'_, str>> {
        if index == 0 {
            return None;
        }
        index -= 1;

        while self.offsets.len() as u32 <= index && self.current < self.mapping.len() {
            self.next_line();
        }

        let mut end = *self.offsets.get(index as usize)? as usize;
        let start = if index == 0 {
            0
        } else {
            self.offsets[index as usize - 1] as usize
        };

        // This is true for all but the last line.
        if self.mapping[end - 1] == b'\n' {
            end -= 1;
        }

        Some(String::from_utf8_lossy(&self.mapping[start..end]))
    }

    pub fn next_line(&mut self) {
        while self.current < self.mapping.len() {
            if self.mapping[self.current] == b'\n' {
                self.current += 1;
                break;
            }
            self.current += 1;
        }
        self.offsets.push(self.current as u32);
    }
}
