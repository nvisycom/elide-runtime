#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(feature = "internal_audio")]
pub mod audio;
#[cfg(feature = "internal_image")]
pub mod image;
#[cfg(feature = "internal_rich")]
pub mod rich;
#[cfg(feature = "internal_tabular")]
pub mod tabular;
#[cfg(feature = "internal_text")]
pub mod text;

use nvisy_codec::CodecRegistry;

/// Extension trait adding built-in format registration to
/// [`CodecRegistry`]. Bring it into scope with
/// `use nvisy_formats::CodecRegistryExt;`.
///
/// - [`builtins`][b] — fresh registry preloaded with every built-in
///   format the active feature set enables.
/// - [`with_builtins`][wb] — builder form: adds the built-ins to an
///   existing registry (typically pre-seeded with custom formats).
///
/// [b]: CodecRegistryExt::builtins
/// [wb]: CodecRegistryExt::with_builtins
pub trait CodecRegistryExt: Sized {
    /// Fresh [`CodecRegistry`] preloaded with every built-in format.
    fn builtins() -> Self;

    /// Add every built-in format to this registry. Downstream
    /// formats registered first take precedence on extension /
    /// content-type collisions (last registration wins).
    fn with_builtins(self) -> Self;
}

impl CodecRegistryExt for CodecRegistry {
    fn builtins() -> Self {
        Self::new().with_builtins()
    }

    fn with_builtins(mut self) -> Self {
        #[cfg(feature = "txt")]
        self.register(crate::text::txt_format());
        #[cfg(feature = "json")]
        self.register(crate::text::json_format());
        #[cfg(feature = "markdown")]
        self.register(crate::text::markdown_format());
        #[cfg(feature = "html")]
        self.register(crate::text::html_format());

        #[cfg(feature = "csv")]
        self.register(crate::tabular::csv_format());
        #[cfg(feature = "xlsx")]
        self.register(crate::tabular::xlsx_format());

        #[cfg(feature = "png")]
        self.register(crate::image::png_format());
        #[cfg(feature = "jpeg")]
        self.register(crate::image::jpeg_format());
        #[cfg(feature = "tiff")]
        self.register(crate::image::tiff_format());

        #[cfg(feature = "wav")]
        self.register(crate::audio::wav_format());
        #[cfg(feature = "mp3")]
        self.register(crate::audio::mp3_format());

        #[cfg(feature = "pdf")]
        self.register(crate::rich::pdf_format());
        #[cfg(feature = "docx")]
        self.register(crate::rich::docx_format());

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "txt")]
    #[tokio::test]
    async fn registry_decodes_txt_from_extension() {
        let reg = CodecRegistry::builtins();
        let handle = reg
            .decode_from_memory("hello\nworld\n".to_owned().into_bytes(), "txt")
            .await
            .expect("txt decoded");
        let typed = handle.into_text().expect("text variant");
        assert_eq!(typed.format().as_str(), "nvisy.text.txt");
    }

    #[cfg(feature = "csv")]
    #[tokio::test]
    async fn registry_decodes_csv_from_extension() {
        let reg = CodecRegistry::builtins();
        let handle = reg
            .decode_from_memory("a,b\n1,2\n".to_owned().into_bytes(), "csv")
            .await
            .expect("csv decoded");
        let typed = handle.into_tabular().expect("tabular variant");
        assert_eq!(typed.format().as_str(), "nvisy.tabular.csv");
    }

    #[cfg(all(feature = "txt", feature = "csv"))]
    #[test]
    fn registry_resolves_by_extension_and_content_type() {
        let reg = CodecRegistry::builtins();
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
    fn with_builtins_extends_existing_registry() {
        let reg = CodecRegistry::new().with_builtins();
        assert!(reg.by_extension("txt").is_some());
    }
}
