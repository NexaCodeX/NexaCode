use reqwest::Client;

pub struct BaseProvider {
    pub client: Client,
}

impl BaseProvider {
    pub fn new() -> Result<Self, anyhow::Error> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()?;
        Ok(Self { client })
    }
}

impl Default for BaseProvider {
    fn default() -> Self {
        Self::new().expect("Failed to create HTTP client")
    }
}
