use clap::Parser;
use lib::{handle_sitemap, post_results, Args};
use std::sync::Arc;
use tokio::task::JoinSet;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let args = Args::parse();
    let mut handles = JoinSet::new();

    let urls_to_ignore = Arc::new(args.ignore);

    for sitemap_url in args.sitemap {
        let ignored_urls = Arc::clone(&urls_to_ignore);

        handles.spawn(async move {
            let urls = Arc::clone(&ignored_urls);

            handle_sitemap(&sitemap_url, urls.as_ref().as_ref()).await
        });
    }

    while let Some(res) = handles.join_next().await {
        if let Ok(site_check_result) = res {
            post_results(site_check_result.clone(), &args.notification_url).await;

            if site_check_result.clone().unreachable_pages.is_some() {
                post_results(site_check_result, &args.critical_url).await;
            }
        }
    }
}
