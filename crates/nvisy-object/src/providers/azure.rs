//! Azure Blob Storage provider using [`object_store::azure::MicrosoftAzureBuilder`].

use object_store::azure::MicrosoftAzureBuilder;
use serde::Deserialize;

use nvisy_core::Error;
use super::Provider;

use crate::client::ObjectStoreClient;

/// Typed credentials for Azure Blob Storage.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AzureCredentials {
    /// Azure storage container name.
    pub container: String,
    /// Azure storage account name.
    pub account_name: String,
    /// Storage account access key.
    #[serde(default)]
    pub access_key: Option<String>,
    /// Shared Access Signature token.
    #[serde(default)]
    pub sas_token: Option<String>,
    /// Custom endpoint URL (for Azure Stack or Azurite).
    #[serde(default)]
    pub endpoint: Option<String>,
}

/// Factory that creates [`ObjectStoreClient`] instances backed by Azure Blob Storage.
pub struct AzureProvider;

#[async_trait::async_trait]
impl Provider for AzureProvider {
    type Credentials = AzureCredentials;
    type Client = ObjectStoreClient;

    const ID: &str = "azure";

    async fn verify(creds: &Self::Credentials) -> Result<(), Error> {
        let client = Self::connect(creds).await?;
        client.verify_reachable().await
    }

    async fn connect(creds: &Self::Credentials) -> Result<Self::Client, Error> {
        let mut builder = MicrosoftAzureBuilder::new()
            .with_container_name(&creds.container)
            .with_account(&creds.account_name);

        if let Some(key) = &creds.access_key {
            builder = builder.with_access_key(key);
        }

        if let Some(sas) = &creds.sas_token {
            let pairs: Vec<(String, String)> = sas
                .trim_start_matches('?')
                .split('&')
                .filter_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    Some((parts.next()?.to_string(), parts.next().unwrap_or("").to_string()))
                })
                .collect();
            builder = builder.with_sas_authorization(pairs);
        }

        if let Some(endpoint) = &creds.endpoint {
            builder = builder.with_endpoint(endpoint.clone());
        }

        let store = builder
            .build()
            .map_err(|e| Error::connection(e.to_string(), "azure", true))?;

        Ok(ObjectStoreClient::new(store))
    }
}
