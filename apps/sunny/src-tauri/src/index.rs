//! Shared immutable index snapshots: readers copy an Arc, never every icon.
use std::sync::{Arc, Mutex};

pub struct SnapshotIndex<T> {
    snapshot: Mutex<Option<Arc<Vec<T>>>>,
    init: Mutex<()>,
}

impl<T> Default for SnapshotIndex<T> {
    fn default() -> Self {
        Self {
            snapshot: Mutex::new(None),
            init: Mutex::new(()),
        }
    }
}

impl<T> SnapshotIndex<T> {
    pub fn get_or_init(&self, load: impl FnOnce() -> Vec<T>) -> Result<Arc<Vec<T>>, String> {
        if let Some(snapshot) = self
            .snapshot
            .lock()
            .map_err(|_| "Index unavailable")?
            .as_ref()
        {
            return Ok(Arc::clone(snapshot));
        }
        let _init = self
            .init
            .lock()
            .map_err(|_| "Index initialization unavailable")?;
        if let Some(snapshot) = self
            .snapshot
            .lock()
            .map_err(|_| "Index unavailable")?
            .as_ref()
        {
            return Ok(Arc::clone(snapshot));
        }
        let snapshot = Arc::new(load());
        *self.snapshot.lock().map_err(|_| "Index unavailable")? = Some(Arc::clone(&snapshot));
        Ok(snapshot)
    }

    pub fn refresh(&self, load: impl FnOnce() -> Vec<T>) -> Result<(), String> {
        let _init = self
            .init
            .lock()
            .map_err(|_| "Index initialization unavailable")?;
        let snapshot = Arc::new(load());
        *self.snapshot.lock().map_err(|_| "Index unavailable")? = Some(snapshot);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn hot_reads_share_the_allocation_without_cloning_entries() {
        // Deliberately not Clone: a cache read must not need deep entry copies.
        struct Entry;
        let index = SnapshotIndex::<Entry>::default();
        let first = index.get_or_init(|| vec![Entry]).unwrap();
        let second = index.get_or_init(|| panic!("hot read rescanned")).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn an_empty_scan_is_still_an_initialized_index() {
        let index = SnapshotIndex::<u8>::default();
        let first = index.get_or_init(Vec::new).unwrap();
        let second = index
            .get_or_init(|| panic!("empty index rescanned"))
            .unwrap();
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn cold_requests_coalesce_and_refresh_keeps_old_snapshots_valid() {
        let index = Arc::new(SnapshotIndex::<usize>::default());
        let scans = Arc::new(AtomicUsize::new(0));
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let index = index.clone();
                let scans = scans.clone();
                std::thread::spawn(move || {
                    index
                        .get_or_init(|| {
                            scans.fetch_add(1, Ordering::Relaxed);
                            vec![1]
                        })
                        .unwrap()
                })
            })
            .collect();
        let snapshots: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(scans.load(Ordering::Relaxed), 1);
        index.refresh(|| vec![2]).unwrap();
        assert_eq!(&**index.get_or_init(Vec::new).unwrap(), &[2]);
        assert!(snapshots.iter().all(|snapshot| snapshot.as_slice() == [1]));
    }
}
