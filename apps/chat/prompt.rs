use reedline::{Prompt, PromptEditMode, PromptHistorySearch};
use std::borrow::Cow;

pub(crate) struct MyPrompt;

impl Prompt for MyPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        "tau> ".into()
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        "".into()
    }

    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<'_, str> {
        "".into()
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        "... ".into()
    }

    fn render_prompt_history_search_indicator(&self, _: PromptHistorySearch) -> Cow<'_, str> {
        "".into()
    }
}
