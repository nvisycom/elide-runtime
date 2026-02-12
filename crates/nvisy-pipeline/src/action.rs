//! The `Action` trait -- the fundamental processing unit in a pipeline.

use serde::de::DeserializeOwned;

use nvisy_core::error::Error;

/// A processing step with typed input and output.
///
/// Actions are the primary unit of work in a pipeline. Each action is
/// constructed via [`connect`](Action::connect), which validates and
/// stores parameters, then executed via [`execute`](Action::execute).
///
/// Actions that need a provider client should hold it as a struct field
/// rather than receiving it as a parameter.
#[async_trait::async_trait]
pub trait Action: Sized + Send + Sync + 'static {
    /// Strongly-typed parameters for this action.
    type Params: DeserializeOwned + Send;
    /// Typed input for this action.
    type Input: Send;
    /// Typed output for this action.
    type Output: Send;

    /// Unique identifier for this action (e.g. "detect-regex").
    fn id(&self) -> &str;

    /// Validate parameters and construct a configured action instance.
    ///
    /// This is where parameter validation, regex compilation, automata
    /// building, and other setup work happens.
    async fn connect(params: Self::Params) -> Result<Self, Error>;

    /// Execute the action with typed input, returning typed output.
    async fn execute(&self, input: Self::Input) -> Result<Self::Output, Error>;
}
