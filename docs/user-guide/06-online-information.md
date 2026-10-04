# 6. Online information

Your files' tags say a lot, but not everything. ano-mp can ask online
services for covers, release details, biographies and descriptions, and
can share what you listen to with ListenBrainz if you want it to. This
chapter says what each service adds, what it is sent and when, how to turn
each one off, and how to fix a wrong match.

Everything here can be turned off. With **Use online services** off in
**Settings › Online sources**, ano-mp contacts no music service at all;
details and covers it already has keep showing.

## The services

| Service | What it adds | What it is sent | On at first |
|---|---|---|---|
| **MusicBrainz** (musicbrainz.org) | which release each album is, and the artist behind each name: release dates, labels and catalogue numbers, formats, countries, release types and genres; artists' dates, places and websites; their other releases | album titles, artist names and track lengths, to find each album and artist | yes |
| **Cover Art Archive** (coverartarchive.org) | covers | the MusicBrainz release of each album | yes |
| **Wikipedia** (en.wikipedia.org, through wikidata.org) | artist biographies and album descriptions: the start of the article | the names of Wikipedia articles MusicBrainz links to | yes |
| **Discogs** (discogs.com) | album details: credits, styles, labels and formats | album titles and artist names, with your Discogs token | no: needs your token |
| **ListenBrainz** (listenbrainz.org) | sends your listens to your account; suggests artists you don't have | each listen, with your token; the MusicBrainz ids of the artists you play most | no |

Your files, their names and their paths are never sent to any of them.

The Cover Art Archive and Wikipedia work from MusicBrainz's matches, so
they do nothing while MusicBrainz is off.

**Discogs**: its terms allow keeping only which release an album is. Its
details are fetched each time an album is shown (and kept in memory for a
short while), never stored on your Mac, and shown with a credit linked to
the release's page on Discogs. Its pictures aren't used.

**Wikipedia** text is shared under the Creative Commons BY-SA licence:
ano-mp always names the article and the licence where it shows the text.

## When ano-mp goes online

With **Look up albums and artists automatically** on (it is at first),
ano-mp looks albums and artists up in the background after each scan, and
looks up the album playing. It works slowly, one request at a time, as the
services ask of apps. A large library takes a while to look up the first
time; after that, only new albums are.

With it off, only what you ask for is looked up: **Find Details…** for an
album, **Look up** on an artist's page.

**Online sources** in the sidebar shows what the lookups are doing:
how many it has done (**12 of 40**), **Up to date**, or which services
**can’t be reached**.

### Offline

When the network is down or a service can't be reached, ano-mp pauses
those lookups and carries on by itself when the service is back (**Try
now** in **Settings › Online sources** tries at once). Everything already
fetched keeps showing. Nothing else in the app needs the network.

## Turn services on or off

Open **Settings › Online sources** (or click **Online sources** in the
sidebar).

<!-- Screenshot: Settings › Online sources. -->

- **Use online services** turns every online service on or off at once.
  Covers from your files and folders keep working either way.
- **Look up albums and artists automatically**: above.
- Under **Sources**, each source has its own switch, says whether it is
  **In use**, needs something (**Needs MusicBrainz**, **Personal access
  token needed**) or **Can’t be reached**, and links to its site.
- Under **Order**, each kind of data (**Album details**, **Album art**,
  **Artist biographies**, **Album descriptions**) lists the sources that
  supply it. They are tried from the top; move one with its arrows. A cover
  you chose yourself always comes first.
- **Reset** restores the sources and order ano-mp started with; saved keys
  are kept.

### Add a Discogs token

Discogs needs a **personal access token** from your Discogs account.

1. Sign in at discogs.com and open **Settings › Developers**
   (discogs.com/settings/developers), or click **Get one** beside Discogs
   in **Settings › Online sources**.
2. Generate a token there, and copy it.
3. In ano-mp, paste it into the token field under Discogs and click
   **Save**. ano-mp keeps it in your Mac's keychain, not in its
   settings.
4. Turn Discogs on.

To stop using it, click **Remove** beside the token.

## ListenBrainz

ListenBrainz (listenbrainz.org, run by the MetaBrainz Foundation, as
MusicBrainz is) keeps a record of what people listen to, and suggests
music from it. ano-mp can use it two ways, both off at first, in **Settings
› Features**.

### Send listens to ListenBrainz

1. Make an account at listenbrainz.org, and copy your **user token** from
   its **Settings** page.
2. In ano-mp, turn on **Send listens to ListenBrainz**, paste the token
   into **ListenBrainz user token** and click **Save token**. ano-mp says
   **Connected to ListenBrainz as** your user name.

Each play that counts (half the track, or four minutes) is sent with the
track's title, artist, album and their MusicBrainz ids. Listens made while
you're offline wait and are sent later; Settings shows how many are
waiting. **Listening history** must be on too.

### Recommendations from outside the library

**Recommendations from outside the library** suggests artists you don't
have, like the ones you play most. ano-mp sends ListenBrainz
(labs.api.listenbrainz.org) the MusicBrainz ids of your most-played
artists, and uses MusicBrainz's relations between artists (band members,
collaborations). Settings shows exactly which artists' ids are sent.

The suggestions appear on **Home** (**Beyond your library**), under an
artist's **Similar artists**, and in an artist's **More Like This**. Each
says why it was suggested ("like Nick Drake on ListenBrainz", "with Brian
Eno"). Click a name for links to the artist on **MusicBrainz**,
**ListenBrainz**, their **Website** or **Bandcamp**; ano-mp doesn't play
music it doesn't have. **Not Interested** stops suggesting an artist;
**Suggest the dismissed artists again**, in Settings, undoes that.

## Fixing a wrong match

Matching is automatic and usually right, but an album may have several
releases (an original and a remaster, a CD and a vinyl), and many artists
share a name. You can always correct it, and ano-mp never overrides your
choice.

### An album: Find Details

On the album's page, click **Find Details…** (or choose **Find Details…**
in its menu). The dialog has a tab for each album details source.

<!-- Screenshot: the Find Details dialog with a possible match selected. -->

1. The status line says how the album is matched now: **Matched
   automatically** (with a score), **Matched to the release you chose**, a
   possible match that **needs your review**, or **Not looked up yet**.
2. The search starts with the album's title and artist. Edit either (or
   choose **Any artist**) and click **Search**. You can also paste a link
   to the release on MusicBrainz or Discogs.
3. Click a result to compare it with your files: its track list beside
   **Your file**'s, with **Lengths agree** where they do. Its score is
   **Including track lengths**, or **From the search result alone**.
4. Click **Use this release**. The album's details, cover and description
   follow it.

Or click **None of these** to say the source doesn't have this album (it
won't be matched there automatically again), or **Use automatic** to forget
your choice and match it again now.

### An artist: Wrong artist?

On the artist's page, click **Wrong artist?** (or **Search MusicBrainz…**
if they weren't found). A dialog lists the MusicBrainz artists who could
be them, with MusicBrainz's score for each.

1. Type a name, or paste a MusicBrainz artist link, and click **Search**.
2. **Open on MusicBrainz** shows an artist's page there, to check.
3. Click **Use this artist**.

**None of these** says they aren't on MusicBrainz; **Use automatic**
forgets your choice and looks them up again. **Look up again** on the
artist's page asks MusicBrainz anew without changing your choice.

## Checking for updates

**Check for updates** in **Settings › About** asks GitHub, where ano-mp is
published, whether there is a newer version. **Check for updates
automatically** (off at first) does so shortly after launch and once a
day, and says when there is one. GitHub sees your IP address; nothing else
is sent. ano-mp doesn't install updates itself: **Download from GitHub**
opens the release's page.
