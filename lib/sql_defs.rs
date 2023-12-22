pub const DB_PATH: &str = "glyfi.db";

/// What challenge a submission belongs to.
#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum Challenge {
    Glyph = 0,
    Ambigram = 1,
}

/// TODO: Figure out how to handle extended weeks properly,
///       if we want them at all.
///
/// Determines what kind of actions should be taken in a week.
///
/// Every week, we need to perform the following actions for
/// each challenge:
///
/// - Make an announcement post that describes that week’s challenge.
/// - Post a panel containing all submissions from the previous week.
/// - Post the top 3 submissions from the week before that.
///
/// Some weeks, however, are special in that we don’t want to take
/// one or more of those actions. A week can either be
///
/// - regular,
/// - special, or
/// - extended.
///
/// At any point in time, up to three weeks overlap. That is, at
/// the ‘beginning’ of the week (that is, the day the announcement
/// is made) we need to:
///
/// - Make a new announcement post for the current week, unless the
///   last week was extended or this week is special.
///
/// - Post a panel containing all submissions from the previous week,
///   unless that week was extended or special.
///
/// - Post the top three from the week before the last, unless that
///   week was extended.
#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum Week {
    Regular = 0,
    Special = 1,
    Extended = 2,
}