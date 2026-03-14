//! File watcher for hot-reloading `.0` intent graphs.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tokio::sync::mpsc;
use tracing::{info, warn};
use zerolang::RuntimeGraph;

pub enum IntentEvent {
    Updated(String, RuntimeGraph, String),
    Removed(String),
}

pub struct IntentWatcher {
    directories: Vec<String>,
    event_tx: mpsc::Sender<IntentEvent>,
}

impl IntentWatcher {
    pub fn new(directories: Vec<String>, event_tx: mpsc::Sender<IntentEvent>) -> Self {
        Self { directories, event_tx }
    }

    pub async fn run(self) {
        info!("Started watching directories for .0 intent graphs: {:?}", self.directories);
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        let mut file_mtimes: HashMap<String, SystemTime> = HashMap::new();

        loop {
            interval.tick().await;
            let mut current_files = HashMap::new();

            for dir in &self.directories {
                let mut entries = match tokio::fs::read_dir(dir).await {
                    Ok(entries) => entries,
                    Err(_) => continue, // Dir doesn't exist yet, skip
                };

                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) != Some("0") {
                        continue;
                    }

                    let path_str = path.to_string_lossy().to_string();
                    if let Ok(metadata) = entry.metadata().await {
                        if let Ok(mtime) = metadata.modified() {
                            current_files.insert(path_str.clone(), mtime);

                            let should_reload = match file_mtimes.get(&path_str) {
                                Some(old_mtime) => mtime > *old_mtime,
                                None => true,
                            };

                            if should_reload {
                                if let Ok(content) = tokio::fs::read_to_string(&path).await {
                                    match RuntimeGraph::from_reader(content.as_bytes()) {
                                        Ok(graph) => {
                                            info!("Loaded/Reloaded intent graph: {}", path_str);
                                            let _ = self.event_tx.send(IntentEvent::Updated(path_str.clone(), graph, content)).await;
                                            file_mtimes.insert(path_str, mtime);
                                        }
                                        Err(e) => warn!("Failed to compile {}: {:?}", path_str, e),
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Detect removals
            let removed: Vec<String> = file_mtimes.keys()
                .filter(|k| !current_files.contains_key(*k))
                .cloned()
                .collect();

            for path in removed {
                info!("Removed intent graph: {}", path);
                let _ = self.event_tx.send(IntentEvent::Removed(path.clone())).await;
                file_mtimes.remove(&path);
            }
        }
    }
}
