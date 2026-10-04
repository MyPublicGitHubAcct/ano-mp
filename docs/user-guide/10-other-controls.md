# 10. Other ways to control it

You don't need the main window to control ano-mp. This chapter covers the
menu bar, the Dock, the mini player, the controls in the macOS menu bar,
media keys and Now Playing, notifications, opening files from the Finder,
and controlling it from a phone.

## The menus

Every menu shortcut works wherever you are in ano-mp, even while typing in
the search box. [Appendix A](appendix-a-shortcuts.md) lists them all.

- **ano-mp**: **About ano-mp**, **Settings…** (⌘,), and Hide and Quit.
- **File**: **New Playlist** (⌘N), **New Smart Playlist…** (⌥⌘N), **Open
  Files…** (⌘O), **Add Folder to Library…** (⇧⌘O), **Import Playlist…**,
  **Export Playlist…**, **Import Library Data…**, **Export Library
  Data…**, **Get Info** (⌘I), and Close Window.
- **Edit**: the usual Undo, Cut, Copy, Paste and Select All, and **Find**
  (⌘F), which goes to the search box.
- **Controls**: **Play** or **Pause** (Space), **Next** (⌘→), **Previous**
  (⌘←), **Increase Volume** (⌘↑), **Decrease Volume** (⌘↓), **Shuffle**,
  **Repeat** (**Off**, **All**, **One**), **Stop After This Track** (⌥⌘.),
  **Sleep Timer** (**Off**, **In 15 Minutes**, **In 30 Minutes**, **In 1
  Hour**, **In 90 Minutes**, **At the End of This Track**, **At the End of
  This Album**), **Record** (⌥⌘R, while **Recording** is on), and **Go to
  Current Track** (⌘L), which opens the album playing.
- **View**: **Home** (⌘1), **Library** (⌘2), **Favourites** (⌘3), **Now
  Playing** (⌘4), **Queue** (⌘5), **Visualizer** (⌘6), **Show or Hide the
  Queue Panel** (⌥⌘Q), **Mini Player** (⌥⌘M) and Enter Full Screen.
- **Window**: Minimize and Zoom.
- **Help**: **ano-mp Help**, which opens this guide in a window of its
  own (it works offline), and **Keyboard Shortcuts** (⌘/).

## The Dock

Right-click (or Control-click) ano-mp's Dock icon for the track playing,
**Play** or **Pause**, **Next** and **Previous**. Drop audio files on the
Dock icon to play them.

## The mini player

The mini player is a small window with just the player bar: the track,
the controls, the seek bar and the volume.

<!-- Screenshot: the mini player above another app's window. -->

- Open it with **View › Mini Player** (⌥⌘M), or **Open the mini player** in
  **Settings › General**. The same shortcut closes it.
- Click the track in it to bring back the main window.
- **Keep the mini player above other windows** (in **Settings ›
  General**) keeps it in front of other apps' windows.

## Controls in the menu bar

With **Controls in the menu bar** on (in **Settings › General**), an
ano-mp icon sits in the macOS menu bar, at the top right of the screen.
Click it for the track playing, **Play** or **Pause**, **Next**,
**Previous**, **Mini Player** and **Show ano-mp**. Point at it to see what
is playing.

## Media keys and Now Playing

ano-mp works with macOS's own music controls:

- the play/pause, next and previous keys on your keyboard;
- the buttons on headphones and AirPods;
- **Now Playing** in Control Centre and the menu bar, with the cover and a
  progress bar you can drag;
- the Lock Screen's Now Playing controls.

When ano-mp opens with a queue from last time, Now Playing shows it paused,
so the play key carries on where you left off.

## Notifications

With **Show each new track** on (in **Settings › General**), a
notification with the cover shows the title and artist when the track
changes, but only while ano-mp isn't the app in front. macOS asks whether
to allow notifications the first time; change that later in **System
Settings › Notifications › ano-mp**.

## Opening files from the Finder

You can play files that aren't in your library:

- choose **File › Open Files…** (⌘O);
- or, in the Finder, right-click audio files and choose **Open With ›
  ano-mp**, or drop them on ano-mp's Dock icon;
- or drop them on ano-mp's window.

They play at once, and are marked **not in the library** in the queue.
They aren't added to the library; to add them, add their folder.

Dropping a **folder** on the window offers to add it to the library
instead. Opening an **M3U playlist** (.m3u or .m3u8) imports it as a
playlist.

## Control it from a phone

The **remote control** lets a phone, tablet or another computer on the
same Wi-Fi network control ano-mp from its web browser, with nothing to
install. It is off at first.

### Set it up

1. In **Settings › Features**, under **Remote control**, turn on
   **Control playback from a phone**. macOS may ask whether to let ano-mp
   use the local network: allow it.
2. Settings says **On your phone, open** an address such as
   "http://192.168.1.20:8765". Open that address in the phone's browser.
   (The phone must be on the same network as the Mac.)
3. In Settings, click **Pair a phone**. A six-digit **Code:** appears,
   valid for five minutes and for one phone.
4. Type the code into the page on the phone and tap **Pair**.

The phone stays paired; bookmark the page or add it to its home screen.
Settings lists each paired phone, with when it was paired and last used,
and **Forget** to unpair it. The **Port** (8765 at first) can be changed if
another app uses it.

<!-- Screenshot: the remote page on a phone. -->

### What a phone can do

- see what is playing, with its cover, and the queue;
- play, pause, skip forward and back, and move through the track;
- change the volume and turn shuffle on or off;
- jump to a track in the queue;
- search the library and play an album or a track.

### What it exposes

Only devices on your local network get an answer; anything from the
internet is ignored. A device must be paired with a code shown on your Mac
before it can do anything, and failed codes are limited. The remote shows
titles, artists and covers, and can change the queue and the volume; it
never gives out your files, their locations or your settings. Turn it off
when you don't use it.
