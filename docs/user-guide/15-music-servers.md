# 15. Music servers

If you keep music on your own Jellyfin or Plex server, ano-mp can play it
next to the music on your Mac: its albums, artists and playlists show up
in the library, search finds them, and they play through the same queue,
with gapless playback, volume levelling, effects and the media keys, as
your own files do. This chapter covers signing in, how the server's music
is kept up to date, playing it away from home, and keeping music for when
the server can't be reached.

Jellyfin is free software you run yourself, on a computer or a network
drive at home. ano-mp needs Jellyfin 10.9 or later. Plex Media Server is
Plex's software, which you also run yourself; you sign in to it with
your Plex account.

## Turn it on

Music servers go online, so they start off. Turn on **Jellyfin servers**
in **Settings › Features**. A **Servers** section appears in Settings, and
a **Servers** heading in the sidebar.

Plex servers aren't offered in this version: **Settings › Features** has
no **Plex servers** switch, unless it is still on from before, so you can
turn it off. What this chapter says about Plex applies once it returns.

## Sign in to a Jellyfin server

1. Open **Settings › Servers** and click **Add a server**, or click **+**
   next to **Servers** in the sidebar.
2. Type the server's address in **Server address**, such as
   http://192.168.1.10:8096, and click **Continue**. Or click **Find
   Servers on This Network** and choose the server from the list. The
   first time, macOS asks whether ano-mp may find devices on your local
   network: allow it, or type the address instead.
3. ano-mp shows the server's name and Jellyfin version. Type your **User
   name** and **Password** and click **Sign In**. Or, if the server allows
   it, click **Use Quick Connect**: ano-mp shows a code to enter in
   another Jellyfin app where you're already signed in, and signs in by
   itself once you have.

Your password goes to the server only, and ano-mp doesn't keep it: it
keeps the sign-in the server gives back, in the macOS keychain.

An address on your own network can start with http://. A server outside
your network needs an https:// address with a certificate from a
certificate authority, so your sign-in and your music don't cross the
internet unprotected. Self-signed certificates aren't supported.

## Sign in to Plex

1. Open **Settings › Servers** and click **Sign In with Plex**.
2. Your browser opens Plex's sign-in page. Sign in to your Plex account
   there if you need to, and approve ano-mp. If the page asks for a code,
   enter the one **Settings › Servers** shows. ano-mp never sees your Plex
   password.
3. ano-mp lists the servers your account reaches, with their music
   libraries. Tick the libraries you want and click **Add Libraries**.
   Each library you add is a server of its own in the sidebar, named by
   the server and the library, such as "Den · Music". Click **Done** when
   you've added what you want.

The first time, macOS may ask whether ano-mp may find devices on your
local network: allow it, so ano-mp can reach a server at home directly.

ano-mp keeps your Plex sign-in in the macOS keychain. It asks plex.tv
only to sign in and to learn where your servers are; your music comes
from your server itself.

A Plex server can be reached in several ways: on your own network, over
the internet, or through Plex's relay when nothing else works. ano-mp
tries your network first and the internet next, and remembers the way
that worked. Plex's relay is slow and limits the bit rate, so ano-mp uses
it only when **Use Plex's relay** is on in **Settings › Servers**; while
it plays through the relay, **Settings › Servers** says so.

Plex may need a Plex Pass to stream music from a server away from its
own network. ano-mp doesn't get around that: if a server refuses, ano-mp
says so, and the music still plays on the server's own network.

## The server's music in your library

Once you've signed in, ano-mp reads the server's music library: titles,
artists, albums, track numbers, years, genres, lengths and covers. This
is a **sync**, the server's version of a rescan. A first sync of a large
library takes a minute or two; the sidebar shows its progress.

The server's tracks are mixed into your library's views, search, Home
and smart playlists. To see the server's tracks alone, click the server
under **Servers** in the sidebar. The server's playlists come in too.
They're marked **From …** with the server's name and can't be changed in
ano-mp: change them in Jellyfin or Plex, and the next sync brings the
change.

ano-mp syncs each server when it starts. To sync now, click the refresh
button next to the server in the sidebar, or **Sync Now** in **Settings ›
Servers**. A sync reads only what changed since the last; **Sync
Everything** reads the whole library again.

If a sync finds most of the server's tracks gone (for example because
the server's music drive isn't mounted), ano-mp keeps them, as it does
for a folder on your Mac, and says so in **Settings › Servers**. If they
really are gone, click **Remove Tracks Not Found**.

## Playing

A server's track is downloaded when you play it, and the next track in
the queue downloads while one plays, so playback is gapless. Tracks stay
downloaded until the space is needed for others. **Space for downloaded
music** in **Settings › Servers** sets how much space they may take, 1 GB
at first; **Remove Downloaded Music** frees it.

The original file is downloaded, unless it's in a format ano-mp can't
play, when the server turns it into MP3. Away from home, files can be
large to download: **Away from home** in **Settings › Servers** can have
a server outside your network send MP3s at a lower bit rate instead.

Get Info shows a server's track's file details once it has been played.
The track has no file on your Mac, so there's no **Show in Finder**.

## Telling the server what you play

With **Tell the server what plays** on (in **Settings › Servers**), ano-mp
tells the server when a track starts and stops, so its play counts and
your other Jellyfin or Plex apps stay up to date. Your listening history, ratings and
favourites in ano-mp stay on your Mac; they aren't sent to the server, and
the server's aren't brought in.

## When the server can't be reached

If the server is off, or you're away from its network, its tracks stay in
the library, marked **can’t be reached** in the sidebar. Tracks already
downloaded still play; others are passed over, and radio and suggestions
leave them out until the server answers again. If the server no longer
accepts your sign-in (a changed password, or the session removed in
Jellyfin or Plex), the sidebar says **signed out**: sign in again in
**Settings › Servers**.

## Keep music available offline

To be sure an album or a playlist plays without the server (on a trip,
say), choose **Keep Available Offline** in its menu. ano-mp downloads its
tracks now and never removes them to make space. **Stop Keeping Offline**
lets them go when space is needed. **Settings › Servers** shows how many
tracks kept offline are downloaded.

## Sign out

Click **Sign Out** next to the server in **Settings › Servers**. ano-mp
ends its session on the server, and the server's music leaves your
library, with its playlists, downloaded tracks, and your plays, ratings
and favourites of its tracks. Signing in again brings the music back.

For Plex, **Sign Out** removes one music library. When you remove the
last library of a server, ano-mp forgets that server's sign-in; when you
remove the last library of your account, it forgets your Plex sign-in
too. To stop ano-mp's access to your account entirely, also remove it
from the authorized devices in your Plex account's settings on plex.tv.
