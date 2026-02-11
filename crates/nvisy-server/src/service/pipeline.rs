//! Pipeline builder and executor for the `/redact` endpoint.
//!
//! Auto-detects file type and constructs the correct action sequence,
//! then executes actions sequentially via mpsc channels.

use bytes::Bytes;
use std::collections::HashMap;
use tokio::sync::mpsc;

use nvisy_core::datatypes::blob::Blob;
use nvisy_core::error::{Error, ErrorKind};
use nvisy_ontology::ontology::entity::Entity;
use nvisy_ontology::ontology::redaction::Redaction;
use nvisy_ontology::redaction::RedactionContext;
use nvisy_core::registry::action::Action;
use nvisy_ingest::loaders::{Loader, LoaderOutput};

use nvisy_detect::actions::detect_dictionary::{DetectDictionaryAction, DetectDictionaryParams, DictionaryDef};
use nvisy_detect::actions::detect_manual::{DetectManualAction, DetectManualParams};
use nvisy_detect::actions::detect_regex::{DetectRegexAction, DetectRegexParams};
use nvisy_detect::actions::detect_tabular::{DetectTabularAction, DetectTabularParams};
use nvisy_detect::actions::detect_checksum::DetectChecksumParams;
use nvisy_detect::actions::evaluate_policy::{EvaluatePolicyAction, EvaluatePolicyParams};
use nvisy_detect::actions::emit_audit::EmitAuditParams;

use nvisy_media::actions::apply_image_redaction::{ApplyImageRedactionAction, ApplyImageRedactionParams};
use nvisy_media::actions::apply_tabular_redaction::{ApplyTabularRedactionAction, ApplyTabularRedactionParams};
use nvisy_media::actions::apply_pdf_redaction::{ApplyPdfRedactionAction, ApplyPdfRedactionParams};

/// Result of a pipeline execution.
#[derive(Debug, serde::Serialize)]
pub struct PipelineResult {
    /// Redacted file content.
    #[serde(skip)]
    pub content: Bytes,
    /// Output file name.
    pub file_name: String,
    /// Content type of the output.
    pub content_type: String,
    /// Execution summary.
    pub summary: PipelineSummary,
    /// Audit trail entries.
    pub audit_trail: Vec<serde_json::Value>,
}

/// Summary statistics for a pipeline run.
#[derive(Debug, serde::Serialize, schemars::JsonSchema, utoipa::ToSchema)]
pub struct PipelineSummary {
    pub total_entities: usize,
    pub total_redactions: usize,
    pub entities_by_category: HashMap<String, usize>,
    pub processing_time_ms: u64,
}

/// Execute the full redaction pipeline for a file.
pub async fn execute_pipeline(
    file_bytes: Bytes,
    file_name: &str,
    content_type: &str,
    context: &RedactionContext,
    dictionaries: &[DictionaryDef],
) -> Result<PipelineResult, Error> {
    let start = std::time::Instant::now();

    // Create blob
    let mut blob = Blob::new(file_name, file_bytes);
    blob = blob.with_content_type(content_type);

    // Step 1: Load file
    blob = run_loader(&blob, content_type, file_name).await?;

    // Step 2: Inject manual entities if present
    if !context.manual_entities.is_empty() {
        for ann in &context.manual_entities {
            blob.add_artifact("manual_entities", ann).map_err(|e| {
                Error::new(ErrorKind::Runtime, format!("failed to add manual entity: {e}"))
            })?;
        }
    }

    // Step 3: Run detection actions
    blob = run_action(&DetectRegexAction, blob, DetectRegexParams {
        confidence_threshold: context.min_confidence,
        patterns: None,
    }).await?;

    // Dictionary detection
    if !dictionaries.is_empty() {
        blob = run_action(&DetectDictionaryAction, blob, DetectDictionaryParams {
            dictionaries: dictionaries.to_vec(),
            confidence: 0.85,
        }).await?;
    }

    // Tabular detection if we have tabular data
    let has_tabular = blob.has_artifacts("tabular");
    if has_tabular {
        blob = run_action(&DetectTabularAction, blob, DetectTabularParams {
            column_rules: vec![],
        }).await?;
    }

    // Manual entity detection
    if !context.manual_entities.is_empty() {
        blob = run_action(&DetectManualAction, blob, DetectManualParams {}).await?;
    }

    // Checksum validation
    blob = run_action(&nvisy_detect::actions::detect_checksum::DetectChecksumAction, blob, DetectChecksumParams {
        drop_invalid: true,
        confidence_boost: 0.05,
    }).await?;

    // Classification
    blob = run_action(&nvisy_detect::actions::classify::ClassifyAction, blob, ()).await?;

    // Step 4: Policy evaluation
    blob = run_action(&EvaluatePolicyAction, blob, EvaluatePolicyParams {
        rules: context.rules.iter().map(|r| {
            nvisy_ontology::redaction::PolicyRule {
                id: r.entity_type.clone(),
                name: r.entity_type.clone(),
                categories: vec![],
                entity_types: vec![r.entity_type.clone()],
                confidence_threshold: context.min_confidence,
                method: r.method,
                replacement_template: r.replacement.clone().unwrap_or_default(),
                enabled: true,
                priority: 0,
            }
        }).collect(),
        default_method: context.default_method,
        default_confidence_threshold: context.min_confidence,
    }).await?;

    // Step 5: Apply redactions
    blob = run_action(&nvisy_detect::actions::apply_redaction::ApplyRedactionAction, blob, ()).await?;

    // Apply image redaction if we have images
    let has_images = blob.has_artifacts("images");
    if has_images {
        blob = run_action(&ApplyImageRedactionAction, blob, ApplyImageRedactionParams {
            blur_sigma: 15.0,
            block_color: [0, 0, 0, 255],
        }).await?;
    }

    // Apply tabular redaction
    if has_tabular {
        blob = run_action(&ApplyTabularRedactionAction, blob, ApplyTabularRedactionParams {}).await?;
    }

    // Apply PDF reassembly if this is a PDF
    if content_type == "application/pdf" {
        blob = run_action(&ApplyPdfRedactionAction, blob, ApplyPdfRedactionParams {}).await?;
    }

    // Step 6: Audit
    blob = run_action(&nvisy_detect::actions::emit_audit::EmitAuditAction, blob, EmitAuditParams {
        run_id: None,
        actor: None,
    }).await?;

    // Collect results
    let entities: Vec<Entity> = blob.get_artifacts("entities").unwrap_or_default();
    let redactions: Vec<Redaction> = blob.get_artifacts("redactions").unwrap_or_default();
    let audit_trail: Vec<serde_json::Value> = blob.get_artifacts("audit").unwrap_or_default();

    let mut entities_by_category: HashMap<String, usize> = HashMap::new();
    for entity in &entities {
        *entities_by_category
            .entry(format!("{:?}", entity.category).to_lowercase())
            .or_insert(0) += 1;
    }

    let elapsed = start.elapsed();

    let output_file_name = format!("redacted_{}", file_name);

    Ok(PipelineResult {
        content: blob.content,
        file_name: output_file_name,
        content_type: content_type.to_string(),
        summary: PipelineSummary {
            total_entities: entities.len(),
            total_redactions: redactions.len(),
            entities_by_category,
            processing_time_ms: elapsed.as_millis() as u64,
        },
        audit_trail,
    })
}

/// Run a file loader based on content type and extension.
async fn run_loader(blob: &Blob, content_type: &str, file_name: &str) -> Result<Blob, Error> {
    let mut result_blob = blob.clone();
    let ext = file_name
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();

    let outputs: Vec<LoaderOutput> = match (content_type, ext.as_str()) {
        ("application/pdf", _) | (_, "pdf") => {
            let loader = nvisy_ingest::loaders::pdf_loader::PdfLoader;
            let params = nvisy_ingest::loaders::pdf_loader::PdfLoaderParams {
                extract_images: true,
                max_pages: None,
            };
            loader.load(blob, &params).await?
        }
        (ct, _) if ct.contains("wordprocessingml") => {
            let loader = nvisy_ingest::loaders::docx_loader::DocxLoader;
            let params = nvisy_ingest::loaders::docx_loader::DocxLoaderParams {
                extract_images: true,
            };
            loader.load(blob, &params).await?
        }
        (_, "docx") => {
            let loader = nvisy_ingest::loaders::docx_loader::DocxLoader;
            let params = nvisy_ingest::loaders::docx_loader::DocxLoaderParams {
                extract_images: true,
            };
            loader.load(blob, &params).await?
        }
        ("text/html", _) | (_, "html") | (_, "htm") => {
            let loader = nvisy_ingest::loaders::html_loader::HtmlLoader;
            let params = nvisy_ingest::loaders::html_loader::HtmlLoaderParams {};
            loader.load(blob, &params).await?
        }
        (ct, _) if ct.starts_with("image/") => {
            let loader = nvisy_ingest::loaders::image_loader::ImageLoader;
            let params = nvisy_ingest::loaders::image_loader::ImageLoaderParams {};
            loader.load(blob, &params).await?
        }
        (_, "jpg") | (_, "jpeg") | (_, "png") | (_, "tiff") | (_, "bmp") | (_, "webp") => {
            let loader = nvisy_ingest::loaders::image_loader::ImageLoader;
            let params = nvisy_ingest::loaders::image_loader::ImageLoaderParams {};
            loader.load(blob, &params).await?
        }
        (_, "parquet") => {
            let loader = nvisy_ingest::loaders::parquet_loader::ParquetLoader;
            let params = nvisy_ingest::loaders::parquet_loader::ParquetLoaderParams {
                max_rows: None,
            };
            loader.load(blob, &params).await?
        }
        (ct, _) if ct.contains("spreadsheetml") || ct.contains("ms-excel") => {
            let loader = nvisy_ingest::loaders::xlsx_loader::XlsxLoader;
            let params = nvisy_ingest::loaders::xlsx_loader::XlsxLoaderParams {
                max_rows: None,
                sheets: vec![],
            };
            loader.load(blob, &params).await?
        }
        (_, "xlsx") | (_, "xls") => {
            let loader = nvisy_ingest::loaders::xlsx_loader::XlsxLoader;
            let params = nvisy_ingest::loaders::xlsx_loader::XlsxLoaderParams {
                max_rows: None,
                sheets: vec![],
            };
            loader.load(blob, &params).await?
        }
        ("text/csv", _) | (_, "csv") => {
            let loader = nvisy_ingest::loaders::csv_loader::CsvLoader;
            loader.load(blob, &()).await?
        }
        ("application/json", _) | (_, "json") => {
            let loader = nvisy_ingest::loaders::json_loader::JsonLoader;
            loader.load(blob, &()).await?
        }
        (ct, _) if ct.starts_with("audio/") => {
            let loader = nvisy_ingest::loaders::audio_loader::AudioLoader;
            let params = nvisy_ingest::loaders::audio_loader::AudioLoaderParams {};
            loader.load(blob, &params).await?
        }
        (_, "mp3") | (_, "wav") | (_, "flac") | (_, "ogg") | (_, "m4a") => {
            let loader = nvisy_ingest::loaders::audio_loader::AudioLoader;
            let params = nvisy_ingest::loaders::audio_loader::AudioLoaderParams {};
            loader.load(blob, &params).await?
        }
        // Default: treat as plain text
        _ => {
            let loader = nvisy_ingest::loaders::plaintext::PlaintextLoader;
            loader.load(blob, &()).await?
        }
    };

    // Add loader outputs as artifacts
    for output in outputs {
        match output {
            LoaderOutput::Document(doc) => {
                result_blob.add_artifact("documents", &doc).map_err(|e| {
                    Error::new(ErrorKind::Runtime, format!("failed to add document: {e}"))
                })?;
            }
            LoaderOutput::Image(img) => {
                result_blob.add_artifact("images", &img).map_err(|e| {
                    Error::new(ErrorKind::Runtime, format!("failed to add image: {e}"))
                })?;
            }
            LoaderOutput::Tabular(tab) => {
                result_blob.add_artifact("tabular", &tab).map_err(|e| {
                    Error::new(ErrorKind::Runtime, format!("failed to add tabular: {e}"))
                })?;
            }
        }
    }

    Ok(result_blob)
}

/// Run a single action on a blob, returning the processed blob.
async fn run_action<A: Action>(
    action: &A,
    blob: Blob,
    params: A::Params,
) -> Result<Blob, Error> {
    let (tx_in, rx_in) = mpsc::channel(1);
    let (tx_out, mut rx_out) = mpsc::channel(1);

    tx_in.send(blob).await.map_err(|_| {
        Error::new(ErrorKind::Runtime, "failed to send blob to action")
    })?;
    drop(tx_in);

    action.execute(rx_in, tx_out, params).await?;

    rx_out.recv().await.ok_or_else(|| {
        Error::new(ErrorKind::Runtime, "action produced no output")
    })
}
