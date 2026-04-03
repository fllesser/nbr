use crate::uv::CmdBuilder;
use anyhow::{Context, Result};
use regex::Regex;

#[derive(Debug, Clone)]
pub struct InstallSpec<'a> {
    pub name: &'a str,
    pub module_name: String,
    pub git_url: Option<&'a str>,
    pub extras: Option<Vec<&'a str>>,
    pub specifier: Option<&'a str>,
}

impl<'a> InstallSpec<'a> {
    pub fn parse(input: &'a str) -> Result<Self> {
        let (name, git_url, extras, specifier) = if input.starts_with("git+") {
            const GIT_URL_PATTERN: &str = r"nonebot-plugin-([^/.@]+)";
            let re = Regex::new(GIT_URL_PATTERN).context("Invalid regex pattern")?;
            let captures = re
                .captures(input)
                .with_context(|| format!("Invalid plugin name: {}", input))?;
            let name = captures
                .get(0)
                .map(|m| m.as_str())
                .context("Regex should have at least one capture group")?;
            (name, Some(input), None, None)
        } else {
            const PATTERN: &str = r"^([a-zA-Z0-9_-]+)(?:\[([a-zA-Z0-9_,\s]*)\])?(?:\s*((?:==|>=|<=|>|<|~=)\s*[a-zA-Z0-9\.]+))?$";
            let re = Regex::new(PATTERN).context("Invalid regex pattern")?;
            let captures = re
                .captures(input)
                .with_context(|| format!("Invalid plugin name: {}", input))?;
            let name = captures
                .get(1)
                .map(|m| m.as_str())
                .context("Regex should have at least one capture group")?;
            let extras = captures
                .get(2)
                .map(|m| m.as_str().split(',').collect::<Vec<&str>>());
            let specifier = captures.get(3).map(|m| m.as_str());
            (name, None, extras, specifier)
        };

        let module_name = name.replace("-", "_");

        Ok(Self {
            name,
            module_name,
            git_url,
            extras,
            specifier,
        })
    }
}

#[derive(Debug, Clone)]
pub struct InstallOptions<'a> {
    pub spec: InstallSpec<'a>,
    pub upgrade: bool,
    pub reinstall: bool,
    pub index_url: Option<&'a str>,
}

impl<'a> InstallOptions<'a> {
    pub fn new(
        input: &'a str,
        upgrade: bool,
        reinstall: bool,
        index_url: Option<&'a str>,
    ) -> Result<Self> {
        let spec = InstallSpec::parse(input)?;

        Ok(Self {
            spec,
            upgrade,
            reinstall,
            index_url,
        })
    }

    pub fn install(&self) -> Result<()> {
        let mut args = vec!["add"];

        if let Some(git_url) = self.spec.git_url {
            args.push(git_url);
        } else {
            args.push(self.spec.name);
        }

        if self.upgrade {
            args.push("--upgrade");
        }
        if self.reinstall {
            args.push("--reinstall");
        }
        if let Some(index_url) = self.index_url {
            args.push("--index-url");
            args.push(index_url);
        }
        if let Some(ref extras) = self.spec.extras {
            let extras = extras.iter().flat_map(|e| ["--extra", e]);
            args.extend(extras);
        }
        CmdBuilder::uv(args).run()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCase {
        input: &'static str,
        name: &'static str,
        module_name: &'static str,
        extras: Option<Vec<&'static str>>,
        specifier: Option<&'static str>,
    }

    #[test]
    fn test_install_spec_parse_with_extras_and_version() {
        let test_cases = vec![
            TestCase {
                input: "nonebot-plugin-test",
                name: "nonebot-plugin-test",
                module_name: "nonebot_plugin_test",
                extras: None,
                specifier: None,
            },
            TestCase {
                input: "nonebot-plugin-test<=0.1.0",
                name: "nonebot-plugin-test",
                module_name: "nonebot_plugin_test",
                extras: None,
                specifier: Some("<=0.1.0"),
            },
            TestCase {
                input: "nonebot-plugin-test>=0.1.0",
                name: "nonebot-plugin-test",
                module_name: "nonebot_plugin_test",
                extras: None,
                specifier: Some(">=0.1.0"),
            },
            TestCase {
                input: "nonebot-plugin-test==0.1.0",
                name: "nonebot-plugin-test",
                module_name: "nonebot_plugin_test",
                extras: None,
                specifier: Some("==0.1.0"),
            },
            TestCase {
                input: "nonebot-plugin-test[extra]",
                name: "nonebot-plugin-test",
                module_name: "nonebot_plugin_test",
                extras: Some(vec!["extra"]),
                specifier: None,
            },
            TestCase {
                input: "nonebot-plugin-test[extra]>=0.1.0",
                name: "nonebot-plugin-test",
                module_name: "nonebot_plugin_test",
                extras: Some(vec!["extra"]),
                specifier: Some(">=0.1.0"),
            },
            TestCase {
                input: "nonebot-plugin-test[extra1,extra2]>=0.1.0",
                name: "nonebot-plugin-test",
                module_name: "nonebot_plugin_test",
                extras: Some(vec!["extra1", "extra2"]),
                specifier: Some(">=0.1.0"),
            },
        ];
        for test_case in test_cases {
            let spec = InstallSpec::parse(test_case.input).expect("Parse failed");
            assert_eq!(spec.name, test_case.name);
            assert_eq!(spec.module_name, test_case.module_name);
            assert_eq!(spec.extras, test_case.extras);
            assert_eq!(spec.specifier, test_case.specifier);
        }
    }

    #[test]
    fn test_install_spec_parse_git_url() {
        let test_cases = vec![
            TestCase {
                input: "git+https://github.com/owner/nonebot-plugin-test",
                name: "nonebot-plugin-test",
                module_name: "nonebot_plugin_test",
                extras: None,
                specifier: None,
            },
            TestCase {
                input: "git+https://github.com/owner/nonebot-plugin-test.git",
                name: "nonebot-plugin-test",
                module_name: "nonebot_plugin_test",
                extras: None,
                specifier: None,
            },
        ];
        for test_case in test_cases {
            let spec = InstallSpec::parse(test_case.input).expect("Parse failed");
            assert_eq!(spec.name, test_case.name);
            assert_eq!(spec.module_name, test_case.module_name);
            assert_eq!(spec.extras, test_case.extras);
            assert_eq!(spec.specifier, test_case.specifier);
            assert_eq!(spec.git_url, Some(test_case.input));
        }
    }
}
