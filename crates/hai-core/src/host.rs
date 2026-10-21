//! Host system queries and Home Assistant reachability checks.
//!
//! Used by the VM flows to size a new VM and to poll a freshly started
//! Home Assistant instance.

use crate::error::Result;
use crate::types::SystemInfo;
use crate::{Backend, HostBackend};
use std::sync::LazyLock;
use std::time::Duration;

/// Query the host for CPU core count and total memory.
fn system_info() -> Result<SystemInfo> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let cpu_cores = Command::new("sysctl")
            .args(["-n", "hw.ncpu"])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|s| s.trim().parse::<usize>().ok())
            .unwrap_or(4);

        let memory_bytes = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(8 * 1024 * 1024 * 1024);

        Ok(SystemInfo {
            cpu_cores,
            memory_mb: memory_bytes / (1024 * 1024),
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(crate::error::Error::UnsupportedPlatform(
            "Host system info is only implemented on macOS".to_string(),
        ))
    }
}

/// Whether the Home Assistant webserver at `ip` answers HTTP on port 80.
async fn check_ha_ready(ip: &str) -> bool {
    // Any non-5xx reply means the webserver is up, even if `/` itself is not served.
    get_status(ip, "/")
        .await
        .is_some_and(|status| !status.is_server_error())
}

/// Whether Home Assistant at `ip` has finished starting up (serves its manifest).
async fn check_ha_updated(ip: &str) -> bool {
    get_status(ip, "/manifest.json")
        .await
        .is_some_and(|status| status.is_success())
}

/// Shared by every check: the frontend polls these every 2 seconds, for up to
/// an hour, and a client per check would redo its setup and drop its
/// connections each time.
static HA_CLIENT: LazyLock<Option<reqwest::Client>> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()
});

async fn get_status(ip: &str, path: &str) -> Option<reqwest::StatusCode> {
    // Never probe an empty host: in `http:///manifest.json` the path would be read as the host.
    if ip.trim().is_empty() {
        return None;
    }
    let client = HA_CLIENT.as_ref()?;
    let response = client.get(format!("http://{ip}{path}")).send().await.ok()?;
    Some(response.status())
}

impl HostBackend for Backend {
    fn system_info(&self) -> Result<SystemInfo> {
        system_info()
    }

    async fn check_ha_ready(&self, ip: &str) -> bool {
        check_ha_ready(ip).await
    }

    async fn check_ha_updated(&self, ip: &str) -> bool {
        check_ha_updated(ip).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn test_check_ha_ready_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .with_status(200)
            .create_async()
            .await;

        assert!(check_ha_ready(&server.host_with_port()).await);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_check_ha_ready_404_is_ready() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .with_status(404)
            .create_async()
            .await;

        assert!(check_ha_ready(&server.host_with_port()).await);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_check_ha_ready_5xx_is_not_ready() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .with_status(502)
            .create_async()
            .await;

        assert!(!check_ha_ready(&server.host_with_port()).await);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_check_ha_updated_success() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/manifest.json")
            .with_status(200)
            .with_body(r#"{"version": "2023.12.0"}"#)
            .create_async()
            .await;

        assert!(check_ha_updated(&server.host_with_port()).await);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_check_ha_updated_404_is_not_updated() {
        let mut server = Server::new_async().await;
        let mock = server
            .mock("GET", "/manifest.json")
            .with_status(404)
            .create_async()
            .await;

        assert!(!check_ha_updated(&server.host_with_port()).await);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_get_status_never_probes_an_empty_host() {
        let mut server = Server::new_async().await;
        let mock = server.mock("GET", "/").expect(0).create_async().await;

        // Without a host, this path would be taken for the server's address
        let path = format!("/{}", server.host_with_port());
        assert_eq!(get_status("", &path).await, None);
        mock.assert_async().await;
    }
}
