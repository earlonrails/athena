use thirtyfour::prelude::*;
use thirtyfour::ChromeCapabilities;
use tracing::{debug, info};

pub struct BrowserAutomation {
    driver: Option<WebDriver>,
    capabilities: ChromeCapabilities,
}

impl Default for BrowserAutomation {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserAutomation {
    pub fn new() -> Self {
        let caps = DesiredCapabilities::chrome();
        Self {
            driver: None,
            capabilities: caps,
        }
    }

    pub async fn connect(&mut self, url: &str) -> Result<(), WebDriverError> {
        info!("Connecting to WebDriver at {}", url);
        let driver = WebDriver::new(url, self.capabilities.clone()).await?;
        self.driver = Some(driver);
        Ok(())
    }

    pub async fn goto(&self, url: &str) -> Result<(), WebDriverError> {
        if let Some(driver) = &self.driver {
            debug!("Navigating to {}", url);
            driver.goto(url).await?;
            Ok(())
        } else {
            Err(WebDriverError::RequestFailed("WebDriver not connected".into()))
        }
    }

    pub async fn get_text(&self) -> Result<String, WebDriverError> {
        if let Some(driver) = &self.driver {
            let body = driver.find(By::Tag("body")).await?;
            body.text().await
        } else {
            Err(WebDriverError::RequestFailed("WebDriver not connected".into()))
        }
    }

    pub async fn click(&self, css_selector: &str) -> Result<(), WebDriverError> {
        if let Some(driver) = &self.driver {
            debug!("Clicking element: {}", css_selector);
            let elem = driver.find(By::Css(css_selector)).await?;
            elem.click().await?;
            Ok(())
        } else {
            Err(WebDriverError::RequestFailed("WebDriver not connected".into()))
        }
    }
    
    pub async fn fill(&self, css_selector: &str, text: &str) -> Result<(), WebDriverError> {
        if let Some(driver) = &self.driver {
            debug!("Filling element {} with text", css_selector);
            let elem = driver.find(By::Css(css_selector)).await?;
            elem.send_keys(text).await?;
            Ok(())
        } else {
            Err(WebDriverError::RequestFailed("WebDriver not connected".into()))
        }
    }

    pub async fn close(&mut self) -> Result<(), WebDriverError> {
        if let Some(driver) = self.driver.take() {
            driver.quit().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_browser_automation_initial_state() {
        let mut browser = BrowserAutomation::new();
        assert!(browser.driver.is_none());

        // Test error handling when not connected
        assert!(browser.goto("https://example.com").await.is_err());
        assert!(browser.get_text().await.is_err());
        assert!(browser.click("#submit").await.is_err());
        assert!(browser.fill("#input", "hello").await.is_err());

        // Close on disconnected browser is a safe no-op
        assert!(browser.close().await.is_ok());
    }

    #[test]
    fn test_browser_automation_default() {
        let browser = BrowserAutomation::default();
        assert!(browser.driver.is_none());
    }
}

// Rust guideline compliant 2026-02-21
