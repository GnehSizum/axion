use std::collections::BTreeSet;

use axion_core::CapabilityConfig;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OriginTrust {
    Host,
    App,
    Remote,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilitySet {
    pub commands: BTreeSet<String>,
    pub events: BTreeSet<String>,
    pub protocols: BTreeSet<String>,
    pub allowed_navigation_origins: BTreeSet<String>,
    pub allow_remote_navigation: bool,
}

impl CapabilitySet {
    pub fn allows_command(&self, command: &str) -> bool {
        self.commands.contains(command)
    }

    pub fn allows_event(&self, event: &str) -> bool {
        self.events.contains(event)
    }

    pub fn allows_protocol(&self, protocol: &str) -> bool {
        self.protocols.contains(protocol)
    }

    pub fn command_names(&self) -> Vec<String> {
        self.commands.iter().cloned().collect()
    }

    pub fn event_names(&self) -> Vec<String> {
        self.events.iter().cloned().collect()
    }

    pub fn protocol_names(&self) -> Vec<String> {
        self.protocols.iter().cloned().collect()
    }

    pub fn navigation_origin_names(&self) -> Vec<String> {
        self.allowed_navigation_origins.iter().cloned().collect()
    }
}

impl From<&CapabilityConfig> for CapabilitySet {
    fn from(value: &CapabilityConfig) -> Self {
        Self {
            commands: value.commands.iter().cloned().collect(),
            events: value.events.iter().cloned().collect(),
            protocols: value.protocols.iter().cloned().collect(),
            allowed_navigation_origins: value.allowed_navigation_origins.iter().cloned().collect(),
            allow_remote_navigation: value.allow_remote_navigation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityPolicy {
    capabilities: CapabilitySet,
    app_origin: String,
    trusted_origins: BTreeSet<String>,
}

impl SecurityPolicy {
    pub fn new(
        capabilities: CapabilitySet,
        app_origin: impl Into<String>,
        trusted_origins: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let app_origin = app_origin.into();
        let mut trusted_origin_set = trusted_origins
            .into_iter()
            .map(Into::into)
            .collect::<BTreeSet<_>>();
        trusted_origin_set.insert(app_origin.clone());

        Self {
            capabilities,
            app_origin,
            trusted_origins: trusted_origin_set,
        }
    }

    pub fn from_capabilities<'a>(
        capabilities: impl IntoIterator<Item = &'a CapabilityConfig>,
        app_origin: impl Into<String>,
        trusted_origins: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        let mut merged = CapabilitySet::default();
        for capability in capabilities {
            let capability = CapabilitySet::from(capability);
            merged.commands.extend(capability.commands);
            merged.events.extend(capability.events);
            merged.protocols.extend(capability.protocols);
            merged
                .allowed_navigation_origins
                .extend(capability.allowed_navigation_origins);
            merged.allow_remote_navigation |= capability.allow_remote_navigation;
        }

        Self::new(merged, app_origin, trusted_origins)
    }

    pub fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    pub fn app_origin(&self) -> &str {
        &self.app_origin
    }

    pub fn trusted_origins(&self) -> Vec<String> {
        self.trusted_origins.iter().cloned().collect()
    }

    pub fn allows_command(&self, command: &str) -> bool {
        self.capabilities.allows_command(command)
    }

    pub fn allows_event(&self, event: &str) -> bool {
        self.capabilities.allows_event(event)
    }

    pub fn allows_protocol(&self, protocol: &str) -> bool {
        self.capabilities.allows_protocol(protocol)
    }

    pub fn allows_navigation(&self, url: &Url) -> bool {
        self.capabilities.allow_remote_navigation
            || self.is_trusted_origin(&Self::origin_string(url))
            || self.allows_navigation_origin(&Self::origin_string(url))
    }

    pub fn is_trusted_origin(&self, origin: &str) -> bool {
        self.trusted_origins.contains(origin)
    }

    pub fn allows_navigation_origin(&self, origin: &str) -> bool {
        self.capabilities
            .allowed_navigation_origins
            .contains(origin)
    }

    pub fn content_security_policy(&self) -> String {
        let mut connect_sources = BTreeSet::from(["'self'".to_owned(), self.app_origin.clone()]);
        connect_sources.extend(self.trusted_origins.iter().cloned());
        connect_sources.extend(self.capabilities.allowed_navigation_origins.iter().cloned());

        let style_sources = ["'self'".to_owned(), self.app_origin.clone()].join(" ");

        format!(
            "default-src 'self'; base-uri 'none'; object-src 'none'; frame-ancestors 'none'; script-src 'self' {}; style-src {}; img-src 'self' data:; font-src 'self'; connect-src {}",
            self.app_origin,
            style_sources,
            connect_sources.into_iter().collect::<Vec<_>>().join(" ")
        )
    }

    pub fn matches_any_trusted_origin<'a>(
        &self,
        origins: impl IntoIterator<Item = &'a str>,
    ) -> bool {
        origins
            .into_iter()
            .any(|origin| self.is_trusted_origin(origin))
    }

    pub fn trust_for_origin(&self, origin: &str) -> OriginTrust {
        if origin == self.app_origin {
            OriginTrust::App
        } else if self.trusted_origins.contains(origin) {
            OriginTrust::Host
        } else {
            OriginTrust::Remote
        }
    }

    pub fn origin_string(url: &Url) -> String {
        format!("{}://{}", url.scheme(), url.host_str().unwrap_or_default())
            + &url
                .port()
                .map(|port| format!(":{port}"))
                .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use axion_core::CapabilityConfig;
    use url::Url;

    use super::{OriginTrust, SecurityPolicy};

    fn capability(
        commands: &[&str],
        events: &[&str],
        protocols: &[&str],
        allowed_navigation_origins: &[&str],
        allow_remote_navigation: bool,
    ) -> CapabilityConfig {
        CapabilityConfig {
            profiles: Vec::new(),
            commands: commands.iter().map(|item| (*item).to_owned()).collect(),
            events: events.iter().map(|item| (*item).to_owned()).collect(),
            protocols: protocols.iter().map(|item| (*item).to_owned()).collect(),
            allowed_navigation_origins: allowed_navigation_origins
                .iter()
                .map(|item| (*item).to_owned())
                .collect(),
            allow_remote_navigation,
            ..Default::default()
        }
    }

    #[test]
    fn policy_merges_capabilities_and_trusted_origins() {
        let policy = SecurityPolicy::from_capabilities(
            [
                &capability(
                    &["app.ping"],
                    &["app.log"],
                    &["axion"],
                    &["https://docs.example"],
                    false,
                ),
                &capability(&["app.echo"], &["plugin.event"], &["asset"], &[], true),
            ],
            "axion://app",
            ["http://127.0.0.1:3000"],
        );

        assert!(policy.allows_command("app.ping"));
        assert!(policy.allows_command("app.echo"));
        assert!(policy.allows_event("app.log"));
        assert!(policy.allows_event("plugin.event"));
        assert!(policy.allows_protocol("axion"));
        assert!(policy.allows_protocol("asset"));
        assert!(policy.allows_navigation_origin("https://docs.example"));
        assert!(policy.capabilities().allow_remote_navigation);
        assert!(policy.is_trusted_origin("axion://app"));
        assert!(policy.is_trusted_origin("http://127.0.0.1:3000"));
    }

    #[test]
    fn policy_classifies_app_host_and_remote_origins() {
        let policy = SecurityPolicy::from_capabilities(
            [&capability(
                &["app.ping"],
                &["app.log"],
                &["axion"],
                &[],
                false,
            )],
            "axion://app",
            ["http://127.0.0.1:3000"],
        );

        assert_eq!(policy.trust_for_origin("axion://app"), OriginTrust::App);
        assert_eq!(
            policy.trust_for_origin("http://127.0.0.1:3000"),
            OriginTrust::Host
        );
        assert_eq!(
            policy.trust_for_origin("https://example.com"),
            OriginTrust::Remote
        );
    }

    #[test]
    fn policy_restricts_navigation_to_trusted_origins_by_default() {
        let policy = SecurityPolicy::from_capabilities(
            [&capability(
                &["app.ping"],
                &["app.log"],
                &["axion"],
                &["https://docs.example"],
                false,
            )],
            "axion://app",
            ["http://127.0.0.1:3000"],
        );

        assert!(policy.allows_navigation(&Url::parse("axion://app/index.html").unwrap()));
        assert!(policy.allows_navigation(&Url::parse("http://127.0.0.1:3000").unwrap()));
        assert!(policy.allows_navigation(&Url::parse("https://docs.example/guide").unwrap()));
        assert!(!policy.allows_navigation(&Url::parse("https://example.com").unwrap()));
    }

    #[test]
    fn policy_generates_strict_content_security_policy() {
        let policy = SecurityPolicy::from_capabilities(
            [&capability(
                &["app.ping"],
                &["app.log"],
                &["axion"],
                &["https://docs.example"],
                false,
            )],
            "axion://app",
            ["http://127.0.0.1:3000"],
        );
        let csp = policy.content_security_policy();

        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("base-uri 'none'"));
        assert!(csp.contains("object-src 'none'"));
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(csp.contains("script-src 'self' axion://app"));
        assert!(csp.contains("style-src 'self' axion://app"));
        assert!(csp.contains("connect-src"));
        assert!(csp.contains("http://127.0.0.1:3000"));
        assert!(csp.contains("https://docs.example"));
        assert!(!csp.contains("'unsafe-inline'"));
        assert!(!csp.contains("*"));
    }
}
