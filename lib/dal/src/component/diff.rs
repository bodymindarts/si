//! This module contains [`ComponentDiff`].

use serde::{Deserialize, Serialize};

use crate::component::ComponentResult;
use crate::{
    CodeLanguage, CodeView, Component, ComponentError, ComponentId, ComponentView,
    ComponentViewProperties, DalContext, StandardModel,
};

const NEWLINE: &str = "\n";

// NOTE(nick): while the destination is the browser, we may want to consider platform-specific
// newline characters.
// #[cfg(target_os != "windows")]
// const NEWLINE: &str = "\n";
// #[cfg(target_os = "windows")]
// const NEWLINE: &str = "\r\n";

/// Contains the "diffs" for a given [`Component`](crate::Component). Generated by
/// [`Self::new()`].
#[derive(Deserialize, Serialize, Debug)]
pub struct ComponentDiff {
    /// The [`Component's`](crate::Component) [`CodeView`](crate::code_view::CodeView) found in the
    /// current [`Visibility`](crate::Visibility).
    pub current: CodeView,
    /// The "diff(s)" between [`Component`](crate::Component)'s
    /// [`CodeViews`](crate::code_view::CodeView) found on _head_ and found in the current
    /// [`Visibility`](crate::Visibility).
    ///
    /// This will be empty if the [`Component`](crate::Component) has been newly added.
    pub diffs: Vec<CodeView>,
}

impl ComponentDiff {
    pub async fn new(ctx: &DalContext, component_id: ComponentId) -> ComponentResult<Self> {
        // We take a clone of the original ctx for comparisons against the head visibility.
        // Importantly, this `head_ctx` will be dropped at the end of this function and will not
        // live any longer (that is, it's garbage collected at a reasonable time)
        let head_ctx = ctx.clone_with_head();

        if ctx.visibility().is_head() || !head_ctx.visibility().is_head() {
            return Err(ComponentError::InvalidContextForDiff);
        }

        let curr_component_view = ComponentView::new(ctx, component_id).await?;
        if curr_component_view.properties.is_null() {
            return Ok(Self {
                current: CodeView::new(CodeLanguage::Json, Some("{}".to_owned())),
                diffs: Vec::new(),
            });
        }

        let mut curr_component_view = ComponentViewProperties::try_from(curr_component_view)?;
        curr_component_view.drop_private();

        let curr_json = serde_json::to_string_pretty(&curr_component_view)?;

        // Find the "diffs" given the head dal context only if the component exists on head.
        let diffs: Vec<CodeView> = if Component::get_by_id(&head_ctx, &component_id)
            .await?
            .is_some()
        {
            let prev_component_view = ComponentView::new(&head_ctx, component_id).await?;
            if prev_component_view.properties.is_null() {
                return Ok(Self {
                    current: CodeView::new(CodeLanguage::Json, Some(curr_json)),
                    diffs: Vec::new(),
                });
            }

            let mut prev_component_view = ComponentViewProperties::try_from(prev_component_view)?;
            prev_component_view.drop_private();

            let prev_json = serde_json::to_string_pretty(&prev_component_view)?;

            let mut lines = Vec::new();
            for diff_object in diff::lines(&prev_json, &curr_json) {
                let line = match diff_object {
                    diff::Result::Left(left) => format!("-{left}"),
                    diff::Result::Both(unchanged, _) => format!(" {unchanged}"),
                    diff::Result::Right(right) => format!("+{right}"),
                };
                lines.push(line);
            }

            // FIXME(nick): generate multiple code views if there are multiple code views.
            let diff = CodeView::new(CodeLanguage::Diff, Some(lines.join(NEWLINE)));
            vec![diff]
        } else {
            vec![]
        };

        Ok(Self {
            current: CodeView::new(CodeLanguage::Json, Some(curr_json)),
            diffs,
        })
    }
}
