//! Cue sheets (PLAN.md O5): the text that splits a single-file album into
//! its tracks, from a `.cue` file next to the audio or a CUESHEET tag. Only
//! what playing the tracks needs is read: the album's title and performer,
//! and each track's number, title, performer and where it starts (its
//! INDEX 01).

/// A parsed cue sheet.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CueSheet {
    pub title: Option<String>,
    pub performer: Option<String>,
    pub files: Vec<CueFile>,
}

/// The tracks in one audio file the sheet names.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CueFile {
    /// As written in the sheet, which often names the WAV it was ripped to
    /// rather than the FLAC it became.
    pub name: String,
    pub tracks: Vec<CueTrack>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CueTrack {
    pub number: u32,
    pub title: Option<String>,
    pub performer: Option<String>,
    /// Seconds from the start of the file (INDEX 01).
    pub start: f64,
}

/// A track of an audio file, from its cue sheet: where it starts and ends
/// (`None` at the end of the file).
#[derive(Debug, Clone, PartialEq)]
pub struct CuePart {
    pub number: u32,
    pub title: Option<String>,
    pub performer: Option<String>,
    pub start: f64,
    pub end: Option<f64>,
}

/// Cue sheet bytes as text: UTF-8 (with or without a byte order mark), or
/// else Windows-1252, the legacy code page most rippers used.
pub fn decode(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => bytes.iter().map(|&byte| windows_1252(byte)).collect(),
    }
}

fn windows_1252(byte: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    match byte {
        0x80..=0x9f => HIGH[usize::from(byte - 0x80)],
        _ => char::from(byte),
    }
}

/// Parses a cue sheet, leniently: unknown commands and malformed lines are
/// skipped. Returns `None` if it names no track with a start.
pub fn parse(text: &str) -> Option<CueSheet> {
    let mut sheet = CueSheet::default();
    let mut track: Option<CueTrack> = None;

    let finish = |track: &mut Option<CueTrack>, sheet: &mut CueSheet| {
        if let Some(track) = track.take().filter(|track| track.start >= 0.0) {
            if let Some(file) = sheet.files.last_mut() {
                file.tracks.push(track);
            }
        }
    };

    for line in text.lines() {
        let (command, rest) = split_word(line.trim());
        match command.to_ascii_uppercase().as_str() {
            "FILE" => {
                finish(&mut track, &mut sheet);
                let (name, _) = split_value(rest);
                sheet.files.push(CueFile {
                    name,
                    tracks: Vec::new(),
                });
            }
            "TRACK" => {
                finish(&mut track, &mut sheet);
                let (number, kind) = split_word(rest);
                if kind.trim().eq_ignore_ascii_case("AUDIO") {
                    if let Ok(number) = number.parse() {
                        track = Some(CueTrack {
                            number,
                            title: None,
                            performer: None,
                            start: -1.0,
                        });
                    }
                }
            }
            "TITLE" | "PERFORMER" => {
                let value = Some(split_value(rest).0).filter(|value| !value.is_empty());
                let upper = command.to_ascii_uppercase();
                match (&mut track, upper.as_str()) {
                    (Some(track), "TITLE") => track.title = value,
                    (Some(track), _) => track.performer = value,
                    (None, "TITLE") => sheet.title = value,
                    (None, _) => sheet.performer = value,
                }
            }
            "INDEX" => {
                let (index, time) = split_word(rest);
                if let (Some(track), Ok(1)) = (&mut track, index.parse::<u32>()) {
                    if let Some(seconds) = parse_time(time.trim()) {
                        track.start = seconds;
                    }
                }
            }
            _ => {}
        }
    }
    finish(&mut track, &mut sheet);
    sheet.files.retain(|file| !file.tracks.is_empty());
    (!sheet.files.is_empty()).then_some(sheet)
}

impl CueSheet {
    /// The tracks of the audio file called `file_name`, in order, with their
    /// ends. The sheet's FILE is matched by name, then by name without the
    /// extension (a rip to WAV, compressed later); a sheet with a single
    /// FILE is taken to be about the file whatever it names.
    pub fn parts_for(&self, file_name: &str) -> Vec<CuePart> {
        let stem = |name: &str| {
            let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
            name.rsplit_once('.')
                .map_or(name, |(stem, _)| stem)
                .to_lowercase()
        };
        let wanted = file_name.to_lowercase();
        let file = self
            .files
            .iter()
            .find(|file| {
                let name = file.name.rsplit(['/', '\\']).next().unwrap_or(&file.name);
                name.to_lowercase() == wanted
            })
            .or_else(|| {
                self.files
                    .iter()
                    .find(|file| stem(&file.name) == stem(file_name))
            })
            .or_else(|| (self.files.len() == 1).then(|| &self.files[0]));
        let Some(file) = file else {
            return Vec::new();
        };
        let mut tracks = file.tracks.clone();
        tracks.sort_by(|a, b| a.start.total_cmp(&b.start));
        tracks.dedup_by(|b, a| a.start == b.start);
        let ends: Vec<Option<f64>> = tracks
            .iter()
            .skip(1)
            .map(|next| Some(next.start))
            .chain([None])
            .collect();
        tracks
            .into_iter()
            .zip(ends)
            .map(|(track, end)| CuePart {
                number: track.number,
                title: track.title,
                performer: track.performer,
                start: track.start,
                end,
            })
            .collect()
    }

    /// Whether the sheet names `file_name` (see `parts_for`).
    #[cfg(test)]
    pub fn names(&self, file_name: &str) -> bool {
        !self.parts_for(file_name).is_empty()
    }
}

/// The first word of `text`, and the rest.
fn split_word(text: &str) -> (&str, &str) {
    let text = text.trim_start();
    match text.find(char::is_whitespace) {
        Some(end) => (&text[..end], &text[end..]),
        None => (text, ""),
    }
}

/// A value, quoted or not, and what follows it.
fn split_value(text: &str) -> (String, &str) {
    let text = text.trim_start();
    if let Some(quoted) = text.strip_prefix('"') {
        match quoted.find('"') {
            Some(end) => (quoted[..end].to_owned(), &quoted[end + 1..]),
            None => (quoted.trim_end().to_owned(), ""),
        }
    } else {
        let (word, rest) = split_word(text);
        (word.to_owned(), rest)
    }
}

/// "mm:ss:ff" (75 frames a second) as seconds.
fn parse_time(text: &str) -> Option<f64> {
    let mut parts = text.split(':').map(|part| part.parse::<u32>().ok());
    let (minutes, seconds, frames) = (parts.next()??, parts.next()??, parts.next()??);
    if parts.next().is_some() || seconds >= 60 || frames >= 75 {
        return None;
    }
    Some(f64::from(minutes * 60 + seconds) + f64::from(frames) / 75.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHEET: &str = r#"REM GENRE Jazz
PERFORMER "The Quartet"
TITLE "Live at the Hall"
FILE "Live at the Hall.wav" WAVE
  TRACK 01 AUDIO
    TITLE "Opening"
    INDEX 01 00:00:00
  TRACK 02 AUDIO
    TITLE "Blues for Café"
    PERFORMER "The Quartet with Guest"
    INDEX 00 03:59:70
    INDEX 01 04:02:37
  TRACK 03 AUDIO
    TITLE Encore
    INDEX 01 10:00:00
"#;

    #[test]
    fn parses_the_album_and_its_tracks() {
        let sheet = parse(SHEET).unwrap();
        assert_eq!(sheet.title.as_deref(), Some("Live at the Hall"));
        assert_eq!(sheet.performer.as_deref(), Some("The Quartet"));
        assert_eq!(sheet.files.len(), 1);
        let parts = sheet.parts_for("Live at the Hall.flac");
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].start, 0.0);
        assert_eq!(parts[0].end, Some(242.0 + 37.0 / 75.0));
        assert_eq!(parts[1].title.as_deref(), Some("Blues for Café"));
        assert_eq!(
            parts[1].performer.as_deref(),
            Some("The Quartet with Guest")
        );
        assert_eq!(parts[1].start, 242.0 + 37.0 / 75.0);
        assert_eq!(parts[2].title.as_deref(), Some("Encore"));
        assert_eq!((parts[2].start, parts[2].end), (600.0, None));
        assert_eq!(parts[2].number, 3);
    }

    #[test]
    fn matches_files_by_name_or_stem() {
        let sheet = parse(
            "FILE \"a.flac\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\n\
             FILE \"b.wav\" WAVE\nTRACK 02 AUDIO\nINDEX 01 00:00:00\nTRACK 03 AUDIO\nINDEX 01 01:00:00\n",
        )
        .unwrap();
        assert_eq!(sheet.parts_for("A.FLAC").len(), 1);
        assert_eq!(sheet.parts_for("b.flac").len(), 2);
        assert!(sheet.parts_for("c.flac").is_empty());
        assert!(sheet.names("b.flac"));
        // With one FILE, whatever it's called.
        assert_eq!(parse(SHEET).unwrap().parts_for("rip.flac").len(), 3);
    }

    #[test]
    fn skips_what_it_cant_read() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("TITLE \"Nothing\"\n"), None);
        // No FILE, a data track, a bad time.
        assert_eq!(parse("TRACK 01 AUDIO\nINDEX 01 00:00:00\n"), None);
        let sheet = parse(
            "FILE x.wav WAVE\nTRACK 01 MODE1/2352\nINDEX 01 00:00:00\n\
             TRACK 02 AUDIO\nINDEX 01 00:61:00\nTRACK 03 AUDIO\nINDEX 01 02:00:74\n",
        )
        .unwrap();
        let parts = sheet.parts_for("x.flac");
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].number, 3);
    }

    #[test]
    fn decodes_utf8_and_windows_1252() {
        assert_eq!(decode("\u{feff}Café".as_bytes()), "Café");
        assert_eq!(decode(b"Caf\xe9 \x93Live\x94"), "Café “Live”");
    }
}
