use super::TaxonomySchema;
use crate::{Result, XbrlError};
use log::{info, warn};
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use reqwest::{Url, blocking::Client};
use std::{
    collections::{HashSet, VecDeque},
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

/// Downloads taxonomy resources (schemas and linkbases) starting from an
/// entry-point schema URL.
#[derive(Debug, Clone)]
pub struct TaxonomyLoader {
    client: Client,
}

impl TaxonomyLoader {
    /// Create a loader with sensible HTTP defaults.
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(io_other)?;
        Ok(Self { client })
    }

    /// Download all reachable HTTP(S) taxonomy files from one or more
    /// `entry_urls`.
    ///
    /// The destination layout mirrors URL paths and strips `/taxonomies/` for
    /// compatibility with `TaxonomySet::discover`.
    ///
    /// This method is best-effort: individual network/parse failures are
    /// logged as warnings and processing continues.
    pub fn download_all<I, S>(
        &self,
        entry_urls: I,
        destination_root: impl AsRef<Path>,
    ) -> Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let destination_root = destination_root.as_ref();
        fs::create_dir_all(destination_root)?;

        let mut queue = VecDeque::new();
        for entry_url in entry_urls {
            let entry_url = entry_url.as_ref();
            let entry = Url::parse(entry_url).map_err(|err| {
                XbrlError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid entry taxonomy url '{entry_url}': {err}"),
                ))
            })?;

            queue.push_back(entry);
        }

        if queue.is_empty() {
            return Err(XbrlError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "no entry taxonomy URLs provided",
            )));
        }

        let mut visited: HashSet<String> = HashSet::new();

        while let Some(url) = queue.pop_front() {
            if !visited.insert(url.as_str().to_owned()) {
                continue;
            }

            if !is_http_url(&url) {
                warn!("Skipping non-HTTP URL: {url}");
                continue;
            }

            let relative_path = local_relative_path(&url);
            let local_path = destination_root.join(&relative_path);

            if local_path.exists() {
                warn!(
                    "File {} already exists, skipping download",
                    relative_path.display(),
                );
            } else {
                info!("Downloading {}", url);

                let fetched = match self.fetch_text(&url) {
                    Ok(body) => body,
                    Err(err) => {
                        warn!("Failed downloading {url}: {err}");
                        continue;
                    }
                };

                if let Some(parent) = local_path.parent()
                    && let Err(err) = fs::create_dir_all(parent)
                {
                    warn!(
                        "Failed creating destination directory {} for {}: {err}",
                        parent.display(),
                        url
                    );
                    continue;
                }

                if let Err(err) = fs::write(&local_path, &fetched) {
                    warn!(
                        "Failed writing downloaded file {}: {err}",
                        local_path.display()
                    );
                    continue;
                }
            }

            if is_schema_url(&url) {
                let mut schema_reader = match open_xml_reader(&local_path) {
                    Ok(reader) => reader,
                    Err(err) => {
                        warn!("Failed opening schema file {}: {err}", local_path.display());
                        continue;
                    }
                };

                let schema = match TaxonomySchema::from_xml(&local_path, &mut schema_reader) {
                    Ok(schema) => schema,
                    Err(err) => {
                        warn!(
                            "Failed parsing schema {} ({}): {err}",
                            local_path.display(),
                            url
                        );
                        continue;
                    }
                };

                for import in &schema.imports {
                    if let Some(location) = &import.schema_location {
                        enqueue_http_reference(&url, location, &mut queue);
                    }
                }

                for include in &schema.includes {
                    enqueue_http_reference(&url, &include.schema_location, &mut queue);
                }

                for linkbase_ref in &schema.linkbase_refs {
                    enqueue_http_reference(&url, &linkbase_ref.href, &mut queue);
                }

                for schema_location in &schema.schema_location_refs {
                    enqueue_http_reference(&url, schema_location, &mut queue);
                }
            } else {
                let mut reader = match open_xml_reader(&local_path) {
                    Ok(reader) => reader,
                    Err(err) => {
                        warn!(
                            "Failed opening XML file {} for schemaLocation scan: {err}",
                            local_path.display()
                        );
                        continue;
                    }
                };

                enqueue_xml_schema_location_references(&url, &mut reader, &mut queue);
            }
        }

        Ok(())
    }

    fn fetch_text(&self, url: &Url) -> std::io::Result<String> {
        let response = self.client.get(to_https(url)).send().map_err(io_other)?;

        if !response.status().is_success() {
            return Err(std::io::Error::other(format!(
                "unexpected HTTP status {} for {}",
                response.status(),
                url
            )));
        }

        response.text().map_err(io_other)
    }
}

fn enqueue_http_reference(base_url: &Url, reference: &str, queue: &mut VecDeque<Url>) {
    let reference = reference.trim();
    if reference.is_empty() {
        return;
    }

    match base_url.join(reference) {
        Ok(url) if is_http_url(&url) => queue.push_back(url),
        Ok(url) => warn!("Skipping non-HTTP reference: {url}"),
        Err(err) => warn!("Failed to resolve reference '{reference}' from {base_url}: {err}"),
    }
}

/// Parse XML content to find schema location references in `xsi:schemaLocation`
/// and `xsi:noNamespaceSchemaLocation` attributes, and enqueue them for
/// downloading.
fn enqueue_xml_schema_location_references(
    base_url: &Url,
    reader: &mut Reader<impl io::BufRead>,
    queue: &mut VecDeque<Url>,
) {
    reader.config_mut().trim_text_start = true;
    reader.config_mut().trim_text_end = true;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                enqueue_schema_location_from_attrs(base_url, e, queue);
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                warn!("Failed parsing XML schemaLocation attributes from {base_url}: {err}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }
}

/// Extract schema location references from XML element attributes and enqueue them.
fn enqueue_schema_location_from_attrs(
    base_url: &Url,
    element: &BytesStart<'_>,
    queue: &mut VecDeque<Url>,
) {
    for attr in element.attributes().with_checks(false).flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if xml_local_name(&key) != "schemaLocation"
            && xml_local_name(&key) != "noNamespaceSchemaLocation"
        {
            continue;
        }

        let value = String::from_utf8_lossy(attr.value.as_ref());
        for location in parse_schema_location_value(&value) {
            enqueue_http_reference(base_url, location, queue);
        }
    }
}

fn parse_schema_location_value(value: &str) -> Vec<&str> {
    let tokens: Vec<&str> = value.split_whitespace().collect();

    if tokens.is_empty() {
        return Vec::new();
    }

    if tokens.len() == 1 {
        return tokens;
    }

    if tokens.len().is_multiple_of(2) {
        return tokens.into_iter().skip(1).step_by(2).collect();
    }

    tokens
}

fn open_xml_reader(path: &Path) -> io::Result<Reader<io::BufReader<fs::File>>> {
    let file = fs::File::open(path)?;
    let mut reader = Reader::from_reader(io::BufReader::new(file));
    reader.config_mut().trim_text_start = true;
    reader.config_mut().trim_text_end = true;
    Ok(reader)
}

fn xml_local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn is_http_url(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

fn is_schema_url(url: &Url) -> bool {
    url.path().to_ascii_lowercase().ends_with(".xsd")
}

fn local_relative_path(url: &Url) -> PathBuf {
    let segments: Vec<&str> = url
        .path_segments()
        .map_or_else(Vec::new, |parts| parts.collect());

    let mut relative = PathBuf::new();
    let starts_with_taxonomies = segments.first().copied() == Some("taxonomies");

    if starts_with_taxonomies {
        for segment in segments
            .into_iter()
            .skip(1)
            .filter(|segment| !segment.is_empty())
        {
            relative.push(segment);
        }
    } else {
        if let Some(host) = url.host_str() {
            relative.push("_external");
            relative.push(host);
        }
        for segment in segments.into_iter().filter(|segment| !segment.is_empty()) {
            relative.push(segment);
        }
    }

    if relative.as_os_str().is_empty() {
        relative.push("index.xsd");
    }

    relative
}

/// Upgrade an `http` URL to `https`.
fn to_https(url: &Url) -> Url {
    if url.scheme() == "http" {
        let mut upgraded = url.clone();
        // set_scheme only fails for cannot-be-a-base URLs, which http/https are not
        let _ = upgraded.set_scheme("https");
        upgraded
    } else {
        url.clone()
    }
}

fn io_other(err: impl std::error::Error + Send + Sync + 'static) -> std::io::Error {
    std::io::Error::other(err)
}
