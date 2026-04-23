//! McpManager -- manages multiple MCP server connections.
//!
//! Provides a high-level interface for discovering and connecting to MCP
//! servers, and aggregating their tools and resources.

use std::collections::HashMap;

use anyhow::Result;
use tracing::{info, warn};

use super::client::McpClient;
use super::{McpResource, McpServerConfig, McpToolDef};

/// Manages multiple MCP server connections.
pub struct McpManager {
    /// Active clients, keyed by server name.
    pub clients: HashMap<String, McpClient>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    /// Connect to all configured MCP servers.
    ///
    /// Discovers servers from settings, connects to each one, and
    /// initializes them. Failures for individual servers are logged
    /// but do not prevent other servers from connecting.
    pub async fn connect_all(&mut self, configs: Vec<McpServerConfig>) -> Result<()> {
        for config in configs {
            let name = config.name.clone();
            // Respect the soft-disable flag from settings. Keeping the entry
            // out of `self.clients` means `list_tools`, `all_tools`, etc. all
            // behave as if the server does not exist for this session, while
            // the on-disk config is preserved for a later re-enable.
            if config.disabled.unwrap_or(false) {
                tracing::info!(server = %name, "MCP: server disabled in settings, skipping");
                super::emit_event(super::McpSubsystemEvent::ServerStateChanged {
                    server_name: name,
                    state: "disabled".to_string(),
                    error: None,
                });
                continue;
            }
            let mut client = McpClient::new(config);

            match client.connect().await {
                Ok(()) => {
                    match client.initialize().await {
                        Ok(()) => {
                            // Discover tools if supported
                            if client.supports_tools() {
                                if let Err(e) = client.list_tools().await {
                                    warn!(
                                        server = %name,
                                        error = %e,
                                        "MCP: failed to list tools"
                                    );
                                }
                            }

                            // Discover resources if supported
                            if client.supports_resources() {
                                if let Err(e) = client.list_resources().await {
                                    warn!(
                                        server = %name,
                                        error = %e,
                                        "MCP: failed to list resources"
                                    );
                                }
                            }

                            info!(
                                server = %name,
                                tools = client.tools.len(),
                                resources = client.resources.len(),
                                "MCP: server ready"
                            );

                            self.clients.insert(name, client);
                        }
                        Err(e) => {
                            warn!(
                                server = %name,
                                error = %e,
                                "MCP: failed to initialize server"
                            );
                            client.disconnect().await;
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        server = %name,
                        error = %e,
                        "MCP: failed to connect to server"
                    );
                }
            }
        }

        Ok(())
    }

    /// Get all tools from all connected servers.
    pub fn all_tools(&self) -> Vec<McpToolDef> {
        self.clients
            .values()
            .flat_map(|c| c.tools.iter().cloned())
            .collect()
    }

    /// Get all resources from all connected servers.
    #[allow(dead_code)]
    pub fn all_resources(&self) -> Vec<McpResource> {
        self.clients
            .values()
            .flat_map(|c| c.resources.iter().cloned())
            .collect()
    }

    /// Find the client that owns a tool by name.
    #[allow(dead_code)]
    pub fn find_client_for_tool(&self, tool_name: &str) -> Option<&McpClient> {
        self.clients
            .values()
            .find(|c| c.tools.iter().any(|t| t.name == tool_name))
    }

    /// Disconnect from all servers.
    #[allow(dead_code)]
    pub async fn disconnect_all(&mut self) {
        let names: Vec<String> = self.clients.keys().cloned().collect();
        for name in names {
            if let Some(mut client) = self.clients.remove(&name) {
                client.disconnect().await;
            }
        }
    }
}
