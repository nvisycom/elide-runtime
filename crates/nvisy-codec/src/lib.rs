#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(not(any(
    feature = "internal_text",
    feature = "internal_tabular",
    feature = "internal_image",
    feature = "internal_audio",
    feature = "internal_rich",
)))]
compile_error!(
    "nvisy-codec requires at least one format feature \
     (txt/json/markdown/html/csv/xlsx/png/jpeg/tiff/wav/mp3/pdf/docx) \
     — or the umbrella `text`/`tabular`/`image`/`audio`/`rich`"
);

pub mod content;
mod core;
mod document;
pub mod handler;

pub use self::core::{Chunk, CodecRegistry, Format, FormatId, Handler, Loader};
pub use self::document::{DocumentHandle, UntypedDocumentHandle};

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "txt")]
    #[tokio::test]
    async fn registry_decodes_txt_from_extension() {
        let reg = CodecRegistry::with_builtin();
        let handle = reg
            .decode("hello\nworld\n".to_owned().into_bytes(), "txt")
            .await
            .expect("txt decoded");
        let typed = handle.into_text().expect("text variant");
        assert_eq!(typed.format_id().as_str(), "nvisy.text.txt");
    }

    #[cfg(feature = "csv")]
    #[tokio::test]
    async fn registry_decodes_csv_from_extension() {
        let reg = CodecRegistry::with_builtin();
        let handle = reg
            .decode("a,b\n1,2\n".to_owned().into_bytes(), "csv")
            .await
            .expect("csv decoded");
        let typed = handle.into_tabular().expect("tabular variant");
        assert_eq!(typed.format_id().as_str(), "nvisy.tabular.csv");
    }

    #[cfg(all(feature = "txt", feature = "csv"))]
    #[test]
    fn registry_resolves_by_extension_and_content_type() {
        let reg = CodecRegistry::with_builtin();
        assert_eq!(
            reg.by_extension("txt").map(|f| f.id.as_str()),
            Some("nvisy.text.txt"),
        );
        assert_eq!(
            reg.by_extension("log").map(|f| f.id.as_str()),
            Some("nvisy.text.txt"),
        );
        assert_eq!(
            reg.by_content_type("text/csv").map(|f| f.id.as_str()),
            Some("nvisy.tabular.csv"),
        );
    }

    #[cfg(feature = "txt")]
    #[test]
    fn empty_registry_skipped_when_user_wants_no_builtins() {
        let reg = CodecRegistry::new();
        assert!(reg.by_extension("txt").is_none());
    }
}
