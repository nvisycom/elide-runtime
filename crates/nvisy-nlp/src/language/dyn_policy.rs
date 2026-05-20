//! Object-safe shim for [`LanguagePolicy`] plus an `Arc<P>`
//! forwarder so policies can be cheaply shared.
//!
//! [`LanguagePolicy`] carries an associated `Detector` type and so
//! is not object-safe on its own. The engine still wants
//! `Arc<dyn _>` storage. [`DynLanguagePolicy`] is the sealed,
//! crate-private twin used for that purpose: it returns
//! `Box<dyn LanguageDetector>` from each constructor so the
//! associated type drops out.
//!
//! The blanket `impl<P: LanguagePolicy> DynLanguagePolicy for P`
//! means external code only ever implements [`LanguagePolicy`];
//! erasure is automatic.

use std::sync::Arc;

use nvisy_ontology::primitive::LanguageTag;

use super::{LanguageDetector, LanguagePolicy};

/// Object-safe twin of [`LanguagePolicy`] used by the engine to
/// hold a policy behind `Arc<dyn _>`.
///
/// Sealed (crate-private) and implemented blanket-style for every
/// [`LanguagePolicy`]. External code implements [`LanguagePolicy`]
/// and gets erasure for free.
pub(crate) trait DynLanguagePolicy: Send + Sync {
    fn detector_for_all(&self) -> Box<dyn LanguageDetector>;
    fn detector_for(&self, languages: &[LanguageTag]) -> Box<dyn LanguageDetector>;
}

impl<P> DynLanguagePolicy for P
where
    P: LanguagePolicy + ?Sized,
{
    fn detector_for_all(&self) -> Box<dyn LanguageDetector> {
        Box::new(<P as LanguagePolicy>::detector_for_all(self))
    }

    fn detector_for(&self, languages: &[LanguageTag]) -> Box<dyn LanguageDetector> {
        Box::new(<P as LanguagePolicy>::detector_for(self, languages))
    }
}

/// `Arc<P>` is itself a [`LanguagePolicy`] when `P` is. Lets
/// callers share a configured policy between engines without
/// re-wrapping.
impl<P> LanguagePolicy for Arc<P>
where
    P: LanguagePolicy + ?Sized,
{
    type Detector = P::Detector;

    fn detector_for_all(&self) -> Self::Detector {
        (**self).detector_for_all()
    }

    fn detector_for(&self, languages: &[LanguageTag]) -> Self::Detector {
        (**self).detector_for(languages)
    }
}
