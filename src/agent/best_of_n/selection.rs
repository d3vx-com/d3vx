//! Best-of-N selection logic: selector agent, heuristic ranking, tie-breaking

use std::sync::Arc;

use super::helpers::truncate_preview;
use super::types::*;
use crate::providers::Provider;

/// Select the best variant using a selector agent with optional prompt override.
pub async fn select_best_with_prompt(
    provider: &Arc<dyn Provider>,
    config: &BestOfNConfig,
    variants: &[VariantResult],
    original_prompt: &str,
    selector_prompt_override: Option<&str>,
) -> Result<(usize, Option<String>), BestOfNError> {
    if variants.is_empty() {
        return Err(BestOfNError::NoVariants);
    }

    if variants.len() == 1 {
        return Ok((0, None));
    }

    let letters: Vec<char> = (0..variants.len())
        .map(|i| (b'A' + i as u8) as char)
        .collect();

    let mut selection_prompt = format!(
        "{}\n\nEvaluate the following {} implementations:\n\n",
        original_prompt,
        variants.len()
    );

    for (i, variant) in variants.iter().enumerate() {
        let letter = letters[i];
        let preview = truncate_preview(&variant.content, 500);
        selection_prompt.push_str(&format!("{}:\n{}\n\n---\n\n", letter, preview));
    }

    selection_prompt.push_str("\nSelect the best implementation (reply with just the letter):");

    let selector_model = config
        .selector_model
        .clone()
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

    let request = crate::providers::MessagesRequest {
        model: selector_model,
        messages: vec![crate::providers::Message {
            role: crate::providers::Role::User,
            content: crate::providers::MessageContent::Text(format!(
                "{}\n\n{}",
                selector_prompt_override.unwrap_or(&config.selector_prompt),
                selection_prompt
            )),
        }],
        system_prompt: None,
        tools: vec![],
        max_tokens: Some(10),
        temperature: Some(0.0),
        thinking: None,
        prompt_caching: true,
    };

    let mut stream = provider
        .send(request)
        .await
        .map_err(|e| BestOfNError::ProviderError(e.to_string()))?;

    let mut response = String::new();
    use futures::StreamExt;
    while let Some(event) = stream.next().await {
        if let Ok(crate::providers::StreamEvent::TextDelta { text }) = event {
            response.push_str(&text);
        }
    }

    let response_upper = response.trim().to_uppercase();
    let selector_selected = response_upper
        .chars()
        .find(|c| c.is_ascii_alphabetic())
        .and_then(|c| Some(c.to_ascii_uppercase()))
        .and_then(|c| {
            let index = (c as u8 - b'A') as usize;
            if index < variants.len() {
                Some(index)
            } else {
                None
            }
        })
        .unwrap_or(0);

    let heuristic_selected = rank_variants_heuristically(variants);

    if selector_selected == heuristic_selected || variants.len() <= 2 {
        return Ok((selector_selected, Some(response)));
    }

    // Tie-break between selector and heuristic disagreement
    let tie_break_prompt = format!(
        "A first-pass selector chose candidate {} but heuristic scoring prefers candidate {}.\nRe-evaluate only these two candidates carefully and return the stronger one.",
        (b'A' + selector_selected as u8) as char,
        (b'A' + heuristic_selected as u8) as char
    );
    let reduced_variants = vec![
        variants[selector_selected].clone(),
        variants[heuristic_selected].clone(),
    ];
    let (tie_break_choice, tie_break_reasoning) = run_selector_pass(
        provider,
        config,
        &reduced_variants,
        original_prompt,
        Some(&format!(
            "{}\n\n{}",
            selector_prompt_override.unwrap_or(&config.selector_prompt),
            tie_break_prompt
        )),
    )
    .await?;
    let final_index = if tie_break_choice == 0 {
        selector_selected
    } else {
        heuristic_selected
    };
    Ok((
        final_index,
        Some(format!(
            "selector={} heuristic={} tie_break={}",
            response,
            heuristic_selected,
            tie_break_reasoning.unwrap_or_default()
        )),
    ))
}

/// Rank variants using heuristic scoring.
pub fn rank_variants_heuristically(variants: &[VariantResult]) -> usize {
    variants
        .iter()
        .enumerate()
        .max_by_key(|(_, variant)| {
            let has_error_penalty = variant.error.is_some() as i32 * -10_000;
            let code_fence_bonus = variant.content.matches("```").count() as i32 * 50;
            let content_len = variant.content.len().min(20_000) as i32;
            let normalized = variant.content.to_lowercase();
            let test_signal = ["test", "tests", "lint", "validated", "verified", "pass"]
                .iter()
                .filter(|marker| normalized.contains(**marker))
                .count() as i32
                * 40;
            let docs_signal = ["readme", "docs", "documentation", "example"]
                .iter()
                .filter(|marker| normalized.contains(**marker))
                .count() as i32
                * 20;
            let scope_signal = ["ownership", "scope", "module", "file"]
                .iter()
                .filter(|marker| normalized.contains(**marker))
                .count() as i32
                * 15;
            let conflict_penalty = ["conflict", "todo", "manual", "blocker", "uncertain"]
                .iter()
                .filter(|marker| normalized.contains(**marker))
                .count() as i32
                * -35;
            has_error_penalty
                + code_fence_bonus
                + content_len
                + test_signal
                + docs_signal
                + scope_signal
                + conflict_penalty
        })
        .map(|(index, _)| index)
        .unwrap_or(0)
}

/// Run a single selector pass over variants.
pub async fn run_selector_pass(
    provider: &Arc<dyn Provider>,
    config: &BestOfNConfig,
    variants: &[VariantResult],
    original_prompt: &str,
    selector_prompt_override: Option<&str>,
) -> Result<(usize, Option<String>), BestOfNError> {
    let letters: Vec<char> = (0..variants.len())
        .map(|i| (b'A' + i as u8) as char)
        .collect();

    let mut selection_prompt = format!(
        "{}\n\nEvaluate the following {} implementations:\n\n",
        original_prompt,
        variants.len()
    );

    for (i, variant) in variants.iter().enumerate() {
        let letter = letters[i];
        let preview = truncate_preview(&variant.content, 500);
        selection_prompt.push_str(&format!("{}:\n{}\n\n---\n\n", letter, preview));
    }

    selection_prompt.push_str("\nSelect the best implementation (reply with just the letter):");

    let selector_model = config
        .selector_model
        .clone()
        .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

    let request = crate::providers::MessagesRequest {
        model: selector_model,
        messages: vec![crate::providers::Message {
            role: crate::providers::Role::User,
            content: crate::providers::MessageContent::Text(format!(
                "{}\n\n{}",
                selector_prompt_override.unwrap_or(&config.selector_prompt),
                selection_prompt
            )),
        }],
        system_prompt: None,
        tools: vec![],
        max_tokens: Some(10),
        temperature: Some(0.0),
        thinking: None,
        prompt_caching: true,
    };

    let mut stream = provider
        .send(request)
        .await
        .map_err(|e| BestOfNError::ProviderError(e.to_string()))?;

    let mut response = String::new();
    use futures::StreamExt;
    while let Some(event) = stream.next().await {
        if let Ok(crate::providers::StreamEvent::TextDelta { text }) = event {
            response.push_str(&text);
        }
    }

    let response_upper = response.trim().to_uppercase();
    let selected = response_upper
        .chars()
        .find(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase())
        .and_then(|c| {
            let index = (c as u8 - b'A') as usize;
            (index < variants.len()).then_some(index)
        })
        .unwrap_or(0);

    Ok((selected, Some(response)))
}
