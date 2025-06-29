use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Index {
    manifests: Vec<PlatformManifest>,
}

#[derive(Deserialize, Debug)]
struct PlatformManifest {
    digest: String,
    platform: Option<Platform>,
}

#[derive(Deserialize, Debug)]
struct Platform {
    architecture: String,
    os: String,
}

#[derive(Deserialize, Debug)]
struct ImageManifest {
    layers: Vec<Layer>,
}

#[derive(Deserialize, Debug)]
struct Layer {/* mediaType, digest, size… */}

pub async fn get_image_info(image_source: &str) -> anyhow::Result<(usize, String)> {
    // Parse image_source to extract repository and tag
    let (repository, tag) = if let Some(colon_pos) = image_source.rfind(':') {
        let repo = &image_source[..colon_pos];
        let tag = &image_source[colon_pos + 1..];
        (repo, tag)
    } else {
        (image_source, "latest")
    };

    let client = Client::new();
    // 1. Get token…
    let token: String = client
        .get("https://auth.docker.io/token")
        .query(&[
            ("service", "registry.docker.io"),
            ("scope", &format!("repository:{}:pull", repository)),
        ])
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?["token"]
        .as_str()
        .unwrap()
        .to_string();

    // 2. Fetch manifest/index
    let manifest_response = client
        .get(format!(
            "https://registry-1.docker.io/v2/{}/manifests/{}",
            repository, tag
        ))
        .bearer_auth(token.as_str())
        .header(
            "Accept",
            "application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.v2+json",
        )
        .send()
        .await?;

    println!("Index: {:?}", manifest_response);

    let content_type = manifest_response
        .headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("");

    // Extract the digest from the response headers
    let digest = manifest_response
        .headers()
        .get("docker-content-digest")
        .and_then(|digest| digest.to_str().ok())
        .unwrap_or("")
        .to_string();

    let manifest: ImageManifest = if content_type.contains("index") || content_type.contains("list")
    {
        // This is a manifest index (multi-platform)
        let idx: Index = manifest_response.json().await?;

        // pick amd64/linux manifest
        let platform_digest = idx
            .manifests
            .iter()
            .find(|m| {
                m.platform
                    .as_ref()
                    .map(|p| p.architecture == "amd64" && p.os == "linux")
                    .unwrap_or(false)
            })
            .unwrap()
            .digest
            .clone();

        // 3. Fetch the actual manifest
        client
            .get(format!(
                "https://registry-1.docker.io/v2/{}/manifests/{}",
                repository, platform_digest
            ))
            .bearer_auth(token.as_str())
            .header(
                "Accept",
                "application/vnd.docker.distribution.manifest.v2+json",
            )
            .send()
            .await?
            .json()
            .await?
    } else {
        // This is already a direct manifest (single platform)
        manifest_response.json().await?
    };

    println!("Manifest: {:?}", manifest);

    let layers_number = manifest.layers.len();
    println!("Number of layers: {}", layers_number);
    Ok((layers_number, digest))
}
