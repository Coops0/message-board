use rand::prelude::SliceRandom;
use std::cell::LazyCell;

const WORDS_STRING_LIST: &str = include_str!("../assets/all_english_words_clean.txt");
#[allow(clippy::declare_interior_mutable_const)]
const WORDS_ARRAY: LazyCell<Vec<&str>> = LazyCell::new(|| WORDS_STRING_LIST.lines().collect());

pub fn generate_code() -> String {
    let mut rng = rand::thread_rng();

    #[allow(clippy::borrow_interior_mutable_const)]
    let words = WORDS_ARRAY
        .choose_multiple(&mut rng, 2)
        .map(ToString::to_string)
        .collect::<Vec<String>>();

    let [first_word, second_word] = &words[..] else {
        unreachable!();
    };

    format!("{first_word}.{second_word}")
}
