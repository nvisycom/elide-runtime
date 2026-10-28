//! Convert Rust OCR requests into Python-owned wire types.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use elide_image::ocr::OcrRequest;

use super::wire::OcrRequest as WireOcrRequest;
use crate::error::BentoError;

impl WireOcrRequest {
    pub(super) fn from_request(
        request: &OcrRequest<'_>,
        threshold: f32,
    ) -> Result<Self, BentoError> {
        if !(0.0..=1.0).contains(&threshold) {
            return Err(BentoError::Protocol(
                "OCR threshold must be in [0, 1]".into(),
            ));
        }
        Ok(Self {
            image: BASE64.encode(request.image),
            confidence_threshold: Some(f64::from(threshold)),
        })
    }
}

#[cfg(test)]
mod tests {
    use elide_image::modality::ImageFormat;
    use elide_image::primitive::Dimensions;

    use super::*;
    use crate::ocr::wire::OcrInput;

    #[test]
    fn domain_requests_match_python_fixtures() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../contracts/ocr/v1/fixtures.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let payload = &case["request"]["requests"][0];
            let bytes = BASE64.decode(payload["image"].as_str().unwrap()).unwrap();
            let request = OcrRequest {
                image: &bytes,
                format: ImageFormat::Png,
                dimensions: Dimensions::new(100, 50),
                language: None,
                correlation_id: None,
            };
            let wire = OcrInput {
                requests: vec![WireOcrRequest::from_request(&request, 0.0).unwrap()],
            };
            assert_eq!(serde_json::to_value(wire).unwrap(), case["request"]);
            assert!(WireOcrRequest::from_request(&request, f32::NAN).is_err());
        }
    }
}
