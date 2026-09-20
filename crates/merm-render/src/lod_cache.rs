use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CachedTexture {
    pub level: u32,
    pub width: u32,
    pub height: u32,
    pub data_bytes: usize,
    pub last_accessed_seq: u64,
}

pub struct LodCache {
    max_memory_bytes: usize,
    current_memory_bytes: usize,
    access_seq: u64,
    textures: HashMap<u32, CachedTexture>,
}

impl LodCache {
    pub fn new(max_memory_mb: usize) -> Self {
        Self {
            max_memory_bytes: max_memory_mb * 1024 * 1024,
            current_memory_bytes: 0,
            access_seq: 0,
            textures: HashMap::new(),
        }
    }

    pub fn get(&mut self, level: u32) -> Option<&CachedTexture> {
        self.access_seq += 1;
        let seq = self.access_seq;
        if let Some(tex) = self.textures.get_mut(&level) {
            tex.last_accessed_seq = seq;
            Some(tex)
        } else {
            None
        }
    }

    pub fn insert(&mut self, level: u32, width: u32, height: u32) {
        let data_bytes = (width * height * 4) as usize; // RGBA8
        self.access_seq += 1;

        // Evict LRU textures if needed
        while self.current_memory_bytes + data_bytes > self.max_memory_bytes
            && !self.textures.is_empty()
        {
            self.evict_lru();
        }

        self.current_memory_bytes += data_bytes;
        self.textures.insert(
            level,
            CachedTexture {
                level,
                width,
                height,
                data_bytes,
                last_accessed_seq: self.access_seq,
            },
        );
    }

    fn evict_lru(&mut self) {
        if let Some((&oldest_level, _)) = self
            .textures
            .iter()
            .min_by_key(|(_, tex)| tex.last_accessed_seq)
        {
            if let Some(evicted) = self.textures.remove(&oldest_level) {
                self.current_memory_bytes = self
                    .current_memory_bytes
                    .saturating_sub(evicted.data_bytes);
                log::debug!(
                    "Evicted LOD level {} from cache to preserve memory envelope",
                    oldest_level
                );
            }
        }
    }

    pub fn current_memory_usage(&self) -> usize {
        self.current_memory_bytes
    }

    pub fn len(&self) -> usize {
        self.textures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }
}
