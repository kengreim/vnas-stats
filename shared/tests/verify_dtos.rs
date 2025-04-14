use chrono::{DateTime, Utc};
use shared::vnas::api::ALL_ARTCCS_ENDPOINT;
use shared::vnas::api::ArtccRoot;
use shared::vnas::datafeed::{DatafeedRoot, VnasEnvironment, datafeed_url};

#[tokio::test]
async fn verify_api_dtos() -> Result<(), reqwest::Error> {
    let res = reqwest::get(ALL_ARTCCS_ENDPOINT)
        .await?
        .json::<Vec<ArtccRoot>>()
        .await?;
    assert_eq!(res.len(), 24);
    Ok(())
}

#[tokio::test]
async fn verify_datafeed_dtos() -> Result<(), reqwest::Error> {
    let url = datafeed_url(VnasEnvironment::Live);
    let res = reqwest::get(url).await?.json::<DatafeedRoot>().await?;
    assert!(res.updated_at > DateTime::<Utc>::MIN_UTC);
    Ok(())
}
