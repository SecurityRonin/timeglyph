//! Whole-world public-holiday lookup, behind the `holiday` feature.
//!
//! The data is an embedded, gzipped export of the MIT-licensed
//! [python-holidays](https://github.com/vacanza/holidays) project covering
//! 1980–2100 (see `data/generate_holidays.py` and `data/README.md`). Names are
//! in each country's default locale, exactly as python-holidays emits them.
//!
//! Epistemics: a hit means the date *is consistent with a public holiday* in
//! that country per the reference data — an annotation, not proof the day was
//! observed at a given place. Coverage varies by country (python-holidays
//! supports different year ranges per locale), so a `None` means "no holiday in
//! the covered data", not "provably an ordinary day".

// GREEN implementation lands in the next commit; this stub compiles so the
// oracle-validated tests in tests/holiday.rs can run and fail (RED).
/// Public-holiday name for `date` in `country` (ISO-3166 alpha-2, uppercase), or
/// `None` when that date is not a holiday there / is outside the country's
/// covered range.
#[must_use]
pub fn lookup(country: &str, date: jiff::civil::Date) -> Option<String> {
    let _ = (country, date);
    None
}
