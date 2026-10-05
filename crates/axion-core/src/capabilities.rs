use crate::{CapabilityConfig, CapabilityProfileConfig};

pub const CAPABILITY_PROFILES: &[&str] = &[
    "minimal",
    "app-info",
    "app-control",
    "app-events",
    "window-control",
    "multi-window",
    "clipboard-access",
    "shell-access",
    "file-access",
    "dialog-access",
];

pub fn profile_commands(profile: &str) -> &'static [&'static str] {
    match profile {
        "app-info" => &["app.ping", "app.info", "app.version", "app.echo"],
        "app-control" => &["app.exit"],
        "window-control" => &[
            "window.info",
            "window.reload",
            "window.focus",
            "window.set_title",
            "window.set_size",
            "window.show",
            "window.hide",
            "window.close",
            "window.confirm_close",
            "window.prevent_close",
        ],
        "multi-window" => &[
            "window.list",
            "window.info",
            "window.reload",
            "window.focus",
            "window.set_title",
            "window.close",
            "window.confirm_close",
            "window.prevent_close",
        ],
        "clipboard-access" => &["clipboard.read_text", "clipboard.write_text"],
        "shell-access" => &["shell.open"],
        "file-access" => &[
            "fs.create_dir",
            "fs.exists",
            "fs.list_dir",
            "fs.read_text",
            "fs.remove",
            "fs.write_text",
        ],
        "dialog-access" => &["dialog.open", "dialog.save"],
        _ => &[],
    }
}

pub fn profile_events(profile: &str) -> &'static [&'static str] {
    match profile {
        "app-events" => &["app.log"],
        _ => &[],
    }
}

pub fn profile_protocols(profile: &str) -> &'static [&'static str] {
    if is_known_capability_profile(profile) {
        &["axion"]
    } else {
        &[]
    }
}

pub fn is_known_capability_profile(value: &str) -> bool {
    CAPABILITY_PROFILES.contains(&value)
}

fn normalized(values: &[String], valid: impl Fn(&str) -> bool) -> Result<Vec<String>, String> {
    let mut values = values
        .iter()
        .map(|value| value.trim().to_owned())
        .collect::<Vec<_>>();
    if let Some(value) = values.iter().find(|value| !valid(value)) {
        return Err(format!("invalid capability value '{value}'"));
    }
    values.sort();
    values.dedup();
    Ok(values)
}
pub fn is_valid_capability_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split('.').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        })
}
pub fn is_valid_protocol_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
pub fn profile_expansions(profiles: &[String]) -> Vec<CapabilityProfileConfig> {
    profiles
        .iter()
        .map(|profile| {
            let convert = |values: &[&str]| {
                let mut values = values.iter().map(|v| (*v).to_owned()).collect::<Vec<_>>();
                values.sort();
                values.dedup();
                values
            };
            CapabilityProfileConfig {
                profile: profile.clone(),
                commands: convert(profile_commands(profile)),
                events: convert(profile_events(profile)),
                protocols: convert(profile_protocols(profile)),
            }
        })
        .collect()
}

pub fn resolve_capability(config: &CapabilityConfig) -> Result<CapabilityConfig, String> {
    let mut result = config.clone();
    result.profiles = normalized(&config.profiles, is_known_capability_profile)?;
    let cached = !config.profile_expansions.is_empty();
    result.profile_expansions = profile_expansions(&result.profiles);
    if cached && config.profile_expansions != result.profile_expansions {
        return Err("capability profile expansion does not match its declarations".to_owned());
    }
    let declared = |explicit: &[String], effective: &[String]| {
        if explicit.is_empty() && !cached {
            effective.to_vec()
        } else {
            explicit.to_vec()
        }
    };
    result.explicit_commands = normalized(
        &declared(&config.explicit_commands, &config.commands),
        is_valid_capability_name,
    )?;
    result.explicit_events = normalized(
        &declared(&config.explicit_events, &config.events),
        is_valid_capability_name,
    )?;
    result.explicit_protocols = normalized(
        &declared(&config.explicit_protocols, &config.protocols),
        is_valid_protocol_name,
    )?;
    let mut commands = result.explicit_commands.clone();
    let mut events = result.explicit_events.clone();
    let mut protocols = result.explicit_protocols.clone();
    for profile in &result.profile_expansions {
        commands.extend(profile.commands.clone());
        events.extend(profile.events.clone());
        protocols.extend(profile.protocols.clone());
    }
    result.commands = normalized(&commands, is_valid_capability_name)?;
    result.events = normalized(&events, is_valid_capability_name)?;
    result.protocols = normalized(&protocols, is_valid_protocol_name)?;
    if cached
        && (config.commands != result.commands
            || config.events != result.events
            || config.protocols != result.protocols)
    {
        return Err("effective capabilities do not match their declarations".to_owned());
    }
    if (!result.commands.is_empty() || !result.events.is_empty())
        && !result.protocols.iter().any(|p| p == "axion")
    {
        return Err("bridge commands and events require the axion protocol".to_owned());
    }
    let mut origins = Vec::new();
    for value in &config.allowed_navigation_origins {
        let url = url::Url::parse(value.trim())
            .map_err(|_| format!("invalid navigation origin '{value}'"))?;
        if url.host_str().is_none()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.cannot_be_a_base()
        {
            return Err(format!(
                "navigation origin must contain only scheme and authority: '{value}'"
            ));
        }
        let origin = format!("{}://{}", url.scheme(), url.host_str().unwrap())
            + &url
                .port()
                .map(|port| format!(":{port}"))
                .unwrap_or_default();
        origins.push(origin);
    }
    origins.sort();
    origins.dedup();
    result.allowed_navigation_origins = origins;
    Ok(result)
}
