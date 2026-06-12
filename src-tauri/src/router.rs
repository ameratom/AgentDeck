//! Priority-ordered handoff routing rules.

use crate::models::{HandoffRouteRequest, HandoffRouteSuggestion, RouterRule};

pub fn suggest_route(
    rules: &[RouterRule],
    request: &HandoffRouteRequest,
) -> Option<HandoffRouteSuggestion> {
    let haystack = format!(
        "{} {}",
        request.title.trim(),
        request.task.trim()
    )
    .to_lowercase();

    let mut candidates: Vec<&RouterRule> = rules
        .iter()
        .filter(|rule| rule.enabled)
        .collect();
    candidates.sort_by_key(|rule| rule.priority);

    for rule in candidates {
        if let Some(source) = rule.source_agent_id.as_deref() {
            if source.trim().is_empty() {
                continue;
            }
            if request.source_agent_id != source {
                continue;
            }
        }

        if let Some(keyword) = rule.keyword.as_deref() {
            let needle = keyword.trim().to_lowercase();
            if needle.is_empty() || !haystack.contains(&needle) {
                continue;
            }
        }

        let reason = match (&rule.source_agent_id, &rule.keyword) {
            (Some(source), Some(keyword))
                if !source.trim().is_empty() && !keyword.trim().is_empty() =>
            {
                format!(
                    "Matched source {source} and keyword \"{}\".",
                    keyword.trim()
                )
            }
            (Some(source), _) if !source.trim().is_empty() => {
                format!("Matched source {source}.")
            }
            (_, Some(keyword)) if !keyword.trim().is_empty() => {
                format!("Matched keyword \"{}\".", keyword.trim())
            }
            _ => "Matched unconditional router rule.".to_owned(),
        };

        return Some(HandoffRouteSuggestion {
            rule_id: rule.id.clone(),
            rule_name: rule.name.clone(),
            target_provider_id: rule.target_provider_id.clone(),
            target_model_id: rule.target_model_id.clone(),
            reason,
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rule(
        id: &str,
        priority: i32,
        source: Option<&str>,
        keyword: Option<&str>,
        provider: &str,
    ) -> RouterRule {
        RouterRule {
            id: id.to_owned(),
            priority,
            name: id.to_owned(),
            enabled: true,
            source_agent_id: source.map(str::to_owned),
            keyword: keyword.map(str::to_owned),
            target_provider_id: provider.to_owned(),
            target_model_id: None,
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn matches_keyword_before_lower_priority_rule() {
        let rules = vec![
            sample_rule("local", 1, None, Some("review"), "lm-studio"),
            sample_rule("cloud", 2, None, Some("research"), "xai"),
        ];
        let request = HandoffRouteRequest {
            source_agent_id: "agent:claude-code".to_owned(),
            title: "Review diff".to_owned(),
            task: "Review this changed file.".to_owned(),
        };
        let suggestion = suggest_route(&rules, &request).expect("suggestion");
        assert_eq!(suggestion.target_provider_id, "lm-studio");
    }

    #[test]
    fn respects_source_agent_filter() {
        let rules = vec![sample_rule(
            "grok-only",
            0,
            Some("agent:grok"),
            Some("research"),
            "xai",
        )];
        let mismatch = HandoffRouteRequest {
            source_agent_id: "agent:codex".to_owned(),
            title: "Research".to_owned(),
            task: "Find current docs.".to_owned(),
        };
        assert!(suggest_route(&rules, &mismatch).is_none());
    }
}