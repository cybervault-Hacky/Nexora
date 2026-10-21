//! A prompt budget that does not need the model's tokenizer.
//!
//! The runtime will accept a prompt that is longer than its context window and
//! quietly drop the oldest tokens. The model then answers with part of the
//! conversation missing, which looks like a model mistake rather than a limit.
//! Nexora refuses that request instead, before the engine is asked to work and
//! before any usage is reserved.
//!
//! The estimate is deliberately conservative, and it is only used to refuse a
//! prompt that cannot fit even under generous assumptions:
//!
//! * ASCII text is counted at one token per four characters, the usual rate for
//!   English and source code.
//! * Every non-ASCII character counts as two tokens. Latin accents, CJK and
//!   emoji all tokenize at or above one token per character; two is an upper
//!   bound for the common cases, so the guard errs towards allowing a request.
//! * Each message costs a few tokens of chat framing.
//!
//! This is a safety bound, not a measurement. Nothing here is reported to the
//! user as a token count; only a refusal uses these numbers, and its wording
//! says "about".

/// Tokens reserved for the system prompt the client prepends.
pub const SYSTEM_PROMPT_TOKENS: usize = 96;

/// Tokens of framing per chat turn.
const PER_TURN_TOKENS: usize = 4;

/// Conservative token estimate for one piece of text.
pub fn estimate_tokens(text: &str) -> usize {
    let mut ascii = 0usize;
    let mut wide = 0usize;

    for character in text.chars() {
        if character.is_ascii() {
            ascii += 1;
        } else {
            wide += 1;
        }
    }

    // Rounded up: a trailing partial group still costs a token.
    ascii.div_ceil(4) + wide * 2
}

/// Conservative estimate for a whole conversation, including framing.
pub fn conversation_tokens<'a>(contents: impl Iterator<Item = &'a str>) -> usize {
    let mut total = SYSTEM_PROMPT_TOKENS;
    let mut turns = 0usize;

    for content in contents {
        total += estimate_tokens(content);
        turns += 1;
    }

    total + turns * PER_TURN_TOKENS
}

/// What a request would need, and what the context actually offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overflow {
    pub prompt_tokens: usize,
    pub reply_tokens: u32,
    pub context_size: u32,
}

/// Returns `Some` when the prompt and the reply allowance cannot both fit.
///
/// The reply allowance is the request's `max_tokens`, because that is what the
/// runtime is permitted to generate on top of the prompt.
pub fn overflow(prompt_tokens: usize, reply_tokens: u32, context_size: u32) -> Option<Overflow> {
    let needed = prompt_tokens + reply_tokens as usize;

    (needed > context_size as usize).then_some(Overflow {
        prompt_tokens,
        reply_tokens,
        context_size,
    })
}

impl Overflow {
    /// A plain-language explanation for the composer.
    pub fn message(&self) -> String {
        format!(
            "This conversation needs about {} tokens, and the reply can take up to {}. The model is configured for {} tokens of context, so part of the conversation would be dropped. Start a new chat, or raise the context window in model settings. Nothing was sent.",
            self.prompt_tokens, self.reply_tokens, self.context_size
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_estimated_at_four_characters_per_token() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abc"), 1);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
        assert_eq!(estimate_tokens(&"a".repeat(4_000)), 1_000);
    }

    #[test]
    fn non_ascii_is_estimated_conservatively() {
        // One CJK character is at least one token, and is counted as two.
        assert_eq!(estimate_tokens("日本"), 4);
        assert_eq!(estimate_tokens("é"), 2);
        // A mixed string adds both halves.
        assert_eq!(estimate_tokens("abcdé"), 3);
    }

    #[test]
    fn a_conversation_includes_the_system_prompt_and_framing() {
        let tokens = conversation_tokens(["abcd", "efgh"].into_iter());

        assert_eq!(tokens, SYSTEM_PROMPT_TOKENS + 1 + 1 + 2 * PER_TURN_TOKENS);
    }

    #[test]
    fn an_empty_conversation_still_reserves_the_system_prompt() {
        assert_eq!(
            conversation_tokens(std::iter::empty()),
            SYSTEM_PROMPT_TOKENS
        );
    }

    #[test]
    fn a_prompt_that_fits_is_allowed() {
        assert_eq!(overflow(1_000, 1_024, 8_192), None);
    }

    #[test]
    fn an_exact_fit_is_allowed() {
        assert_eq!(overflow(7_168, 1_024, 8_192), None);
    }

    #[test]
    fn a_prompt_that_cannot_fit_is_reported() {
        let reported = overflow(8_000, 1_024, 8_192).expect("over the limit");

        assert_eq!(reported.prompt_tokens, 8_000);
        assert_eq!(reported.reply_tokens, 1_024);
        assert_eq!(reported.context_size, 8_192);
        assert!(reported.message().contains("8192"));
        assert!(reported.message().contains("Nothing was sent"));
    }

    #[test]
    fn the_smallest_supported_context_still_allows_a_short_exchange() {
        // 2 048 tokens is the app's minimum; a short prompt plus a small reply
        // must not be refused there.
        let prompt =
            conversation_tokens(["Explain this function.".to_string().as_str()].into_iter());

        assert_eq!(overflow(prompt, 256, 2_048), None);
    }

    #[test]
    fn a_long_conversation_at_the_minimum_context_is_refused() {
        let long = "x".repeat(20_000);
        let prompt = conversation_tokens([long.as_str()].into_iter());

        assert!(overflow(prompt, 1_024, 2_048).is_some());
    }
}
