use std::collections::HashSet;
use scraper::selector::Selector;
use scraper::Html;
use reqwest::{Url, StatusCode};
use std::time::Instant;
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Arguments {
    /// The URL on which to perform the check.
    url: String
}

/// Normalize an URL by rebuilding a valid addressable link.
fn normalize_url(url: &str, path: &str) -> Option<String> {
    // If the href attribute is an anchor, we want to ignore it
    if path.starts_with("#") {
        return None
    }

    let base_url = Url::parse(url).unwrap();

    return match Url::parse(path) {
        Ok(href) => if href.has_host() && href.host() == base_url.host() { Some(href.to_string()) } else { None },
        Err(_) => {
            // If the path is relative, we can simply return the concatenation
            if path.starts_with("/") {
                return Some(format!("{}://{}{}", base_url.scheme(), base_url.domain().unwrap(), path))
            }
            Some(format!("{}://{}/{}", base_url.scheme(), base_url.domain().unwrap(), path))
        }
    };
}

/// Send a request to the URL provided in params and return true if the
/// request status code is 200
async fn check_url(url: &str) -> Result<(StatusCode, bool, String), reqwest::Error> {
    let response = reqwest::get(url).await?;
    Ok((
        response.status(),
        response.status() == 200,
        response.text().await?
    ))
}

/// Parse a raw HTML and returns a HashSet of links.
fn get_links_from_raw_html(url: &str, html: &str) -> HashSet<String> {
    let document = Html::parse_document(&html);
    let selector = Selector::parse("a[href]").unwrap();

    document
        .select(&selector)
        .filter_map(|el| normalize_url(url, el.value().attr("href").unwrap()))
        .collect::<HashSet<String>>()
}

/// Format and ensure the URL provided by the user is valid
fn format_url(url: &str) -> String {
    let parsed = Url::parse(url)
        .map_err(|_| {
            println!("😥 Oh no ! '{}' is not a valid URL. Please check that the URL is valid an retry.", &url);
            std::process::exit(1);
        })
        .unwrap();

    format!(
        "{}://{}/",
        parsed.scheme(),
        parsed.domain().unwrap()
    )
}

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let args = Arguments::parse();
    let url = format_url(&args.url);

    println!("🚀 Fuze starting analysis of {}", &url);

    let mut visited = HashSet::<String>::new();
    let mut broken_link = HashSet::<String>::new();
    let mut to_visit = HashSet::<String>::new();

    to_visit.insert(url);

    let start_time = Instant::now();

    while !to_visit.is_empty() {
        for url in to_visit.clone().drain() {
            let (status, is_ok, html) = check_url(&url).await?;

            visited.insert(url.to_string());

            let links = get_links_from_raw_html(&url, &html)
                .difference(&visited)
                .map(|el| el.to_string())
                .collect::<HashSet<String>>();

            if !is_ok {
                println!("❌ {} [{}]", &url, &status);
                broken_link.insert(url.clone());
            } else {
                println!("✅ {} [{}]", &url, &status);
                if !links.is_empty() {
                    println!("➡️ {} link(s) reconciled.", &links.len());
                }
            }

            to_visit = links;
        }
    }

    println!("👻 Done ! Fuze visited {} links in {:?}.", &visited.len(), &start_time.elapsed());

    if broken_link.len() > 0 {
        println!("Found {} broken links !", broken_link.len());
        broken_link.iter().for_each(|link| println!("❌ {}", link));
    } else {
        println!("No broken link detected !");
    }

    Ok(())
}
