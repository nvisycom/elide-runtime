//! OCR adapter round trips against the real Python service.

#![cfg(feature = "ocr")]

mod support;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use elide_bentoml::ocr::BentoOcr;
use elide_core::backend::Backend;
use elide_image::modality::ImageFormat;
use elide_image::ocr::OcrRequest;
use elide_image::primitive::Dimensions;
use support::{Server, status};

#[tokio::test]
#[ignore = "requires Docker; builds and runs real BentoML service containers"]
async fn ocr_roundtrip_decodes_image_filters_words_and_projects_pixels() {
    let server = Server::start("ocr").await;
    let backend = BentoOcr::new(&server.url, "e2e/ocr")
        .unwrap()
        .with_default_threshold(0.5);
    let request = |image| OcrRequest {
        image,
        format: ImageFormat::Png,
        dimensions: Dimensions::new(100, 50),
        language: None,
        correlation_id: None,
    };
    let image = image_fixture();
    let responses = backend
        .call_batch(vec![request(&image), request(&image)])
        .await
        .unwrap();
    assert_eq!(responses.len(), 2);
    for response in responses {
        assert_eq!(
            response.regions.len(),
            1,
            "confidence_threshold must filter the weak word"
        );
        let region = &response.regions[0];
        assert_eq!(region.text, "Ada");
        assert_eq!(region.region.page, Some(1));
        let bbox = region.region.bounding_box;
        assert_eq!(
            [bbox.min.x, bbox.min.y, bbox.max.x, bbox.max.y],
            [10.0, 10.0, 40.0, 20.0]
        );
    }
    assert_eq!(
        status(&backend.call(request(b"not an image")).await.unwrap_err()),
        Some(400)
    );
}

/// Reuse the contract fixture's valid PNG rather than embedding a second image.
fn image_fixture() -> Vec<u8> {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("../../../../contracts/ocr/v1/fixtures.json")).unwrap();
    BASE64
        .decode(
            fixtures[0]["request"]["requests"][0]["image"]
                .as_str()
                .unwrap(),
        )
        .unwrap()
}
