use reedline::{Prompt, PromptEditMode, PromptHistorySearch};
use std::borrow::Cow;

pub(crate) struct MyPrompt;

impl Prompt for MyPrompt {
    fn render_prompt_left(&self) -> Cow<str> {
        "tau$ ".into()
    }

    fn render_prompt_right(&self) -> Cow<str> {
        "".into()
    }

    fn render_prompt_indicator(&self, _: PromptEditMode) -> Cow<str> {
        "".into()
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<str> {
        "... ".into()
    }

    fn render_prompt_history_search_indicator(&self, _: PromptHistorySearch) -> Cow<str> {
        "".into()
    }
}
