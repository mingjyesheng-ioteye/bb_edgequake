//! HTTP server.
//!
//! Provides the main HTTP server with middleware and configuration.
//!
//! ## Implements
//!
//! - [`FEAT0440`]: HTTP server with Axum
//! - [`FEAT0441`]: CORS configuration
//! - [`FEAT0442`]: Response compression
//! - [`FEAT0443`]: Swagger UI integration
//!
//! ## Use Cases
//!
//! - [`UC2040`]: System starts HTTP server
//! - [`UC2041`]: System serves OpenAPI documentation
//!
//! ## Enforces
//!
//! - [`BR0440`]: Configurable host and port
//! - [`BR0441`]: Optional feature toggles (CORS, compression, Swagger)

use std::net::SocketAddr;

use axum::extract::DefaultBodyLimit;
use axum::middleware;
use serde::{Deserialize, Serialize};
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::middleware::{request_id, request_logging};
use crate::openapi::ApiDoc;
use crate::routes::create_router;
use crate::state::AppState;

/// Server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server host.
    pub host: String,

    /// Server port.
    pub port: u16,

    /// Enable CORS.
    pub enable_cors: bool,

    /// Enable compression.
    pub enable_compression: bool,

    /// Enable Swagger UI.
    pub enable_swagger: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            enable_cors: true,
            enable_compression: true,
            enable_swagger: true,
        }
    }
}

/// The HTTP server.
pub struct Server {
    config: ServerConfig,
    state: AppState,
}

impl Server {
    /// Create a new server.
    pub fn new(config: ServerConfig, state: AppState) -> Self {
        Self { config, state }
    }

    /// Build the application router with all middleware.
    pub fn build_router(&self) -> axum::Router {
        let mut app = create_router(self.state.clone());

        // Optional: serve the edgequake_webui static export.
        // Priority order:
        //   1. EDGEQUAKE_WEBUI_DIR env var (explicit path)
        //   2. Auto-discovery: look for webui/ or out/ adjacent to the exe
        let webui_path: Option<std::path::PathBuf> = {
            // 1. Explicit env var
            let from_env = std::env::var("EDGEQUAKE_WEBUI_DIR").ok().and_then(|d| {
                let p = std::path::PathBuf::from(d);
                if p.is_dir() { Some(p) } else { None }
            });
            // 2. Adjacent to executable
            let from_exe = from_env.is_none().then(|| {
                std::env::current_exe().ok().and_then(|exe| {
                    let dir = exe.parent()?;
                    for candidate in &["webui", "edgequake-webui", "out"] {
                        let p = dir.join(candidate);
                        if p.is_dir() && p.join("index.html").exists() {
                            return Some(p);
                        }
                    }
                    None
                })
            }).flatten();
            from_env.or(from_exe)
        };

        if let Some(webui_path) = webui_path {
            let index_path = webui_path.join("index.html");
            // Read index.html once for the SPA fallback.
            // tower-http 0.6's not_found_service preserves the 404 status code;
            // a custom fallback handler is needed to return 200 so Next.js
            // client-side routing can render the correct page.
            let index_html = std::sync::Arc::new(
                std::fs::read(&index_path).unwrap_or_default(),
            );
            let webui = std::sync::Arc::new(webui_path.clone());
            let idx = index_html.clone();
            app = app.fallback(move |req: axum::extract::Request| {
                let webui = webui.clone();
                let idx = idx.clone();
                async move {
                    use tower::ServiceExt as _;
                    let serve = ServeDir::new(webui.as_ref());
                    match serve.oneshot(req).await {
                        Ok(resp)
                            if resp.status().is_success()
                                || resp.status().is_redirection() =>
                        {
                            resp.map(axum::body::Body::new)
                        }
                        _ => axum::response::Response::builder()
                            .status(axum::http::StatusCode::OK)
                            .header(
                                axum::http::header::CONTENT_TYPE,
                                "text/html; charset=utf-8",
                            )
                            .body(axum::body::Body::from((*idx).clone()))
                            .unwrap(),
                    }
                }
            });
            info!("WebUI static files served from {}", webui_path.display());
        } else {
            tracing::debug!("No webui directory found — serving API only");
        }

        // Add middleware
        app = app
            .layer(DefaultBodyLimit::max(100 * 1024 * 1024)) // 100 MB limit for file uploads
            .layer(middleware::from_fn(request_logging))
            .layer(middleware::from_fn(request_id))
            .layer(TraceLayer::new_for_http());

        // CORS
        if self.config.enable_cors {
            let cors = CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any);
            app = app.layer(cors);
        }

        // Compression
        if self.config.enable_compression {
            app = app.layer(CompressionLayer::new());
        }

        // Swagger UI
        if self.config.enable_swagger {
            app = app.merge(
                SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()),
            );
        }

        app
    }

    /// Run the server.
    pub async fn run(self) -> Result<(), std::io::Error> {
        let app = self.build_router();
        let addr: SocketAddr = format!("{}:{}", self.config.host, self.config.port)
            .parse()
            .expect("Invalid address");

        info!("Starting EdgeQuake API server on {}", addr);

        if self.config.enable_swagger {
            info!("Swagger UI available at http://{}/swagger-ui", addr);
        }

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await
    }

    /// Get the server configuration.
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_default() {
        let config = ServerConfig::default();

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert!(config.enable_cors);
        assert!(config.enable_swagger);
    }

    #[test]
    fn test_build_router() {
        let config = ServerConfig::default();
        let state = AppState::test_state();
        let server = Server::new(config, state);

        let _router = server.build_router();
        // Router builds successfully
    }
}
