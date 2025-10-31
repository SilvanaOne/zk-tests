use serde::Deserialize;
use tracing::{error, info};

const NEWS_API_BASE_URL: &str = "https://newsapi.org/v2/everything";

/// Source information for a news article
#[derive(Debug, Deserialize, Clone)]
pub struct Source {
    #[allow(dead_code)]
    pub id: Option<String>,
    pub name: String,
}

/// News article from NewsAPI
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Article {
    pub source: Source,
    pub author: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub url: String,
    #[allow(dead_code)]
    pub url_to_image: Option<String>,
    pub published_at: String,
    pub content: Option<String>,
}

/// Response from NewsAPI
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsResponse {
    #[allow(dead_code)]
    pub status: String,
    pub total_results: i32,
    pub articles: Vec<Article>,
}

/// Client for fetching news from NewsAPI
pub struct NewsApiClient {
    api_key: String,
    client: reqwest::Client,
}

impl NewsApiClient {
    /// Create a new NewsAPI client
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::builder()
                .user_agent("Binance-Prices-CLI/1.0")
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    /// Fetch news articles for a specific cryptocurrency from the last 48 hours
    pub async fn get_news(
        &self,
        query: &str,
    ) -> Result<NewsResponse, Box<dyn std::error::Error>> {
        info!("Fetching news for: {}", query);

        // Calculate 48 hours ago in RFC3339 format
        let from_time = chrono::Utc::now() - chrono::Duration::try_hours(48).unwrap();
        let from_param = from_time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        let url = format!(
            "{}?q={}&from={}&sortBy=publishedAt&apiKey={}",
            NEWS_API_BASE_URL, query, from_param, self.api_key
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            error!("NewsAPI error: {} - {}", status, error_text);
            return Err(format!("NewsAPI returned error: {} - {}", status, error_text).into());
        }

        let news_response: NewsResponse = response.json().await?;
        info!(
            "Successfully fetched {} articles for {}",
            news_response.articles.len(),
            query
        );

        Ok(news_response)
    }
}
