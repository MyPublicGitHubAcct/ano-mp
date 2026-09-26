//! Scoring how well a release matches an album in the library, and deciding
//! whether the best one is good enough to accept without asking the user.
//! Pure functions over `AlbumFacts` and `Release`.

use super::musicbrainz::Release;
use crate::library::sort_key::fold;

/// What the library knows about an album, from its tags.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AlbumFacts {
    pub title: String,
    /// The album artist; `None` if no track has one or an artist.
    pub artist: Option<String>,
    /// The earliest year among its tracks.
    pub year: Option<u32>,
    /// How many tracks the album has: the tagged track totals where there
    /// are any, else the tracks in the library.
    pub track_count: u32,
    /// The library's tracks' durations, in seconds.
    pub durations: Vec<f64>,
}

/// A score at least this high is accepted automatically…
pub const ACCEPT_SCORE: f64 = 0.85;
/// …if no release of another release group comes within this of it.
pub const ACCEPT_LEAD: f64 = 0.05;
/// Below this a release isn't offered for review either.
pub const REVIEW_SCORE: f64 = 0.5;

/// Tracks whose lengths differ by at most this match.
const DURATION_TOLERANCE: f64 = 3.0;

// How much each comparison counts. Comparisons that can't be made (no year
// in the tags, no track list in a search result) are left out and the rest
// reweighted.
const TITLE_WEIGHT: f64 = 0.3;
const ARTIST_WEIGHT: f64 = 0.2;
const TRACK_COUNT_WEIGHT: f64 = 0.2;
const DURATIONS_WEIGHT: f64 = 0.25;
const YEAR_WEIGHT: f64 = 0.05;

/// How well `release` matches `album`, from 0 to 1.
pub fn score(album: &AlbumFacts, release: &Release) -> f64 {
    let title = title_similarity(&album.title, &release.title);
    let artist = artist_similarity(album.artist.as_deref(), &release.artist);
    let mut parts: Vec<(f64, f64)> = vec![
        (TITLE_WEIGHT, title),
        (
            TRACK_COUNT_WEIGHT,
            count_similarity(album.track_count, release.track_count),
        ),
    ];
    if let Some(artist) = artist {
        parts.push((ARTIST_WEIGHT, artist));
    }
    if !album.durations.is_empty() && !release.tracks.is_empty() {
        let lengths: Vec<f64> = release
            .tracks
            .iter()
            .filter_map(|track| track.length_ms.map(|ms| f64::from(ms) / 1000.0))
            .collect();
        if !lengths.is_empty() {
            parts.push((
                DURATIONS_WEIGHT,
                durations_matched(&album.durations, &lengths),
            ));
        }
    }
    if let Some(year) = album.year {
        let closeness = |other: Option<u32>| -> f64 {
            match other {
                Some(other) if other == year => 1.0,
                Some(other) if other.abs_diff(year) == 1 => 0.5,
                _ => 0.0,
            }
        };
        // Tags carry the original year or the reissue's; either will do.
        let best = closeness(release.year()).max(closeness(release.original_year()));
        if release.year().is_some() || release.original_year().is_some() {
            parts.push((YEAR_WEIGHT, best));
        }
    }
    let weights: f64 = parts.iter().map(|(weight, _)| weight).sum();
    let average = parts
        .iter()
        .map(|(weight, value)| weight * value)
        .sum::<f64>()
        / weights;
    // A different title or artist is a different album, however well the
    // rest agrees (another album by the same band, a same-titled "Greatest
    // Hits" by someone else).
    average * gate(title) * artist.map_or(1.0, gate)
}

/// 1 for a similarity of at least 0.7, falling to 0 at 0.3.
fn gate(similarity: f64) -> f64 {
    ((similarity - 0.3) / 0.4).clamp(0.0, 1.0)
}

/// A scored release.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub release: Release,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    /// Good enough to accept.
    Matched(Candidate),
    /// The best candidate, for the user to confirm.
    Review(Candidate),
    NoMatch,
}

/// Sorts `candidates` best first and decides on the first. Releases of one
/// release group (the same album issued in several countries or years)
/// score alike, and that is agreement, not doubt: only a close runner-up
/// from another release group holds a match back for review. Ties go to a
/// release with front cover art, then the earliest.
pub fn decide(candidates: &mut [Candidate]) -> Decision {
    candidates.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| {
                b.release
                    .has_front_art
                    .unwrap_or(false)
                    .cmp(&a.release.has_front_art.unwrap_or(false))
            })
            .then_with(|| {
                // Missing dates last.
                let date = |c: &Candidate| (c.release.date.is_none(), c.release.date.clone());
                date(a).cmp(&date(b))
            })
    });
    let Some(best) = candidates.first() else {
        return Decision::NoMatch;
    };
    let runner_up = candidates.iter().skip(1).find(|candidate| {
        candidate.release.release_group_id.is_none()
            || candidate.release.release_group_id != best.release.release_group_id
    });
    let clear_lead = runner_up.is_none_or(|other| best.score - other.score >= ACCEPT_LEAD);
    if best.score >= ACCEPT_SCORE && clear_lead {
        Decision::Matched(best.clone())
    } else if best.score >= REVIEW_SCORE {
        Decision::Review(best.clone())
    } else {
        Decision::NoMatch
    }
}

/// Similarity of two titles, ignoring case, accents and punctuation, and
/// counting a title with a bracketed suffix ("… (Remastered)", "… [Deluxe
/// Edition]") as nearly the same as one without.
pub fn title_similarity(a: &str, b: &str) -> f64 {
    let full = similarity(&normalize(a), &normalize(b));
    let stripped = similarity(&normalize(strip_suffix(a)), &normalize(strip_suffix(b)));
    full.max(stripped * 0.95)
}

/// Similarity of the album's artist and a release's credit, or `None` if
/// the album has no artist to compare. "Various Artists" and its spellings
/// are all alike.
fn artist_similarity(album: Option<&str>, release: &str) -> Option<f64> {
    let album = album?;
    if is_various(album) || is_various(release) {
        return Some(if is_various(album) == is_various(release) {
            1.0
        } else {
            0.0
        });
    }
    Some(title_similarity(album, release))
}

fn is_various(artist: &str) -> bool {
    matches!(
        normalize(artist).as_str(),
        "various artists" | "various" | "va" | "v a" | "verschiedene interpreten"
    )
}

/// 1 when equal, falling off with the difference relative to the larger.
fn count_similarity(a: u32, b: u32) -> f64 {
    if a == b {
        return 1.0;
    }
    1.0 - f64::from(a.abs_diff(b)) / f64::from(a.max(b))
}

/// The share of the album's durations that a release track's length
/// matches, each release track used once.
fn durations_matched(durations: &[f64], lengths: &[f64]) -> f64 {
    let mut unused: Vec<f64> = lengths.to_vec();
    let mut matched = 0;
    for duration in durations {
        let nearest = unused
            .iter()
            .enumerate()
            .map(|(index, length)| (index, (length - duration).abs()))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((index, difference)) = nearest {
            if difference <= DURATION_TOLERANCE {
                unused.swap_remove(index);
                matched += 1;
            }
        }
    }
    matched as f64 / durations.len() as f64
}

/// Folded (lowercase, no accents), with punctuation as spaces and runs of
/// spaces collapsed, and "&" read as "and".
fn normalize(text: &str) -> String {
    let folded = fold(&text.replace('&', " and "));
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `text` without a trailing "(…)" or "[…]".
fn strip_suffix(text: &str) -> &str {
    let trimmed = text.trim_end();
    for (open, close) in [('(', ')'), ('[', ']')] {
        if trimmed.ends_with(close) {
            if let Some(start) = trimmed.rfind(open) {
                if start > 0 {
                    return trimmed[..start].trim_end();
                }
            }
        }
    }
    trimmed
}

/// 1 minus the edit distance relative to the longer string; 1 for two
/// empty strings.
fn similarity(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let longest = a.len().max(b.len());
    if longest == 0 {
        return 1.0;
    }
    1.0 - levenshtein(&a, &b) as f64 / longest as f64
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != cb);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::musicbrainz::{fixtures, parse_release, ReleaseTrack};

    /// "In Rainbows" as a library would have it: ten tracks whose durations
    /// are the 2016 release's, plus up to a second of encoder padding.
    fn in_rainbows() -> AlbumFacts {
        AlbumFacts {
            title: "In Rainbows".into(),
            artist: Some("Radiohead".into()),
            year: Some(2007),
            track_count: 10,
            durations: [
                237.4, 242.9, 255.6, 318.5, 228.3, 129.8, 290.5, 328.9, 248.6, 279.2,
            ]
            .into(),
        }
    }

    fn release(title: &str, artist: &str, group: &str, tracks: u32) -> Release {
        Release {
            id: format!("{title}-{group}"),
            title: title.into(),
            artist: artist.into(),
            artist_ids: vec![],
            date: None,
            country: None,
            status: None,
            barcode: None,
            labels: vec![],
            release_group_id: Some(group.into()),
            release_type: None,
            secondary_types: vec![],
            first_release_date: None,
            genres: vec![],
            track_count: tracks,
            has_front_art: None,
            tracks: vec![],
        }
    }

    fn candidate(release: Release, score: f64) -> Candidate {
        Candidate { release, score }
    }

    #[test]
    fn compares_titles_loosely() {
        assert_eq!(title_similarity("In Rainbows", "in rainbows"), 1.0);
        assert_eq!(title_similarity("Björk: Début", "Bjork - Debut"), 1.0);
        assert_eq!(
            title_similarity("Simon & Garfunkel", "Simon and Garfunkel"),
            1.0
        );
        assert!(title_similarity("OK Computer (Remastered)", "OK Computer") >= 0.95);
        assert!(title_similarity("Kid A [Deluxe]", "Kid A (Collector's Edition)") >= 0.95);
        assert!(title_similarity("Amnesiac", "Hail to the Thief") < 0.3);
        assert_eq!(strip_suffix("(What's the Story)"), "(What's the Story)");
        assert_eq!(similarity("", ""), 1.0);
        assert_eq!(levenshtein(&['a', 'b'], &['b']), 1);
    }

    #[test]
    fn compares_artists_and_counts() {
        assert_eq!(
            artist_similarity(Some("Various Artists"), "Various Artists"),
            Some(1.0)
        );
        assert_eq!(artist_similarity(Some("VA"), "Various Artists"), Some(1.0));
        assert_eq!(
            artist_similarity(Some("Radiohead"), "Various Artists"),
            Some(0.0)
        );
        assert_eq!(artist_similarity(None, "Radiohead"), None);
        assert_eq!(count_similarity(10, 10), 1.0);
        assert_eq!(count_similarity(8, 10), 0.8);
        assert_eq!(
            durations_matched(&[100.0, 100.0, 200.0], &[101.0, 250.0]),
            1.0 / 3.0
        );
    }

    #[test]
    fn scores_the_recorded_releases_highly() {
        let album = in_rainbows();
        for (id, json) in fixtures::RELEASES {
            let release = parse_release(json).unwrap();
            let score = score(&album, &release);
            assert!(score >= ACCEPT_SCORE, "{id}: {score}");
        }
        // A different album by the same band, or the same title by another
        // artist, doesn't come close.
        let other = release("Amnesiac", "Radiohead", "g2", 11);
        assert!(
            score(&album, &other) < REVIEW_SCORE,
            "{}",
            score(&album, &other)
        );
        let other = release("In Rainbows", "Tony Bennett", "g3", 10);
        assert!(
            score(&album, &other) < REVIEW_SCORE,
            "{}",
            score(&album, &other)
        );
        assert_eq!(gate(0.2), 0.0);
        assert_eq!(gate(0.5), 0.5);
        assert_eq!(gate(0.9), 1.0);
    }

    #[test]
    fn leaves_out_what_cannot_be_compared() {
        let album = AlbumFacts {
            title: "In Rainbows".into(),
            track_count: 10,
            ..AlbumFacts::default()
        };
        let mut hit = release("In Rainbows", "Radiohead", "g", 10);
        assert_eq!(score(&album, &hit), 1.0, "only title and count are known");
        hit.tracks = vec![ReleaseTrack {
            disc: 1,
            position: 1,
            title: "15 Step".into(),
            length_ms: None,
            recording_id: None,
        }];
        assert_eq!(score(&album, &hit), 1.0, "no lengths to compare");
    }

    #[test]
    fn a_partial_album_or_wrong_durations_scores_lower() {
        let full = parse_release(fixtures::RELEASES[1].1).unwrap();
        let mut partial = in_rainbows();
        partial.durations.truncate(4);
        partial.track_count = 4;
        let mut wrong = in_rainbows();
        wrong.durations = vec![60.0; 10];
        let whole = score(&in_rainbows(), &full);
        assert!(score(&partial, &full) < whole);
        assert!(score(&wrong, &full) < ACCEPT_SCORE);
    }

    #[test]
    fn same_group_ties_are_agreement_but_other_groups_need_review() {
        let mut with_art = release("A", "B", "g1", 10);
        with_art.has_front_art = Some(true);
        with_art.date = Some("2010".into());
        let mut earlier = release("A", "B", "g1", 10);
        earlier.date = Some("2001".into());
        let mut candidates = vec![
            candidate(earlier.clone(), 0.95),
            candidate(with_art.clone(), 0.95),
        ];
        assert_eq!(
            decide(&mut candidates),
            Decision::Matched(candidate(with_art.clone(), 0.95))
        );

        // Without art either way, the earliest wins.
        let mut undated = earlier.clone();
        undated.date = None;
        let mut candidates = vec![candidate(undated, 0.9), candidate(earlier.clone(), 0.9)];
        assert_eq!(
            decide(&mut candidates),
            Decision::Matched(candidate(earlier.clone(), 0.9))
        );

        let rival = release("A", "B", "g2", 10);
        let mut close = vec![
            candidate(earlier.clone(), 0.95),
            candidate(rival.clone(), 0.92),
        ];
        assert!(matches!(decide(&mut close), Decision::Review(_)));
        let mut clear = vec![candidate(earlier.clone(), 0.95), candidate(rival, 0.85)];
        assert!(matches!(decide(&mut clear), Decision::Matched(_)));

        assert!(matches!(
            decide(&mut [candidate(earlier.clone(), 0.7)]),
            Decision::Review(_)
        ));
        assert_eq!(decide(&mut [candidate(earlier, 0.3)]), Decision::NoMatch);
        assert_eq!(decide(&mut []), Decision::NoMatch);
    }
}
