use std::fmt::{write, Display, Write};

use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, Utc};
use inquire::{
    ui::{Attributes, Color, RenderConfig, StyleSheet, Styled},
    Select,
};
use rusoto_core::Region;
use rusoto_ecr::{
    BatchGetImageRequest, BatchGetImageResponse, DescribeImagesRequest, DescribeImagesResponse,
    DescribeRepositoriesRequest, DescribeRepositoriesResponse, Ecr, EcrClient, ImageIdentifier,
    PutImageRequest,
};

#[derive(Debug)]
struct Respository {
    name: String,
}

impl From<&rusoto_ecr::Repository> for Respository {
    fn from(value: &rusoto_ecr::Repository) -> Self {
        Self {
            name: value.repository_name.clone().unwrap(),
        }
    }
}

impl Display for Respository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug)]
struct ImageDetail {
    pub tags: Vec<String>,
    pub digest: String,
    pub created: DateTime<Utc>,
    pub repository_name: String,
}

fn convert(timestamp: i64) -> DateTime<Utc> {
    let naive = NaiveDateTime::from_timestamp(timestamp, 0);
    DateTime::<Utc>::from_utc(naive, Utc)
}

impl From<&rusoto_ecr::ImageDetail> for ImageDetail {
    fn from(value: &rusoto_ecr::ImageDetail) -> Self {
        Self {
            tags: value.image_tags.clone().unwrap_or_default(),
            digest: value.image_digest.clone().unwrap(),
            created: convert(value.image_pushed_at.unwrap() as i64),
            repository_name: value.repository_name.clone().unwrap(),
        }
    }
}

impl Display for ImageDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.tags.len() {
            0 => write!(f, "{} - {}", self.created.to_rfc3339(), self.digest,),
            _ => write!(
                f,
                "{} - {} - {}",
                self.created.to_rfc3339(),
                self.digest,
                self.tags.join(", ")
            ),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let ecr_client = EcrClient::new(Region::default());
    let repositories: Vec<Respository> = fetch_repositories(&ecr_client)
        .await?
        .repositories
        .unwrap()
        .iter()
        .map(|r| r.into())
        .collect();
    let repository = Select::new("repository:", repositories)
        .with_page_size(10)
        .with_render_config(get_render_config())
        .prompt()?;

    let mut images: Vec<ImageDetail> = fetch_images(&ecr_client, repository.name)
        .await?
        .image_details
        .unwrap()
        .iter()
        .map(|r| r.into())
        .collect();
    images.sort_by_key(|img| img.created);
    images.reverse();

    let image_detail = Select::new("image:", images)
        .with_page_size(10)
        .with_render_config(get_render_config())
        .prompt()?;

    let image = get_image(&ecr_client, image_detail).await?;

    put_image(&ecr_client, image).await?;

    Ok(())
}

async fn fetch_repositories(client: &EcrClient) -> Result<DescribeRepositoriesResponse> {
    let request = DescribeRepositoriesRequest {
        ..Default::default()
    };
    let response = client.describe_repositories(request).await?;
    Ok(response)
}

async fn fetch_images(
    client: &EcrClient,
    repository_name: String,
) -> Result<DescribeImagesResponse> {
    let request = DescribeImagesRequest {
        repository_name,
        ..Default::default()
    };
    let response = client.describe_images(request).await?;
    Ok(response)
}

async fn get_image(client: &EcrClient, image_detail: ImageDetail) -> Result<rusoto_ecr::Image> {
    let request = BatchGetImageRequest {
        repository_name: image_detail.repository_name,
        image_ids: vec![ImageIdentifier {
            image_digest: Some(image_detail.digest),
            image_tag: None,
        }],
        ..Default::default()
    };
    let response = client.batch_get_image(request).await?;
    Ok(response.images.unwrap().get(0).unwrap().clone())
}

async fn put_image(client: &EcrClient, image: rusoto_ecr::Image) -> Result<()> {
    let request = PutImageRequest {
        repository_name: image.repository_name.unwrap(),
        image_tag: Some("latest".to_string()),
        image_manifest: image.image_manifest.unwrap(),
        ..Default::default()
    };
    client.put_image(request).await?;
    Ok(())
}

fn get_render_config() -> RenderConfig {
    let mut render_config = RenderConfig::default();
    render_config.prompt_prefix = Styled::new("$").with_fg(Color::LightRed);
    render_config.selected_checkbox = Styled::new("☑").with_fg(Color::LightGreen);
    render_config.scroll_up_prefix = Styled::new("⇞");
    render_config.scroll_down_prefix = Styled::new("⇟");

    render_config.answer = StyleSheet::new()
        .with_attr(Attributes::BOLD)
        .with_fg(Color::LightGreen);

    render_config.help_message = StyleSheet::new().with_fg(Color::DarkYellow);

    render_config
}
