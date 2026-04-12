//! [`impl_audio_handler!`]: shared macro for audio handler structs.

/// Implement [`Handler`] + [`AudioHandler`] + inherent methods for an
/// audio handler struct that holds raw bytes.
macro_rules! impl_audio_handler {
    ($handler:ident, $doc_type:expr, $origin:literal, $encode_name:literal) => {
        impl crate::handler::Handler for $handler {
            fn document_type(&self) -> nvisy_core::media::DocumentType {
                $doc_type
            }

            fn source(&self) -> nvisy_core::content::ContentSource {
                self.source
            }

            #[tracing::instrument(name = $encode_name, skip_all, fields(output_bytes))]
            fn encode(&self) -> Result<nvisy_core::content::ContentData, nvisy_core::Error> {
                tracing::Span::current().record("output_bytes", self.bytes.len());
                let source = nvisy_core::content::ContentSource::new().with_parent(&self.source);
                Ok(nvisy_core::content::ContentData::new(
                    source,
                    self.bytes.clone(),
                ))
            }
        }

        #[async_trait::async_trait]
        impl crate::handler::AudioHandler for $handler {
            async fn audio_spans(
                &self,
            ) -> crate::document::SpanStream<
                '_,
                nvisy_ontology::entity::AudioLocation,
                crate::handler::AudioData,
            > {
                // Single-track audio: the entire audio as one span
                // with a time span covering the full duration.
                // Duration is unknown without decoding — use 0..0 as
                // a placeholder. The actual time span is set by the
                // STT extraction operation after transcription.
                let location = nvisy_ontology::entity::AudioLocation {
                    time_span: nvisy_ontology::math::TimeSpan {
                        start_us: 0,
                        end_us: 0,
                    },
                    speaker_id: None,
                    audio_id: None,
                };
                crate::document::SpanStream::new(futures::stream::iter(std::iter::once(
                    crate::document::Span::new(
                        location,
                        crate::handler::AudioData::new(self.bytes.clone()),
                    ),
                )))
            }

            async fn edit_audio(
                &mut self,
                edits: crate::document::SpanStream<
                    '_,
                    nvisy_ontology::entity::AudioLocation,
                    crate::handler::AudioData,
                >,
            ) -> Result<(), nvisy_core::Error> {
                use futures::StreamExt;
                let edits: Vec<_> = edits.collect().await;
                if let Some(edit) = edits.into_iter().next() {
                    self.bytes = edit.data.into_inner();
                }
                Ok(())
            }

            async fn value_at(
                &self,
                _location: &nvisy_ontology::entity::AudioLocation,
            ) -> Option<crate::handler::AudioData> {
                // Full audio segment: extracting a sub-segment by
                // time span requires decoding, which we don't do here.
                Some(crate::handler::AudioData::new(self.bytes.clone()))
            }
        }

        impl $handler {
            /// Create a handler from raw audio bytes.
            pub fn new(bytes: bytes::Bytes) -> Self {
                Self {
                    source: nvisy_core::content::ContentSource::new(),
                    bytes,
                }
            }

            /// Set the content source for lineage tracking.
            pub fn with_source(mut self, source: nvisy_core::content::ContentSource) -> Self {
                self.source = source;
                self
            }

            /// Reference to the raw audio bytes.
            pub fn bytes(&self) -> &bytes::Bytes {
                &self.bytes
            }
        }
    };
}

pub(crate) use impl_audio_handler;
