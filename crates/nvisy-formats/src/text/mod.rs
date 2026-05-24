//! Text-format implementations: TXT, JSON, Markdown, HTML.

#[cfg(feature = "html")]
mod html_handler;
#[cfg(feature = "html")]
mod html_loader;
#[cfg(feature = "json")]
mod json_handler;
#[cfg(feature = "json")]
mod json_loader;
#[cfg(feature = "markdown")]
mod markdown_loader;
#[cfg(feature = "txt")]
mod txt_handler;
#[cfg(feature = "txt")]
mod txt_loader;

#[cfg(feature = "html")]
pub use self::html_handler::{HtmlData, HtmlHandler};
#[cfg(feature = "html")]
pub use self::html_loader::{HtmlLoader, HtmlParams};
#[cfg(feature = "json")]
pub use self::json_handler::{JsonData, JsonHandler, JsonIndent};
#[cfg(feature = "json")]
pub use self::json_loader::{JsonLoader, JsonParams};
#[cfg(feature = "markdown")]
pub use self::markdown_loader::{MarkdownLoader, MarkdownParams};
#[cfg(feature = "txt")]
pub use self::txt_handler::TxtHandler;
#[cfg(feature = "txt")]
pub use self::txt_loader::{TxtLoader, TxtParams};
