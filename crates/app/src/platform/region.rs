//! Where the person is, as their system says: the country of its locale.
//!
//! Asked by the music stores that are one store a country, where a song
//! sold in one may not be found in another.

/// The two letter code of the system locale's country, upper case: "TR"
/// for `tr-TR`. `None` when the locale names no country.
pub fn country() -> Option<String> {
    #[cfg(windows)]
    let locale = win::locale();
    #[cfg(not(windows))]
    let locale = std::env::var("LANG").ok();
    locale.as_deref().and_then(country_of)
}

/// The country in a locale's name: `tr-TR`, `en_GB.UTF-8`, `zh-Hans-CN`.
fn country_of(locale: &str) -> Option<String> {
    let locale = locale.split(['.', '@']).next()?;
    locale
        .split(['-', '_'])
        .skip(1)
        .find(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()))
        .map(str::to_ascii_uppercase)
}

#[cfg(windows)]
mod win {
    use windows::Win32::Globalization::GetUserDefaultLocaleName;

    /// The longest locale name there is, its terminator counted
    /// (`LOCALE_NAME_MAX_LENGTH`).
    const MOST: usize = 85;

    pub(super) fn locale() -> Option<String> {
        let mut name = [0u16; MOST];
        // SAFETY: the buffer is as long as the call is told it is.
        let written = unsafe { GetUserDefaultLocaleName(&mut name) };
        let length = usize::try_from(written).ok()?.checked_sub(1)?;
        String::from_utf16(&name[..length]).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_country_is_read_from_any_way_a_locale_is_written() {
        assert_eq!(country_of("tr-TR").as_deref(), Some("TR"));
        assert_eq!(country_of("en_GB.UTF-8").as_deref(), Some("GB"));
        assert_eq!(country_of("zh-Hans-CN").as_deref(), Some("CN"));
        assert_eq!(country_of("en"), None);
        assert_eq!(country_of(""), None);
    }
}
