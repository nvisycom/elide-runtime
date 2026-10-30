//! Container lifecycle and binary fixtures shared by the adapter E2E tests.

use std::path::PathBuf;
use std::time::Duration;

use testcontainers::core::logs::LogFrame;
use testcontainers::core::wait::HttpWaitStrategy;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::{AsyncBuilder, AsyncRunner};
use testcontainers::{ContainerAsync, GenericBuildableImage, GenericImage, ImageExt};

// Build once per test executable; Docker reuses dependency layers across runs.
static IMAGE: tokio::sync::OnceCell<GenericImage> = tokio::sync::OnceCell::const_new();

/// Keeps the service container alive until its test finishes.
pub(super) struct Server {
    _container: ContainerAsync<GenericImage>,
    pub(super) url: String,
}

impl Server {
    /// Start an isolated service and wait for its real BentoML readiness endpoint.
    pub(super) async fn start(task: &str) -> Self {
        let image = IMAGE
            .get_or_init(|| async {
                let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .ancestors()
                    .nth(3)
                    .unwrap()
                    .to_owned();
                // Sources and manifests only, never a whole package directory:
                // each package keeps its own .venv, which a build context has
                // no use for and would upload in full.
                let mut image = GenericBuildableImage::new("elide-bento-e2e", "local")
                    .with_dockerfile(
                        root.join("crates/adapter/elide-bentoml/tests/support/Dockerfile"),
                    )
                    .with_file(root.join("pyproject.toml"), "pyproject.toml")
                    .with_file(root.join("uv.lock"), "uv.lock")
                    .with_file(
                        root.join("packages/elide-core/pyproject.toml"),
                        "packages/elide-core/pyproject.toml",
                    );
                for package in ["elide-core", "elide-ner", "elide-ocr", "elide-stt"] {
                    let src = format!("packages/{package}/src");
                    image = image.with_file(root.join(&src), src);
                }
                image
                    .with_file(
                        root.join("crates/adapter/elide-bentoml/tests/support/server.py"),
                        "server.py",
                    )
                    .build_image()
                    .await
                    .expect("build BentoML test image")
            })
            .await
            .clone();
        let container = image
            .with_exposed_port(3000.tcp())
            .with_wait_for(WaitFor::http(
                HttpWaitStrategy::new("/readyz").with_expected_status_code(200u16),
            ))
            .with_cmd([task])
            .with_startup_timeout(Duration::from_secs(60))
            .with_log_consumer(|frame: &LogFrame| match frame {
                LogFrame::StdOut(bytes) | LogFrame::StdErr(bytes) => {
                    eprint!("{}", String::from_utf8_lossy(bytes))
                }
            })
            .start()
            .await
            .expect("start BentoML test container");
        let host = container.get_host().await.unwrap();
        let port = container.get_host_port_ipv4(3000.tcp()).await.unwrap();
        Self {
            _container: container,
            url: format!("http://{host}:{port}"),
        }
    }
}

/// Find the HTTP status through the adapter's domain-error wrapper.
pub(super) fn status(error: &elide_core::Error) -> Option<u16> {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(current) = source {
        if let Some(error) = current.downcast_ref::<bentoml::Error>() {
            return error.status();
        }
        source = current.source();
    }
    None
}
