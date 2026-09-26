// The position in the current track: the only state that changes about
// every 50 ms while playing. It is kept apart from everything else so that
// only the components showing it (the seek bar and its times) re-render.

export const playback = $state({ position: 0, duration: 0 });
