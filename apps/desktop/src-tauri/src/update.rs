use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    body: Option<String>,
    draft: bool,
    prerelease: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current_version: String,
    latest_version: String,
    release_url: String,
    notes: String,
    available: bool,
}

fn release_info(current: &Version, release: Release) -> Result<UpdateInfo, String> {
    let latest = Version::parse(
        release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name),
    )
    .map_err(|_| "发布版本号格式不正确".to_owned())?;
    if release.draft || release.prerelease || !latest.pre.is_empty() {
        return Err("暂无正式发布版本".to_owned());
    }
    // Construct the destination from the validated version, never from release text.
    let mut url =
        reqwest::Url::parse("https://github.com/shadow7-cn/tong-net/releases/tag/").unwrap();
    url.path_segments_mut()
        .unwrap()
        .pop_if_empty()
        .push(&release.tag_name);
    Ok(UpdateInfo {
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        release_url: url.to_string(),
        notes: release.body.unwrap_or_default(),
        available: latest.cmp_precedence(current).is_gt(),
    })
}

#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("tong-net/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|_| "无法创建更新检查请求".to_owned())?;
    let response = client
        .get("https://api.github.com/repos/shadow7-cn/tong-net/releases/latest")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| "无法连接 GitHub，请检查网络后重试".to_owned())?;
    match response.status().as_u16() {
        200 => {}
        404 => return Err("暂无正式发布版本".to_owned()),
        403 | 429 => return Err("GitHub 请求受限，请稍后重试".to_owned()),
        _ => return Err("GitHub 暂时不可用，请稍后重试".to_owned()),
    }
    let release = response
        .json::<Release>()
        .await
        .map_err(|_| "无法读取 GitHub 发布信息".to_owned())?;
    release_info(&app.package_info().version, release)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(current: &str, tag: &str) -> Result<UpdateInfo, String> {
        release_info(
            &Version::parse(current).unwrap(),
            Release {
                tag_name: tag.into(),
                body: None,
                draft: false,
                prerelease: false,
            },
        )
    }

    #[test]
    fn compares_numeric_versions_and_prerelease_correctly() {
        assert!(info("0.2.2", "v0.2.10").unwrap().available);
        assert!(!info("0.2.10", "v0.2.2").unwrap().available);
        assert!(!info("0.2.2", "v0.2.2").unwrap().available);
        assert!(!info("0.2.2+local", "v0.2.2+release").unwrap().available);
        assert!(info("0.3.0-beta.1", "v0.3.0").unwrap().available);
        assert!(info("0.2.2", "v0.3.0-beta.1").is_err());
        assert!(info("0.2.2", "not-a-version").is_err());
    }

    #[test]
    fn release_url_is_pinned_and_missing_notes_are_allowed() {
        let result = info("0.2.2", "v0.3.0").unwrap();
        assert_eq!(
            result.release_url,
            "https://github.com/shadow7-cn/tong-net/releases/tag/v0.3.0"
        );
        assert_eq!(result.notes, "");
        for (draft, prerelease) in [(true, false), (false, true)] {
            assert!(release_info(
                &Version::new(0, 2, 2),
                Release {
                    tag_name: "v0.3.0".into(),
                    body: None,
                    draft,
                    prerelease,
                }
            )
            .is_err());
        }
    }
}
