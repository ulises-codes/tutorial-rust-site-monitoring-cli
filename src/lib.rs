use clap::{arg, Parser};
use log::{debug, error, info};
use reqwest::StatusCode;
use serde::Serialize;

/// Our CLI args
#[derive(Parser, Debug)]
pub struct Args {
    /// URL for a sitemap containing a list of paths to ping
    #[arg(short, long)]
    pub sitemap: Vec<String>,

    /// URL where all results will be sent via POST request
    #[arg(short = 'w', long)]
    pub notification_url: String,

    /// URL where errors will be sent via POST request
    #[arg(short = 'c', long)]
    pub critical_url: String,

    /// URL(s) to ignore
    #[arg(short, long)]
    pub ignore: Option<Vec<String>>,
}

/// Identifies whether a sitemap was successfully retrieved and parsed
#[derive(Serialize, Debug, Clone)]
pub enum SiteCheckStatus {
    Success,
    /// Used when we are unable to fetch the given sitemap
    UnreachableSitemap,
    /// Used when the sitemap cannot be parsed
    InvalidSitemap,
}

/// Result of all listed sites within a sitemap
#[derive(Serialize, Debug, Clone)]
pub struct SiteCheckResult {
    status: SiteCheckStatus,
    sitemap_url: String,
    /// Number of pages that were tested
    page_total: Option<usize>,
    /// Total number of pages that could not be reached
    unreachable_total: Option<usize>,
    /// Comma-separated list of unreachable pages
    pub unreachable_pages: Option<String>,
}

/// Gets a sitemap and pings each URL
pub async fn handle_sitemap(
    sitemap_url: &str,
    urls_to_ignore: Option<&Vec<String>>,
) -> SiteCheckResult {
    let mut all_urls: Vec<String> = Vec::new();
    let mut unreachable_pages: Vec<String> = Vec::new();

    info!("Getting sitemap for {}", &sitemap_url);
    let sitemap_res = reqwest::get(sitemap_url).await;

    if sitemap_res.is_err() {
        return SiteCheckResult {
            status: SiteCheckStatus::UnreachableSitemap,
            sitemap_url: sitemap_url.to_owned(),
            page_total: None,
            unreachable_pages: None,
            unreachable_total: None,
        };
    }

    let sitemap_content = sitemap_res.unwrap().text().await;

    if sitemap_content.is_err() {
        return SiteCheckResult {
            status: SiteCheckStatus::InvalidSitemap,
            sitemap_url: sitemap_url.to_owned(),
            page_total: None,
            unreachable_pages: None,
            unreachable_total: None,
        };
    }

    let sitemap_content = sitemap_content.unwrap();

    let sitemap = roxmltree::Document::parse(&sitemap_content);

    if sitemap.is_err() {
        return SiteCheckResult {
            status: SiteCheckStatus::InvalidSitemap,
            sitemap_url: sitemap_url.to_owned(),
            page_total: None,
            unreachable_pages: None,
            unreachable_total: None,
        };
    }

    sitemap
        .unwrap()
        .descendants()
        .filter(|n| n.tag_name().name() == "loc" && n.children().next().is_some())
        .for_each(|n| all_urls.push(n.children().next().unwrap().text().unwrap().to_owned()));

    info!("Pinging {0} urls for sitemap {sitemap_url}", all_urls.len());
    for (i, url) in all_urls.iter().enumerate() {
        if let Some(ignored_urls) = urls_to_ignore {
            if ignored_urls.contains(url) {
                debug!("Ignoring url {url}");
                continue;
            }
        }

        debug!("{i} {sitemap_url} Pinging site {url}");
        let res = reqwest::get(url).await;

        if res.is_err() {
            error!("Page {url} is unreachable");
            unreachable_pages.push(url.to_owned());
            continue;
        }

        let res = res.unwrap();

        if res.status() == reqwest::StatusCode::OK {
            debug!("Response was successful");
        } else {
            error!("Response was {}", res.status());
            unreachable_pages.push(url.to_owned());
        }
    }

    SiteCheckResult {
        status: SiteCheckStatus::Success,
        sitemap_url: sitemap_url.to_owned(),
        page_total: Some(all_urls.len()),
        unreachable_total: Some(unreachable_pages.len()),
        unreachable_pages: if unreachable_pages.is_empty() {
            None
        } else {
            Some(unreachable_pages.join(", "))
        },
    }
}

/// Notify an endpoint via POST request
///
/// See [SiteCheckResult] to see the JSON body that will be sent
///
/// ## Arguments
/// * site_check_result - [SiteCheckResult]
/// * url - An endpoint to post to
pub async fn post_results(site_check_result: SiteCheckResult, url: &str) {
    debug!("{:?}", site_check_result);

    let client = reqwest::Client::new();

    info!("Posting result for {}", &site_check_result.sitemap_url);
    info!("Result is: {:?}", &site_check_result);
    let res = client
        .post(url)
        .json(&site_check_result)
        .send()
        .await
        .expect("Error posting result");

    if res.status() != StatusCode::OK {
        error!("Error posting result");

        error!("{:?}", res.error_for_status());
    }
}
