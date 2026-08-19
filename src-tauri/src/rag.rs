use crate::models::SourceRef;
use std::collections::{HashMap, HashSet};

pub const EMBEDDING_DIMENSIONS: usize = 384;

pub fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|part| part.len() > 1)
        .map(str::to_lowercase)
        .collect()
}

fn fnv1a(token: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in token.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// A deterministic, entirely local feature-hashing embedding. It provides a
/// compact vector index with no network or model download requirement.
pub fn embed(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0f32; EMBEDDING_DIMENSIONS];
    let tokens = tokenize(text);
    if tokens.is_empty() {
        return vector;
    }

    let mut counts = HashMap::<String, usize>::new();
    for token in tokens {
        *counts.entry(token).or_insert(0) += 1;
    }

    for (token, count) in counts {
        let hash = fnv1a(&token);
        let index = (hash as usize) % EMBEDDING_DIMENSIONS;
        let sign = if (hash >> 63) == 0 { 1.0 } else { -1.0 };
        vector[index] += sign * (1.0 + count as f32).ln();
    }

    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

pub fn cosine(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}

pub fn chunk_text(text: &str, target_chars: usize, overlap_chars: usize) -> Vec<String> {
    if text.trim().is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();

    while start < bytes.len() {
        let mut end = (start + target_chars).min(bytes.len());
        while end < bytes.len() && !text.is_char_boundary(end) {
            end -= 1;
        }

        if end < bytes.len() {
            let search_start = start + (target_chars * 2 / 3).min(end - start);
            if let Some(relative) =
                text[search_start..end].rfind(|c: char| matches!(c, '.' | '!' | '?' | '\n'))
            {
                end = search_start + relative + 1;
            }
        }

        let chunk = text[start..end].trim();
        if !chunk.is_empty() {
            chunks.push(chunk.to_string());
        }
        if end == bytes.len() {
            break;
        }

        let mut next = end.saturating_sub(overlap_chars);
        while next < end && !text.is_char_boundary(next) {
            next += 1;
        }
        start = next.max(start + 1);
    }
    chunks
}

pub fn lexical_overlap(query: &str, text: &str) -> f32 {
    let query_tokens: HashSet<_> = tokenize(query).into_iter().collect();
    if query_tokens.is_empty() {
        return 0.0;
    }
    let text_tokens: HashSet<_> = tokenize(text).into_iter().collect();
    let matches = query_tokens.intersection(&text_tokens).count();
    matches as f32 / query_tokens.len() as f32
}

pub fn build_context(sources: &[SourceRef]) -> String {
    if sources.is_empty() {
        return String::new();
    }
    let mut output = String::from(
        "\n\nLOCAL DOCUMENT CONTEXT\nUse these excerpts as the source of truth. Cite them as [1], [2], etc.\n",
    );
    for (index, source) in sources.iter().enumerate() {
        let page = source
            .page
            .map(|p| format!(", page {p}"))
            .unwrap_or_default();
        output.push_str(&format!(
            "\n[{}] {}{}\n{}\n",
            index + 1,
            source.document_name,
            page,
            source.excerpt
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relevant_text_scores_higher() {
        let query = embed("quantum computing hardware");
        let related = embed("quantum computing requires specialized hardware");
        let unrelated = embed("banana bread recipe and baking time");
        assert!(cosine(&query, &related) > cosine(&query, &unrelated));
    }

    #[test]
    fn chunker_preserves_content_without_empty_chunks() {
        let input =
            "A useful first sentence. A second sentence with more detail. A final sentence.";
        let chunks = chunk_text(input, 32, 8);
        assert!(chunks.len() >= 2);
        assert!(chunks.iter().all(|chunk| !chunk.trim().is_empty()));
    }
}
