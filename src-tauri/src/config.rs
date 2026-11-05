use toml_edit::{value, DocumentMut, Item, Table};

pub const MODEL: &str = "gpt-6-astra";
pub const ENDPOINT: &str = env!("HAC_BASE_URL");
pub const ENV_KEY: &str = "HAC_API_KEY";

fn parse(input: &str) -> Result<DocumentMut, String> {
    input.parse().map_err(|_| {
        "TOML không hợp lệ. Sửa file cấu hình rồi thử lại; chưa thay đổi dữ liệu.".into()
    })
}

fn table<'a>(parent: &'a mut Table, name: &str) -> Result<&'a mut Table, String> {
    if !parent.contains_key(name) {
        parent.insert(name, Item::Table(Table::new()));
    }
    parent
        .get_mut(name)
        .and_then(Item::as_table_mut)
        .ok_or_else(|| {
            format!("Mục {name} phải là bảng TOML. Hãy chỉnh cấu hình trước khi tiếp tục.")
        })
}

pub fn render(input: &str, profile: bool) -> Result<String, String> {
    render_with_effort(input, profile, "high")
}

pub(crate) fn render_with_effort(
    input: &str,
    profile: bool,
    effort: &str,
) -> Result<String, String> {
    let mut doc = parse(input)?;
    doc["model"] = value(MODEL);
    doc["model_provider"] = value("hac");
    doc["model_reasoning_effort"] = value(effort);
    if !profile {
        doc["forced_login_method"] = value("api");
        // A previously selected default profile could otherwise override these root values.
        doc.remove("profile");
        let providers = table(doc.as_table_mut(), "model_providers")?;
        // Own this provider completely: stale auth/header fields must not override env_key.
        let mut provider = Table::new();
        for (key, val) in [
            ("name", "HAC"),
            ("base_url", ENDPOINT),
            ("env_key", ENV_KEY),
            ("wire_api", "responses"),
        ] {
            provider[key] = value(val);
        }
        provider["requires_openai_auth"] = value(false);
        provider["supports_websockets"] = value(false);
        providers.insert("hac", Item::Table(provider));
        let memories = table(doc.as_table_mut(), "memories")?;
        memories["generate_memories"] = value(true);
        memories["use_memories"] = value(true);
    }
    Ok(doc.to_string())
}

pub fn matches(input: &str, profile: bool) -> bool {
    let Ok(doc) = parse(input) else {
        return false;
    };
    let root_ok = doc.get("model").and_then(Item::as_str) == Some(MODEL)
        && doc.get("model_provider").and_then(Item::as_str) == Some("hac")
        && doc.get("model_reasoning_effort").and_then(Item::as_str) == Some("high");
    if profile {
        return root_ok;
    }
    if !root_ok
        || doc.contains_key("profile")
        || doc.get("forced_login_method").and_then(Item::as_str) != Some("api")
    {
        return false;
    }
    let Some(p) = doc.get("model_providers").and_then(|p| p.get("hac")) else {
        return false;
    };
    [
        ("name", "HAC"),
        ("base_url", ENDPOINT),
        ("env_key", ENV_KEY),
        ("wire_api", "responses"),
    ]
    .iter()
    .all(|(k, v)| p.get(k).and_then(Item::as_str) == Some(*v))
        && ["requires_openai_auth", "supports_websockets"]
            .iter()
            .all(|k| p.get(k).and_then(Item::as_bool) == Some(false))
        && [
            "auth",
            "experimental_bearer_token",
            "http_headers",
            "env_http_headers",
        ]
        .iter()
        .all(|k| p.get(k).is_none())
        && ["generate_memories", "use_memories"].iter().all(|k| {
            doc.get("memories")
                .and_then(|m| m.get(k))
                .and_then(Item::as_bool)
                == Some(true)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn base_config_uses_hac_without_profile_flag_and_matches_high_profile() {
        let base: DocumentMut = render("", false).unwrap().parse().unwrap();
        let profile: DocumentMut = render("", true).unwrap().parse().unwrap();
        assert_eq!(base["model_provider"].as_str(), Some("hac"));
        assert_eq!(base["model_reasoning_effort"].as_str(), Some("high"));
        assert_eq!(
            base["model_reasoning_effort"].as_str(),
            profile["model_reasoning_effort"].as_str()
        );
        assert!(!base.contains_key("profile"));
    }
    #[test]
    fn preserves_unrelated_settings_and_comments() {
        let input = "# user settings\napproval_policy = 'on-request'\nprofile = 'old'\n[projects.'/tmp/work']\ntrust_level = 'trusted'\n[model_providers.other]\nname = 'Other'\n[memories]\nmax_raw_memories_for_consolidation = 50\n";
        let rendered = render(input, false).unwrap();
        assert!(matches(&rendered, false));
        assert!(rendered.contains("# user settings"));
        assert!(rendered.contains("approval_policy = 'on-request'"));
        assert!(rendered.contains("trust_level = 'trusted'"));
        assert!(rendered.contains("name = 'Other'"));
        assert!(rendered.contains("max_raw_memories_for_consolidation = 50"));
        assert_eq!(render(&rendered, false).unwrap(), rendered);
    }
    #[test]
    fn replaces_conflicting_hac_auth_and_generates_profile() {
        let output = render(
            "[model_providers.hac]\nexperimental_bearer_token = 'old-secret'\n",
            false,
        )
        .unwrap();
        assert!(!output.contains("old-secret"));
        assert!(matches(&output, false));
        let profile = render("", true).unwrap();
        assert!(matches(&profile, true));
        assert!(!profile.contains("env_key"));
    }
    #[test]
    fn invalid_config_is_rejected_without_echoing_contents() {
        let error = render("SECRET bad toml = [", false).unwrap_err();
        assert!(!error.contains("SECRET"));
        assert!(render("memories = 12", false).is_err());
        assert!(render("model_providers = 'bad'", false).is_err());
    }
}
