use anyhow::Result;
use serde::de::DeserializeOwned;

/// HTTP client for the RustChain API.
pub struct NodeClient {
    base_url: String,
    client: reqwest::Client,
}

#[allow(dead_code)]
impl NodeClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.get(&url).send().await?;
        let body = resp.json::<T>().await?;
        Ok(body)
    }

    pub async fn post<B: serde::Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self.client.post(&url).json(body).send().await?;
        let result = resp.json::<T>().await?;
        Ok(result)
    }

    pub async fn get_balance(&self, address: &str) -> Result<serde_json::Value> {
        self.get(&format!("/accounts/{}/balance", address)).await
    }

    pub async fn get_account(&self, address: &str) -> Result<serde_json::Value> {
        self.get(&format!("/accounts/{}", address)).await
    }

    pub async fn get_chain_info(&self) -> Result<serde_json::Value> {
        self.get("/chain/info").await
    }

    pub async fn get_block(&self, id: &str) -> Result<serde_json::Value> {
        self.get(&format!("/blocks/{}", id)).await
    }

    pub async fn get_latest_block(&self) -> Result<serde_json::Value> {
        self.get("/blocks/latest").await
    }

    pub async fn submit_transaction(&self, body: &serde_json::Value) -> Result<serde_json::Value> {
        self.post("/tx", body).await
    }
}
