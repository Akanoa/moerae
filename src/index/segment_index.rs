use std::fs;
use std::path::{Path, PathBuf};

use usearch::ffi::{IndexOptions, MetricKind, ScalarKind};
use usearch::Index;

pub struct SegmentIndex {
    index: Index,
    path: PathBuf,
    writable: bool,
}

impl SegmentIndex {
    pub fn create(path: &Path, capacity: usize, dimensions: usize) -> Result<Self, String> {
        let options = IndexOptions {
            dimensions,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            ..Default::default()
        };

        let index = Index::new(&options).map_err(|e| e.to_string())?;
        index.reserve(capacity).map_err(|e| e.to_string())?;

        Ok(Self {
            index,
            path: path.to_path_buf(),
            writable: true,
        })
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let options = IndexOptions {
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            ..Default::default()
        };

        let index = Index::new(&options).map_err(|e| e.to_string())?;
        index
            .load(path.to_str().ok_or("invalid path")?)
            .map_err(|e| e.to_string())?;

        Ok(Self {
            index,
            path: path.to_path_buf(),
            writable: true,
        })
    }

    pub fn view(path: &Path) -> Result<Self, String> {
        let options = IndexOptions {
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            ..Default::default()
        };

        let index = Index::new(&options).map_err(|e| e.to_string())?;
        index
            .view(path.to_str().ok_or("invalid path")?)
            .map_err(|e| e.to_string())?;

        Ok(Self {
            index,
            path: path.to_path_buf(),
            writable: false,
        })
    }

    pub fn add(&self, key: u64, vector: &[f32]) -> Result<(), String> {
        if !self.writable {
            return Err("index is read-only (view mode)".into());
        }
        self.index.add(key, vector).map_err(|e| e.to_string())
    }

    pub fn search(&self, query: &[f32], limit: usize) -> Vec<(u64, f32)> {
        let results = self.index.search(query, limit);
        match results {
            Ok(results) => results
                .keys
                .into_iter()
                .zip(results.distances.into_iter())
                .collect(),
            Err(_) => vec![],
        }
    }

    pub fn save(&self) -> Result<(), String> {
        self.index
            .save(self.path.to_str().ok_or("invalid path")?)
            .map_err(|e| e.to_string())
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        self.index
            .save(path.to_str().ok_or("invalid path")?)
            .map_err(|e| e.to_string())
    }

    pub fn len(&self) -> usize {
        self.index.size()
    }

    pub fn is_empty(&self) -> bool {
        self.index.size() == 0
    }

    pub fn remove_file(path: &Path) -> Result<(), std::io::Error> {
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_vector(dims: usize, seed: u64) -> Vec<f32> {
        // Simple deterministic pseudo-random for testing
        let mut v = Vec::with_capacity(dims);
        let mut state = seed;
        for _ in 0..dims {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            v.push(((state >> 33) as f32) / (u32::MAX as f32) - 0.5);
        }
        v
    }

    #[test]
    fn create_add_search() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.usearch");

        let index = SegmentIndex::create(&path, 100, 8).unwrap();

        let v1 = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let v3 = vec![0.9, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        index.add(1, &v1).unwrap();
        index.add(2, &v2).unwrap();
        index.add(3, &v3).unwrap();

        assert_eq!(index.len(), 3);

        // Search for v1 — should find key 1 first (distance ~0), then key 3
        let results = index.search(&v1, 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, 1); // closest match
        assert!(results[0].1 < 0.01); // nearly identical
    }

    #[test]
    fn save_and_reload() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.usearch");

        let index = SegmentIndex::create(&path, 100, 8).unwrap();
        let v1 = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        index.add(1, &v1).unwrap();
        index.save().unwrap();

        drop(index);

        // Reload writable
        let reloaded = SegmentIndex::load(&path).unwrap();
        let results = reloaded.search(&v1, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 1);
    }

    #[test]
    fn view_is_readonly() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.usearch");

        let index = SegmentIndex::create(&path, 100, 8).unwrap();
        let v1 = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        index.add(1, &v1).unwrap();
        index.save().unwrap();

        drop(index);

        let viewed = SegmentIndex::view(&path).unwrap();
        let results = viewed.search(&v1, 1);
        assert_eq!(results[0].0, 1);

        // Write should fail
        let v2 = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert!(viewed.add(2, &v2).is_err());
    }

    #[test]
    fn cosine_distance_range() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.usearch");

        let index = SegmentIndex::create(&path, 100, 4).unwrap();
        let v1 = vec![1.0, 0.0, 0.0, 0.0];
        let v_opposite = vec![-1.0, 0.0, 0.0, 0.0];

        index.add(1, &v1).unwrap();
        index.add(2, &v_opposite).unwrap();

        let results = index.search(&v1, 2);
        // Identical: distance ~0, opposite: distance ~2
        assert!(results[0].1 < 0.01);
        assert!(results[1].1 > 1.5);
    }

    #[test]
    fn many_vectors() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.usearch");

        let dims = 32;
        let index = SegmentIndex::create(&path, 200, dims).unwrap();

        for i in 0..100 {
            let v = random_vector(dims, i);
            index.add(i, &v).unwrap();
        }

        assert_eq!(index.len(), 100);

        // Search should return results
        let query = random_vector(dims, 42);
        let results = index.search(&query, 10);
        assert_eq!(results.len(), 10);
        // Key 42 should be the closest match
        assert_eq!(results[0].0, 42);
    }

    #[test]
    fn temp_then_rename() {
        let tmp = tempfile::tempdir().unwrap();
        let final_path = tmp.path().join("1.usearch");
        let tmp_path = tmp.path().join("1.usearch.tmp");

        let index = SegmentIndex::create(&tmp_path, 100, 4).unwrap();
        index.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
        index.save().unwrap();

        drop(index);
        fs::rename(&tmp_path, &final_path).unwrap();

        assert!(!tmp_path.exists());
        assert!(final_path.exists());

        let reloaded = SegmentIndex::view(&final_path).unwrap();
        assert_eq!(reloaded.len(), 1);
    }

    #[test]
    fn remove_file_works() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.usearch");

        let index = SegmentIndex::create(&path, 10, 4).unwrap();
        index.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
        index.save().unwrap();
        drop(index);

        assert!(path.exists());
        SegmentIndex::remove_file(&path).unwrap();
        assert!(!path.exists());

        // Removing nonexistent is fine
        SegmentIndex::remove_file(&path).unwrap();
    }
}
