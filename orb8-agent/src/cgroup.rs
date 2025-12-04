//! cgroup ID resolver for mapping containers to pods
//!
//! Kubernetes assigns each pod container to a cgroup, and the cgroup's inode
//! number is used by eBPF probes to identify the container. This module
//! resolves pod UID + container ID to cgroup inode number.
//!
//! Supported container runtimes:
//! - containerd: cri-containerd-{id}.scope

use anyhow::{anyhow, Context, Result};
use log::{debug, warn};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// Quality of Service classes in Kubernetes
const QOS_CLASSES: [&str; 3] = ["", "burstable-", "besteffort-"];

/// Cgroup v2 root path
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// CgroupResolver handles mapping pod containers to cgroup IDs
pub struct CgroupResolver {
    cgroup_root: PathBuf,
}

impl CgroupResolver {
    /// Create a new CgroupResolver with default cgroup root
    pub fn new() -> Self {
        Self {
            cgroup_root: PathBuf::from(CGROUP_ROOT),
        }
    }

    /// Create a new CgroupResolver with custom cgroup root (for testing)
    #[allow(dead_code)]
    pub fn with_root(cgroup_root: PathBuf) -> Self {
        Self { cgroup_root }
    }

    /// Resolve a container to its cgroup ID (inode number)
    ///
    /// Arguments:
    /// - pod_uid: The pod's UID (e.g., "abc-123-def-456")
    /// - container_id: The container's ID from the runtime
    ///
    /// Returns the cgroup inode number if found
    pub fn resolve(&self, pod_uid: &str, container_id: &str) -> Result<u64> {
        // Normalize pod UID: replace dashes with underscores for cgroup path
        let normalized_uid = pod_uid.replace('-', "_");

        // Clean container ID (remove prefix like "containerd://")
        let clean_container_id = container_id.split("://").last().unwrap_or(container_id);

        // Try each QoS class path pattern
        for qos in QOS_CLASSES {
            // Try containerd path pattern
            if let Some(inode) = self.try_containerd_path(&normalized_uid, clean_container_id, qos)
            {
                return Ok(inode);
            }
        }

        Err(anyhow!(
            "Could not resolve cgroup for pod {} container {}",
            pod_uid,
            container_id
        ))
    }

    /// Try containerd cgroup path pattern
    fn try_containerd_path(&self, pod_uid: &str, container_id: &str, qos: &str) -> Option<u64> {
        // containerd pattern:
        // /sys/fs/cgroup/kubepods.slice/kubepods-{qos}pod{uid}.slice/cri-containerd-{container_id}.scope
        let pod_slice = if qos.is_empty() {
            format!("kubepods-pod{}.slice", pod_uid)
        } else {
            format!(
                "kubepods-{}.slice/kubepods-{}pod{}.slice",
                qos.trim_end_matches('-'),
                qos,
                pod_uid
            )
        };

        let container_scope = format!("cri-containerd-{}.scope", container_id);

        let path = self
            .cgroup_root
            .join("kubepods.slice")
            .join(&pod_slice)
            .join(&container_scope);

        debug!("Trying cgroup path: {}", path.display());

        self.get_inode(&path)
    }

    /// Get the inode number of a path
    fn get_inode(&self, path: &Path) -> Option<u64> {
        match fs::metadata(path) {
            Ok(metadata) => {
                let inode = metadata.ino();
                debug!("Found cgroup at {} with inode {}", path.display(), inode);
                Some(inode)
            }
            Err(e) => {
                debug!("Path {} not found: {}", path.display(), e);
                None
            }
        }
    }

    /// Scan the cgroup filesystem to find all container cgroups
    /// and build a reverse map of inode -> (pod_uid, container_id)
    ///
    /// This is useful for resolving cgroup IDs that we didn't see at pod creation time
    pub fn scan_all(&self) -> Result<Vec<(u64, String, String)>> {
        let mut results = Vec::new();
        let kubepods_path = self.cgroup_root.join("kubepods.slice");

        if !kubepods_path.exists() {
            warn!("kubepods.slice not found at {}", kubepods_path.display());
            return Ok(results);
        }

        // Walk the cgroup tree looking for container scopes
        self.scan_directory(&kubepods_path, &mut results)?;

        Ok(results)
    }

    /// Recursively scan a directory for container cgroup scopes
    fn scan_directory(&self, dir: &Path, results: &mut Vec<(u64, String, String)>) -> Result<()> {
        let entries = fs::read_dir(dir).context(format!("Failed to read directory: {:?}", dir))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Recurse into subdirectories
                self.scan_directory(&path, results)?;
            } else {
                continue;
            }

            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Look for containerd container scopes
            if name.starts_with("cri-containerd-") && name.ends_with(".scope") {
                if let Some(inode) = self.get_inode(&path) {
                    // Extract container ID from scope name
                    let container_id = name
                        .strip_prefix("cri-containerd-")
                        .and_then(|s| s.strip_suffix(".scope"))
                        .unwrap_or("")
                        .to_string();

                    // Try to extract pod UID from parent path
                    if let Some(pod_uid) = extract_pod_uid_from_path(&path) {
                        results.push((inode, pod_uid, container_id));
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for CgroupResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract pod UID from a cgroup path
fn extract_pod_uid_from_path(path: &Path) -> Option<String> {
    // Look for parent directory containing "pod" in the name
    // Pattern: kubepods-{qos}pod{uid}.slice or kubepods-pod{uid}.slice
    for ancestor in path.ancestors() {
        if let Some(name) = ancestor.file_name().and_then(|n| n.to_str()) {
            if name.contains("-pod") && name.ends_with(".slice") {
                // Extract UID from pattern: kubepods-{qos}pod{uid}.slice
                // or kubepods-pod{uid}.slice
                if let Some(start) = name.find("-pod") {
                    let uid_start = start + 4; // skip "-pod"
                    if let Some(uid_part) = name.get(uid_start..) {
                        if let Some(uid) = uid_part.strip_suffix(".slice") {
                            // Convert underscores back to dashes for standard UID format
                            return Some(uid.replace('_', "-"));
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_pod_uid_simple() {
        let path =
            PathBuf::from("/sys/fs/cgroup/kubepods.slice/kubepods-pod12345.slice/container.scope");
        let uid = extract_pod_uid_from_path(&path);
        assert_eq!(uid, Some("12345".to_string()));
    }

    #[test]
    fn test_extract_pod_uid_with_qos() {
        let path = PathBuf::from("/sys/fs/cgroup/kubepods.slice/kubepods-burstable.slice/kubepods-burstable-pod12345_6789.slice/container.scope");
        let uid = extract_pod_uid_from_path(&path);
        assert_eq!(uid, Some("12345-6789".to_string()));
    }
}
