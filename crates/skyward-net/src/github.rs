//! Skyward's own releases on GitHub: which is the newest, and the image in
//! it for this phone, as the release workflow names it.

use anyhow::{bail, ensure, Context, Result};
use serde::Deserialize;

/// The repository this fork's releases are published in. Upstream's own
/// `Rinrroy/skyward-for-haotian` builds for many phones; this fork is the Xiaomi 15 Pro
/// alone, so it looks at its own releases and nowhere else.
pub const REPOSITORY: &str = "Rinrroy/skyward-for-haotian";

/// Where GitHub answers which release of this fork is the newest.
pub const LATEST: &str = "https://api.github.com/repos/Rinrroy/skyward-for-haotian/releases/latest";

/// The release whose Resources hold files shared by Skyward builds.
pub const RESOURCE: &str = "https://github.com/CLF184/skyward/releases/tag/resource";

/// The version of GitHub's API the answer is read as.
const API_VERSION: &str = "2022-11-28";

/// Where GitHub pages and release files are read from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Address {
    #[default]
    GitHub,
    GhProxy,
    GhFast,
    GhProxyNet,
}

impl Address {
    pub fn key(self) -> &'static str {
        match self {
            Address::GitHub => "github",
            Address::GhProxy => "gh_proxy",
            Address::GhFast => "ghfast",
            Address::GhProxyNet => "ghproxy_net",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "github" => Some(Address::GitHub),
            "gh_proxy" => Some(Address::GhProxy),
            "ghfast" => Some(Address::GhFast),
            "ghproxy_net" => Some(Address::GhProxyNet),
            _ => None,
        }
    }

    /// `url`, through this address when it is a proxy.
    pub fn source(self, url: &str) -> String {
        match self {
            Address::GitHub => url.to_string(),
            Address::GhProxy => format!("https://gh-proxy.com/{url}"),
            Address::GhFast => format!("https://ghfast.top/{url}"),
            Address::GhProxyNet => format!("https://ghproxy.net/{url}"),
        }
    }
}

/// The newest release, as the build of Skyward that asked sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Newest {
    /// Its version, such as 0.1.4.
    pub version: String,
    /// It is newer than the build that asked.
    pub newer: bool,
    /// Its build for the phone; none when it has none.
    pub build: Option<Build>,
}

/// A phone's build in a release, which is Skyward's image for the phone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Build {
    /// The image's own name, such as
    /// skyward-v0.1.4-ishtar-OS3.0.308.0.WMACNXM.img.
    pub name: String,
    /// Where GitHub's API hands it out, which [`locate`] asks for where the
    /// file itself is.
    pub url: String,
    /// The release's own link to it on github.com.
    pub link: String,
    pub size: u64,
    /// Its SHA-256 as GitHub gives it, in hex.
    pub sha256: String,
}

impl Build {
    /// Where to download the image from, in the order to try: the file's own
    /// short-lived address, where GitHub's API gave one, then the release's
    /// link on github.com, which some networks do not reach.
    pub fn sources(&self, located: Option<String>, address: Address) -> Vec<String> {
        match address {
            Address::GitHub => located.into_iter().chain([self.link.clone()]).collect(),
            Address::GhProxy | Address::GhFast | Address::GhProxyNet => [address.source(&self.link), self.link.clone()].to_vec(),
        }
    }
}

/// The files in the Resources section `page` holds.
pub fn assets(page: &str) -> Result<Vec<Build>> {
    let mut builds = Vec::new();
    let marker = "/releases/download/";
    let mut at = 0;
    while let Some(found) = page[at..].find(marker) {
        let found = at + found;
        let href_start = page[..found].rfind("href=\"").context("the Resources section has a file without a link")? + 6;
        let href_end = page[href_start..].find('"').with_context(|| format!("read the link at byte {href_start}"))? + href_start;
        let href = &page[href_start..href_end];
        at = href_end;
        let name = href.rsplit('/').next().unwrap_or(href);
        let block_end = page[href_start..].find("</li>").map_or(page.len(), |end| href_start + end);
        let block = &page[href_start..block_end];
        let sha256 = resource_sha256(block).with_context(|| format!("GitHub gives no SHA-256 of {name}"))?;
        let link = if href.starts_with("https://") { href.to_string() } else { format!("https://github.com{href}") };
        builds.push(Build { name: name.to_string(), url: link.clone(), link, size: resource_size(block).unwrap_or(0), sha256 });
    }
    ensure!(!builds.is_empty(), "the release Resources section has no files");
    Ok(builds)
}

/// The SHA-256 GitHub shows in a Resources section block.
fn resource_sha256(block: &str) -> Option<String> {
    let start = block.find("sha256:")? + "sha256:".len();
    let digest = block.get(start..start + 64)?;
    digest.bytes().all(|b| b.is_ascii_hexdigit()).then(|| digest.to_ascii_lowercase())
}

/// The size GitHub shows in a Resources section block.
fn resource_size(block: &str) -> Option<u64> {
    const MARKER: &str = "min-width: clamp(50px, 15vw, 75px)";
    let marker = block.find(MARKER)?;
    let start = block[marker..].find('>')? + marker + 1;
    let end = block[start..].find("</span>")? + start;
    bytes(block[start..end].trim())
}

fn bytes(text: &str) -> Option<u64> {
    let (number, unit) = text.split_once(' ')?;
    let number = number.parse::<f64>().ok()?;
    let scale = match unit {
        "Bytes" => 1.0,
        "KB" => 1_000.0,
        "MB" => 1_000_000.0,
        "GB" => 1_000_000_000.0,
        "TB" => 1_000_000_000_000.0,
        _ => return None,
    };
    Some((number * scale).round() as u64)
}

/// The Resources section of the shared `resource` release.
fn resource_page() -> &'static str {
    "https://github.com/CLF184/skyward/releases/expanded_assets/resource"
}

/// Ask GitHub for the files in the shared `resource` release.
pub fn ask_resources(agent: &ureq::Agent, address: Address, running: &str) -> Result<Vec<Build>> {
    let page = read(agent, &address.source(resource_page()), running)?;
    assets(&page)
}

/// What `url` answers as text.
fn read(agent: &ureq::Agent, url: &str, running: &str) -> Result<String> {
    let mut response = agent
        .get(url)
        .header("Accept", "text/html")
        .header("User-Agent", format!("Skyward/{running}"))
        .call()
        .with_context(|| format!("ask {url}"))?;
    let status = response.status();
    let body = response.body_mut().read_to_string().with_context(|| format!("read the answer of {url}"))?;
    if !status.is_success() {
        bail!(refusal(status, &body));
    }
    Ok(body)
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    url: String,
    name: String,
    size: u64,
    /// The digest GitHub took of the file as it was uploaded, such as
    /// `sha256:` and the SHA-256 in hex.
    digest: Option<String>,
    browser_download_url: String,
}

/// Why GitHub turned a question down, as it says.
#[derive(Deserialize)]
struct Message {
    message: String,
}

/// The newest release that `answer`, GitHub's answer at [`LATEST`], tells
/// of, and its build for the phone `codename` names, as the build of version
/// `running` sees it.
pub fn newest(answer: &str, codename: &str, running: &str) -> Result<Newest> {
    let release: Release = serde_json::from_str(answer).context("read GitHub's answer")?;
    let tag = release.tag_name;
    let version = tag.strip_prefix('v').with_context(|| format!("the newest release is tagged {tag}, which is not v and a version"))?;
    let newest = semver::Version::parse(version).with_context(|| format!("the newest release is tagged {tag}"))?;
    let this = semver::Version::parse(running).with_context(|| format!("this build's version is {running}"))?;
    // The image is named skyward-, the tag, the phone's codename, and the
    // stock build it is made on, and is put on the release as it is - not
    // zipped, so the phone downloads what it writes.
    let start = format!("skyward-{tag}-{codename}-");
    let build = match release.assets.into_iter().find(|a| a.name.starts_with(&start) && a.name.ends_with(".img")) {
        None => None,
        Some(a) => {
            let sha256 = a.digest.as_deref().and_then(|d| d.strip_prefix("sha256:")).with_context(|| format!("GitHub gives no SHA-256 of {}", a.name))?;
            ensure!(sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_hexdigit()), "GitHub gives {} the SHA-256 {sha256}", a.name);
            Some(Build { sha256: sha256.to_lowercase(), name: a.name, url: a.url, link: a.browser_download_url, size: a.size })
        }
    };
    Ok(Newest { version: version.to_string(), newer: newest > this, build })
}

/// Ask GitHub for Skyward's newest release, and its build for the phone
/// `codename` names, as the build of version `running`.
pub fn ask(agent: &ureq::Agent, codename: &str, running: &str) -> Result<Newest> {
    let mut response = agent
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .header("User-Agent", format!("Skyward/{running}"))
        .call()
        .with_context(|| format!("ask {LATEST}"))?;
    let status = response.status();
    let body = response.body_mut().read_to_string().with_context(|| format!("read the answer of {LATEST}"))?;
    if !status.is_success() {
        bail!(refusal(status, &body));
    }
    newest(&body, codename, running)
}

/// Where the file a build's `url` hands out is now, as GitHub's API sends
/// the build of version `running` that asks for it on: a short-lived
/// address of the file itself, away from github.com.
pub fn locate(agent: &ureq::Agent, url: &str, running: &str) -> Result<String> {
    let mut response = agent
        .get(url)
        .header("Accept", "application/octet-stream")
        .header("X-GitHub-Api-Version", API_VERSION)
        .header("User-Agent", format!("Skyward/{running}"))
        .config()
        .max_redirects(0)
        .build()
        .call()
        .with_context(|| format!("ask {url}"))?;
    let status = response.status();
    if status.is_redirection() {
        let location = response.headers().get(ureq::http::header::LOCATION).with_context(|| format!("GitHub answered {status}, and names no address"))?;
        return Ok(location.to_str().context("GitHub names an address that is not text")?.to_string());
    }
    ensure!(!status.is_success(), "GitHub sends the file itself in its answer, at no address of its own");
    let body = response.body_mut().read_to_string().with_context(|| format!("read the answer of {url}"))?;
    bail!(refusal(status, &body))
}

/// Why GitHub turned a question down, as it says, as when too many questions
/// came from the address that asked.
fn refusal(status: ureq::http::StatusCode, body: &str) -> String {
    match serde_json::from_str::<Message>(body) {
        Ok(m) => format!("GitHub answered {status}: {}", m.message),
        Err(_) => format!("GitHub answered {status}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What GitHub answered of the newest release on 2026-11-07, with the
    /// parts that do not matter here left out.
    const ANSWERED_013: &str = r#"{"url":"https://api.github.com/repos/Rinrroy/skyward-for-haotian/releases/437365758","tag_name":"v0.1.3","name":"v0.1.3","draft":false,"prerelease":false,"assets":[{"url":"https://api.github.com/repos/Rinrroy/skyward-for-haotian/releases/assets/719751468","id":719751468,"name":"SHA256SUMS","label":"","content_type":"application/octet-stream","state":"uploaded","size":2337,"digest":"sha256:052f9ca757baeb2eac78c8e546bcc54178f6bb9bf9e5bde53fa81b3f48448957","browser_download_url":"https://github.com/Rinrroy/skyward-for-haotian/releases/download/v0.1.3/SHA256SUMS"},{"url":"https://api.github.com/repos/Rinrroy/skyward-for-haotian/releases/assets/719751598","id":719751598,"name":"skyward-v0.1.3-ishtar-OS3.0.308.0.WMACNXM.img","label":"Xiaomi 13 Ultra (ishtar)","content_type":"application/octet-stream","state":"uploaded","size":42739608,"digest":"sha256:62b66f7d13a0eb6c9af3359b5ec6f9c223fd9a86b1f8abab3e9d2646cb2ce534","browser_download_url":"https://github.com/Rinrroy/skyward-for-haotian/releases/download/v0.1.3/skyward-v0.1.3-ishtar-OS3.0.308.0.WMACNXM.img"},{"url":"https://api.github.com/repos/Rinrroy/skyward-for-haotian/releases/assets/719751747","id":719751747,"name":"skyward-v0.1.3-nezha-OS4.0.4.0.XPACNXM.img","label":"Xiaomi 17 Ultra (nezha)","content_type":"application/octet-stream","state":"uploaded","size":43145196,"digest":"sha256:9f6165079529418d8f6635beddfdd9abf9a70c89d69eb467a9922b0db69bfaf1","browser_download_url":"https://github.com/Rinrroy/skyward-for-haotian/releases/download/v0.1.3/skyward-v0.1.3-nezha-OS4.0.4.0.XPACNXM.img"}]}"#;

    #[test]
    fn the_newest_release_gives_the_build_for_the_phone() {
        let n = newest(ANSWERED_013, "ishtar", "0.1.2").unwrap();
        assert_eq!(n.version, "0.1.3");
        assert!(n.newer);
        assert_eq!(
            n.build,
            Some(Build {
                name: "skyward-v0.1.3-ishtar-OS3.0.308.0.WMACNXM.img".into(),
                url: "https://api.github.com/repos/Rinrroy/skyward-for-haotian/releases/assets/719751598".into(),
                link: "https://github.com/Rinrroy/skyward-for-haotian/releases/download/v0.1.3/skyward-v0.1.3-ishtar-OS3.0.308.0.WMACNXM.img".into(),
                size: 42739608,
                sha256: "62b66f7d13a0eb6c9af3359b5ec6f9c223fd9a86b1f8abab3e9d2646cb2ce534".into(),
            })
        );
        assert_eq!(newest(ANSWERED_013, "nezha", "0.1.2").unwrap().build.unwrap().size, 43145196);
    }

    #[test]
    fn an_image_is_downloaded_from_where_github_sends_it_then_from_its_link() {
        let build = newest(ANSWERED_013, "ishtar", "0.1.2").unwrap().build.unwrap();
        let link = "https://github.com/Rinrroy/skyward-for-haotian/releases/download/v0.1.3/skyward-v0.1.3-ishtar-OS3.0.308.0.WMACNXM.img";
        assert_eq!(
            build.sources(Some("https://release-assets.githubusercontent.com/a?sig=1".into()), Address::GitHub),
            ["https://release-assets.githubusercontent.com/a?sig=1", link]
        );
        assert_eq!(build.sources(None, Address::GitHub), [link], "where GitHub's API gave no address");
        assert_eq!(
            build.sources(None, Address::GhProxy),
            [format!("https://gh-proxy.com/{link}"), link.into()],
            "the proxy comes first, with GitHub itself after it"
        );
        assert_eq!(Address::GhFast.source("https://github.com/a/b"), "https://ghfast.top/https://github.com/a/b");
        assert_eq!(Address::from_key("ghproxy_net"), Some(Address::GhProxyNet));
    }

    #[test]
    fn every_file_in_the_resource_release_is_an_option() {
        let page = r#"<ul>
        <li><a href="/CLF184/skyward/releases/download/resource/root.zip">Root</a><span>sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa</span><span style="min-width: clamp(50px, 15vw, 75px);">12.0 MB</span></li>
        <li><a href="/CLF184/skyward/releases/download/resource/boot.img">Boot</a><span>sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb</span><span style="min-width: clamp(50px, 15vw, 75px);">8.5 MB</span></li>
        </ul>"#;
        let builds = assets(page).unwrap();
        assert_eq!(builds.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["root.zip", "boot.img"]);
        assert_eq!(builds[0].size, 12_000_000);
        assert_eq!(builds[0].link, "https://github.com/CLF184/skyward/releases/download/resource/root.zip");
        assert_eq!(RESOURCE, "https://github.com/CLF184/skyward/releases/tag/resource");
    }

    /// A server on this computer that answers one request with `answer`,
    /// and gives the request it had.
    fn server(answer: String) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/repos/Rinrroy/skyward-for-haotian/releases/assets/719751598", listener.local_addr().unwrap());
        let held = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 512];
            while !request.ends_with(b"\r\n\r\n") {
                let n = s.read(&mut chunk).unwrap();
                request.extend_from_slice(&chunk[..n]);
            }
            s.write_all(answer.as_bytes()).unwrap();
            String::from_utf8(request).unwrap()
        });
        (url, held)
    }

    #[test]
    fn github_s_api_sends_the_image_on_to_where_the_file_is() {
        let agent = crate::http::agent(Default::default());
        let sent_on = "HTTP/1.1 302 Found\r\nLocation: https://release-assets.githubusercontent.com/a?sig=1\r\nContent-Length: 0\r\n\r\n";
        let (url, held) = server(sent_on.into());
        assert_eq!(locate(&agent, &url, "0.1.2").unwrap(), "https://release-assets.githubusercontent.com/a?sig=1", "the address, not followed");
        let request = held.join().unwrap().to_lowercase();
        assert!(request.contains("accept: application/octet-stream\r\n"), "asked for the file itself: {request}");
        assert!(request.contains("user-agent: skyward/0.1.2\r\n"), "{request}");
        let why = r#"{"message":"API rate limit exceeded for 203.0.113.7"}"#;
        let (url, _) = server(format!("HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{why}", why.len()));
        assert_eq!(locate(&agent, &url, "0.1.2").unwrap_err().to_string(), "GitHub answered 403 Forbidden: API rate limit exceeded for 203.0.113.7");
        let (url, _) = server("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nPK".into());
        assert!(locate(&agent, &url, "0.1.2").is_err(), "the file itself, with no address to download it from again");
    }

    #[test]
    fn a_release_is_newer_only_than_an_older_build() {
        assert!(!newest(ANSWERED_013, "ishtar", "0.1.3").unwrap().newer, "the same version");
        assert!(!newest(ANSWERED_013, "ishtar", "0.2.0").unwrap().newer, "a build ahead of every release");
        assert!(newest(ANSWERED_013, "ishtar", "0.1.3-rc.1").unwrap().newer, "a build before the release it led to");
        assert!(newest(ANSWERED_013, "ishtar", "0.1").is_err(), "a version this build cannot have");
    }

    #[test]
    fn a_phone_is_found_by_its_whole_codename() {
        assert_eq!(newest(ANSWERED_013, "fuxi", "0.1.2").unwrap().build, None, "a phone the release has no build for");
        assert_eq!(newest(ANSWERED_013, "ish", "0.1.2").unwrap().build, None, "the start of another phone's codename");
        assert_eq!(newest(ANSWERED_013, "SHA256SUMS", "0.1.2").unwrap().build, None);
    }

    #[test]
    fn a_build_needs_its_sha256_and_a_release_its_version() {
        let unsummed = ANSWERED_013.replace("\"digest\":\"sha256:62b66f7d13a0eb6c9af3359b5ec6f9c223fd9a86b1f8abab3e9d2646cb2ce534\"", "\"digest\":null");
        let e = newest(&unsummed, "ishtar", "0.1.2").unwrap_err();
        assert_eq!(e.to_string(), "GitHub gives no SHA-256 of skyward-v0.1.3-ishtar-OS3.0.308.0.WMACNXM.img");
        assert!(newest(&unsummed, "nezha", "0.1.2").is_ok(), "another phone's build has its own");
        let untagged = ANSWERED_013.replace("\"tag_name\":\"v0.1.3\"", "\"tag_name\":\"nightly\"");
        assert!(newest(&untagged, "ishtar", "0.1.2").is_err());
        assert!(newest("{}", "ishtar", "0.1.2").is_err());
    }
}
