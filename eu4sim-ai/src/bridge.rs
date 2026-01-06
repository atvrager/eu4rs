//! Python inference bridge client.
//!
//! Connects to the Python inference server for ROCm-accelerated inference.
//! Can auto-spawn the server process if not already running.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Default server address.
pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 9876;

/// Request to the inference server.
#[derive(Debug, Serialize)]
struct InferenceRequest<'a> {
    prompt: &'a str,
    max_tokens: usize,
}

/// Response from the inference server.
#[derive(Debug, Deserialize)]
struct InferenceResponse {
    response: Option<String>,
    inference_ms: Option<u64>,
    error: Option<String>,
}

/// Client for the Python inference server.
///
/// Maintains a persistent TCP connection for low-latency inference.
/// The connection is lazily established on first request.
pub struct BridgeClient {
    host: String,
    port: u16,
    stream: Option<TcpStream>,
    /// Connection timeout in milliseconds
    connect_timeout_ms: u64,
    /// Read timeout in milliseconds (inference can take a while)
    read_timeout_ms: u64,
}

impl BridgeClient {
    /// Create a new bridge client with default settings.
    pub fn new() -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
            stream: None,
            connect_timeout_ms: 5000,
            read_timeout_ms: 30000, // 30s for inference
        }
    }

    /// Create a client connecting to a specific address.
    pub fn with_address(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            stream: None,
            connect_timeout_ms: 5000,
            read_timeout_ms: 30000,
        }
    }

    /// Set the read timeout (for long inference).
    pub fn with_read_timeout(mut self, timeout_ms: u64) -> Self {
        self.read_timeout_ms = timeout_ms;
        self
    }

    /// Check if connected to the server.
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    /// Try to connect to the inference server.
    ///
    /// Returns Ok(true) if connected, Ok(false) if server not available.
    pub fn try_connect(&mut self) -> Result<bool> {
        if self.stream.is_some() {
            return Ok(true);
        }

        let addr = format!("{}:{}", self.host, self.port);
        log::info!("Connecting to inference server at {}", addr);

        match TcpStream::connect_timeout(
            &addr.parse().context("Invalid address")?,
            Duration::from_millis(self.connect_timeout_ms),
        ) {
            Ok(stream) => {
                stream
                    .set_read_timeout(Some(Duration::from_millis(self.read_timeout_ms)))
                    .context("Failed to set read timeout")?;
                stream
                    .set_write_timeout(Some(Duration::from_millis(5000)))
                    .context("Failed to set write timeout")?;
                stream.set_nodelay(true).ok(); // Disable Nagle for lower latency

                log::info!("Connected to inference server");
                self.stream = Some(stream);
                Ok(true)
            }
            Err(e) => {
                log::warn!("Could not connect to inference server: {}", e);
                Ok(false)
            }
        }
    }

    /// Disconnect from the server.
    pub fn disconnect(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
            log::info!("Disconnected from inference server");
        }
    }

    /// Run inference on the given prompt.
    ///
    /// Returns the generated text and inference time in milliseconds.
    pub fn generate(&mut self, prompt: &str, max_tokens: usize) -> Result<(String, u64)> {
        // Ensure connected
        if !self.try_connect()? {
            anyhow::bail!(
                "Inference server not available at {}:{}",
                self.host,
                self.port
            );
        }

        let stream = self.stream.as_mut().unwrap();

        // Send request
        let request = InferenceRequest { prompt, max_tokens };
        let request_json =
            serde_json::to_string(&request).context("Failed to serialize request")?;

        stream
            .write_all(request_json.as_bytes())
            .context("Failed to send request")?;
        stream.write_all(b"\n").context("Failed to send newline")?;
        stream.flush().context("Failed to flush")?;

        // Read response (newline-delimited JSON)
        let mut reader = BufReader::new(stream.try_clone().context("Failed to clone stream")?);
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .context("Failed to read response")?;

        // Parse response
        let response: InferenceResponse =
            serde_json::from_str(&response_line).context("Failed to parse response")?;

        if let Some(error) = response.error {
            anyhow::bail!("Inference server error: {}", error);
        }

        let text = response
            .response
            .ok_or_else(|| anyhow::anyhow!("Missing response field"))?;
        let inference_ms = response.inference_ms.unwrap_or(0);

        Ok((text, inference_ms))
    }
}

impl Default for BridgeClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BridgeClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}

/// Managed inference server that auto-spawns Python subprocess.
///
/// Spawns the inference server on creation and kills it on drop.
/// Use this when you want automatic server lifecycle management.
pub struct BridgeServer {
    process: Option<Child>,
    client: BridgeClient,
}

impl BridgeServer {
    /// Spawn the inference server with the given adapter.
    ///
    /// Looks for the Python venv at `scripts/.venv-rocm` and the server script
    /// at `scripts/inference_server.py` relative to the workspace root.
    pub fn spawn(adapter_path: Option<PathBuf>) -> Result<Self> {
        Self::spawn_with_options(adapter_path, DEFAULT_HOST, DEFAULT_PORT)
    }

    /// Spawn with custom host/port.
    pub fn spawn_with_options(
        adapter_path: Option<PathBuf>,
        host: &str,
        port: u16,
    ) -> Result<Self> {
        // Find workspace root (look for Cargo.toml)
        let workspace_root = Self::find_workspace_root()?;
        let scripts_dir = workspace_root.join("scripts");

        // Find Python interpreter
        let python = Self::find_python(&scripts_dir)?;
        let server_script = scripts_dir.join("inference_server.py");

        if !server_script.exists() {
            anyhow::bail!(
                "Inference server script not found at {}",
                server_script.display()
            );
        }

        // Build command
        let mut cmd = Command::new(&python);
        cmd.arg(&server_script)
            .arg("--host")
            .arg(host)
            .arg("--port")
            .arg(port.to_string())
            .current_dir(&scripts_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(ref adapter) = adapter_path {
            cmd.arg("--adapter").arg(adapter);
        }

        log::info!(
            "Spawning inference server: {} {}",
            python.display(),
            server_script.display()
        );

        let process = cmd.spawn().context("Failed to spawn inference server")?;

        // Wait for server to be ready (poll connection)
        let mut client = BridgeClient::with_address(host, port);
        let start = Instant::now();
        let timeout = Duration::from_secs(120); // Model loading can take a while

        loop {
            if start.elapsed() > timeout {
                anyhow::bail!(
                    "Inference server failed to start within {}s",
                    timeout.as_secs()
                );
            }

            if client.try_connect()? {
                log::info!("Inference server ready after {:?}", start.elapsed());
                break;
            }

            std::thread::sleep(Duration::from_millis(500));
        }

        Ok(Self {
            process: Some(process),
            client,
        })
    }

    /// Find workspace root by looking for Cargo.toml.
    fn find_workspace_root() -> Result<PathBuf> {
        let mut dir = std::env::current_dir().context("Failed to get current directory")?;

        loop {
            if dir.join("Cargo.toml").exists() && dir.join("scripts").exists() {
                return Ok(dir);
            }

            if !dir.pop() {
                anyhow::bail!("Could not find workspace root (no Cargo.toml with scripts/ found)");
            }
        }
    }

    /// Find Python interpreter (prefer ROCm venv).
    fn find_python(scripts_dir: &Path) -> Result<PathBuf> {
        // Try ROCm venv first
        let rocm_python = if cfg!(windows) {
            scripts_dir.join(".venv-rocm/Scripts/python.exe")
        } else {
            scripts_dir.join(".venv-rocm/bin/python")
        };

        if rocm_python.exists() {
            return Ok(rocm_python);
        }

        // Try regular venv
        let venv_python = if cfg!(windows) {
            scripts_dir.join(".venv/Scripts/python.exe")
        } else {
            scripts_dir.join(".venv/bin/python")
        };

        if venv_python.exists() {
            return Ok(venv_python);
        }

        // Fall back to system Python
        let system_python = if cfg!(windows) { "python" } else { "python3" };
        Ok(PathBuf::from(system_python))
    }

    /// Run inference (delegates to client).
    pub fn generate(&mut self, prompt: &str, max_tokens: usize) -> Result<(String, u64)> {
        self.client.generate(prompt, max_tokens)
    }

    /// Check if the server process is still running.
    pub fn is_running(&mut self) -> bool {
        self.process
            .as_mut()
            .map(|p| p.try_wait().ok().flatten().is_none())
            .unwrap_or(false)
    }
}

impl Drop for BridgeServer {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            log::info!("Shutting down inference server...");
            // Try graceful shutdown first
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = BridgeClient::new();
        assert_eq!(client.host, DEFAULT_HOST);
        assert_eq!(client.port, DEFAULT_PORT);
        assert!(!client.is_connected());
    }

    #[test]
    fn test_custom_address() {
        let client = BridgeClient::with_address("192.168.1.100", 8080);
        assert_eq!(client.host, "192.168.1.100");
        assert_eq!(client.port, 8080);
    }

    #[test]
    fn test_connection_failure_graceful() {
        let mut client = BridgeClient::with_address("127.0.0.1", 59999);
        // Should return Ok(false), not an error
        let result = client.try_connect();
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
