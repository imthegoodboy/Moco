use anyhow::{Context, Result, anyhow, bail};
use lopdf::Document;
use quick_xml::Reader;
use quick_xml::events::Event;
use regex::Regex;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

#[derive(Debug, Clone)]
pub struct ExtractedPage {
    pub page: Option<u32>,
    pub text: String,
}

pub fn supported_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "pdf" | "txt" | "md" | "markdown" | "csv" | "html" | "htm" | "docx"
    )
}

pub fn extract(path: &Path) -> Result<Vec<ExtractedPage>> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "pdf" => extract_pdf(path),
        "docx" => extract_docx(path),
        "html" | "htm" => extract_html(path),
        "txt" | "md" | "markdown" | "csv" => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("Could not read {}", path.display()))?;
            Ok(vec![ExtractedPage { page: None, text }])
        }
        _ => bail!("This file type is not supported yet."),
    }
}

fn extract_pdf(path: &Path) -> Result<Vec<ExtractedPage>> {
    let document = Document::load(path).context("The PDF could not be opened")?;
    let mut pages = Vec::new();
    for page_number in document.get_pages().keys() {
        let text = document
            .extract_text(&[*page_number])
            .unwrap_or_else(|_| String::new());
        if !text.trim().is_empty() {
            pages.push(ExtractedPage {
                page: Some(*page_number),
                text,
            });
        }
    }
    if pages.is_empty() {
        return Err(anyhow!(
            "No selectable text was found in this PDF. OCR is not enabled yet."
        ));
    }
    Ok(pages)
}

fn extract_docx(path: &Path) -> Result<Vec<ExtractedPage>> {
    let file = File::open(path).context("The Word document could not be opened")?;
    let mut archive = ZipArchive::new(file).context("The Word document is damaged")?;
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .context("The Word document contains no main content")?
        .read_to_string(&mut xml)?;

    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut output = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(text)) => {
                output.push_str(&text.decode()?);
                output.push(' ');
            }
            Ok(Event::End(end)) if end.name().as_ref() == b"w:p" => output.push('\n'),
            Ok(Event::Eof) => break,
            Err(error) => return Err(error.into()),
            _ => {}
        }
    }
    Ok(vec![ExtractedPage {
        page: None,
        text: output,
    }])
}

fn extract_html(path: &Path) -> Result<Vec<ExtractedPage>> {
    let html = std::fs::read_to_string(path).context("The HTML file could not be read")?;
    let scripts = Regex::new(r"(?is)<script\b.*?>.*?</script>")?;
    let styles = Regex::new(r"(?is)<style\b.*?>.*?</style>")?;
    let tags = Regex::new(r"(?s)<[^>]*>")?;
    let whitespace = Regex::new(r"[ \t\r\x0B\x0C]+")?;
    let text = scripts.replace_all(&html, " ");
    let text = styles.replace_all(&text, " ");
    let text = tags.replace_all(&text, "\n");
    let text = whitespace.replace_all(&text, " ").to_string();
    Ok(vec![ExtractedPage { page: None, text }])
}
