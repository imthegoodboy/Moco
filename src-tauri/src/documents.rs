use anyhow::{Context, Result, anyhow, bail};
use lopdf::Document;
use quick_xml::Reader;
use quick_xml::events::Event;
use regex::Regex;
use std::collections::BTreeMap;
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

pub async fn extract(path: &Path) -> Result<Vec<ExtractedPage>> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "pdf" => extract_pdf(path).await,
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

async fn extract_pdf(path: &Path) -> Result<Vec<ExtractedPage>> {
    let document = Document::load(path).context("The PDF could not be opened")?;
    if document.is_encrypted() {
        bail!("This PDF is password-protected. Remove the password and import it again.");
    }

    let pdf_pages = document.get_pages();
    let mut extracted = BTreeMap::new();
    for page_number in pdf_pages.keys() {
        let text = document
            .extract_text_chunks(&[*page_number])
            .into_iter()
            .filter_map(Result::ok)
            .collect::<String>();
        let text = clean_extracted_text(&text);
        if !text.is_empty() {
            extracted.insert(*page_number, text);
        }
    }

    #[cfg(windows)]
    let mut ocr_error = None;
    #[cfg(windows)]
    if extracted.len() < pdf_pages.len() {
        match extract_pdf_with_windows_ocr(path).await {
            Ok(ocr_pages) => {
                for page in ocr_pages {
                    if let Some(page_number) = page.page {
                        extracted.entry(page_number).or_insert(page.text);
                    }
                }
            }
            Err(error) => ocr_error = Some(error),
        }
    }

    if extracted.is_empty() {
        #[cfg(windows)]
        if let Some(error) = ocr_error {
            return Err(error
                .context("No selectable text was found, and local OCR could not read this PDF"));
        }
        return Err(anyhow!(
            "No readable text was found. Moco tried selectable text and local OCR. For best results, use a clear PDF scan with a Windows OCR language installed."
        ));
    }

    Ok(extracted
        .into_iter()
        .map(|(page, text)| ExtractedPage {
            page: Some(page),
            text,
        })
        .collect())
}

fn clean_extracted_text(text: &str) -> String {
    let text = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace(['\u{0000}', '\u{000c}'], "\n")
        .replace(['\u{00a0}', '\u{2007}', '\u{202f}'], " ")
        .replace('\u{00ad}', "");
    let horizontal_space = Regex::new(r"[\t ]+").expect("PDF whitespace pattern should be valid");
    let mut output = String::new();
    let mut blank_lines = 0usize;

    for raw_line in text.lines() {
        let line = horizontal_space.replace_all(raw_line.trim(), " ");
        if line.is_empty() {
            blank_lines += 1;
            continue;
        }
        if !output.is_empty() {
            let joins_hyphenated_word = output.ends_with('-')
                && line
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_lowercase());
            if joins_hyphenated_word {
                output.pop();
            } else if blank_lines > 0 {
                output.push_str("\n\n");
            } else {
                output.push('\n');
            }
        }
        output.push_str(&line);
        blank_lines = 0;
    }

    output.trim().to_string()
}

#[cfg(windows)]
async fn extract_pdf_with_windows_ocr(path: &Path) -> Result<Vec<ExtractedPage>> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("The local OCR worker could not start")?
            .block_on(extract_pdf_with_windows_ocr_inner(&path))
    })
    .await
    .context("The local OCR worker stopped unexpectedly")?
}

#[cfg(windows)]
async fn extract_pdf_with_windows_ocr_inner(path: &Path) -> Result<Vec<ExtractedPage>> {
    use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
    use windows::Graphics::Imaging::BitmapDecoder;
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::StorageFile;
    use windows::Storage::Streams::InMemoryRandomAccessStream;
    use windows::core::HSTRING;

    let absolute_path = path
        .canonicalize()
        .context("The PDF path could not be resolved for OCR")?;
    // `canonicalize` adds the Win32 `\\?\` prefix on Windows. WinRT's
    // StorageFile API expects a normal drive path and reports the extended
    // form as a misleading "path too long" error.
    let absolute_path = absolute_path.to_string_lossy();
    let storage_path = absolute_path
        .strip_prefix(r"\\?\")
        .unwrap_or(absolute_path.as_ref());
    let storage_path = HSTRING::from(storage_path);
    let open_file = StorageFile::GetFileFromPathAsync(&storage_path)?;
    let file = open_file
        .await
        .context("Windows could not open the PDF for OCR")?;
    let document = PdfDocument::LoadFromFileAsync(&file)?
        .await
        .context("Windows could not render the PDF for OCR")?;
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .context("No compatible Windows OCR language is installed")?;
    let max_dimension = OcrEngine::MaxImageDimension()? as f32;
    let mut pages = Vec::new();

    for index in 0..document.PageCount()? {
        let page = document.GetPage(index)?;
        let size = page.Size()?;
        let scale = 2.5_f32
            .min(max_dimension / size.Width.max(1.0))
            .min(max_dimension / size.Height.max(1.0));
        let options = PdfPageRenderOptions::new()?;
        options.SetDestinationWidth((size.Width * scale).round().max(1.0) as u32)?;
        options.SetDestinationHeight((size.Height * scale).round().max(1.0) as u32)?;
        let stream = InMemoryRandomAccessStream::new()?;
        page.RenderWithOptionsToStreamAsync(&stream, &options)?
            .await?;
        stream.Seek(0)?;
        let decoder = BitmapDecoder::CreateAsync(&stream)?.await?;
        let bitmap = decoder.GetSoftwareBitmapAsync()?.await?;
        let result = engine.RecognizeAsync(&bitmap)?.await?;
        let text = clean_extracted_text(&result.Text()?.to_string());
        if !text.is_empty() {
            pages.push(ExtractedPage {
                page: Some(index + 1),
                text,
            });
        }
        page.Close()?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{Object, Stream, dictionary};
    use uuid::Uuid;

    #[test]
    fn cleans_pdf_spacing_and_rejoins_wrapped_words() {
        assert_eq!(
            clean_extracted_text("A  clear\u{00a0}line\ninter-\nnational text\n\n\nNext paragraph"),
            "A clear line\ninternational text\n\nNext paragraph"
        );
    }

    #[tokio::test]
    async fn extracts_and_preserves_page_number_from_a_text_pdf() {
        let path = std::env::temp_dir().join(format!("moco-pdf-test-{}.pdf", Uuid::new_v4()));
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let font_id = document.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Courier",
        });
        let resources_id = document.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 16.into()]),
                Operation::new("Td", vec![72.into(), 720.into()]),
                Operation::new(
                    "Tj",
                    vec![Object::string_literal(
                        "The launch checklist is stored in this PDF.",
                    )],
                ),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id =
            document.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        });
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
                "Resources" => resources_id,
                "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog_id);
        document.save(&path).expect("test PDF should be saved");

        let pages = extract(&path).await.expect("test PDF should be extracted");

        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].page, Some(1));
        assert!(pages[0].text.contains("launch checklist"));
        let _ = std::fs::remove_file(path);
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "requires MOCO_OCR_FIXTURE pointing to an image-only PDF"]
    async fn extracts_text_from_a_scanned_pdf_with_windows_ocr() {
        let path = std::env::var("MOCO_OCR_FIXTURE").expect("OCR fixture path should be set");
        let pages = extract(Path::new(&path))
            .await
            .expect("scanned PDF should be extracted through the public pipeline");
        assert!(
            pages
                .iter()
                .any(|page| page.text.to_ascii_lowercase().contains("pinky"))
        );
    }
}
