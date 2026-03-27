//! Extraction node configurations: visual and audial.
//!
//! Both extraction nodes run at **phase 1**, converting raw binary content
//! into structured text that downstream detection nodes can operate on.
//! [`VisualExtraction`] handles images and scanned documents via OCR;
//! [`AudialExtraction`] handles speech audio via automatic speech recognition.

mod speech;
mod vision;

pub use self::speech::AudialExtraction;
pub use self::vision::VisualExtraction;
