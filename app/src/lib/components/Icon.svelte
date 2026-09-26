<script lang="ts" module>
  // 24×24 paths, filled with the current colour.
  const paths = {
    play: "M8 5.14v13.72a1 1 0 0 0 1.52.85l10.9-6.86a1 1 0 0 0 0-1.7L9.52 4.29A1 1 0 0 0 8 5.14z",
    pause: "M7 5h3.5v14H7zM13.5 5H17v14h-3.5z",
    previous: "M6 5h2.5v14H6zM19 5.9v12.2a.9.9 0 0 1-1.38.76L9.5 13.76a.9.9 0 0 1 0-1.52l8.12-5.1A.9.9 0 0 1 19 5.9z",
    next: "M15.5 5H18v14h-2.5zM5 5.9v12.2a.9.9 0 0 0 1.38.76l8.12-5.1a.9.9 0 0 0 0-1.52L6.38 7.14A.9.9 0 0 0 5 5.9z",
    shuffle:
      "M16.5 4.5 20 8l-3.5 3.5V9h-1.7c-1.2 0-2 .6-2.8 1.8L8.6 16c-1.1 1.7-2.5 2.5-4.4 2.5H3v-2h1.2c1.2 0 2-.5 2.8-1.7l3.4-5.2C11.5 7.9 12.9 7 14.8 7h1.7zM3 7h1.2c1.9 0 3.2.8 4.3 2.3l-1.2 1.8C6.6 9.6 5.6 9 4.2 9H3zm10.3 6.6c.5.8 1 1.4 1.5 1.4h1.7v-2.5L20 16l-3.5 3.5V17h-1.7c-1.8 0-3-.8-4-2.1z",
    repeat:
      "M7 7h10.5V4.5L21 8l-3.5 3.5V9H7a2 2 0 0 0-2 2v1.5H3V11a4 4 0 0 1 4-4zm12 4.5h2V13a4 4 0 0 1-4 4H6.5v2.5L3 16l3.5-3.5V15H17a2 2 0 0 0 2-2z",
    queue: "M3 6h13v2H3zm0 5h13v2H3zm0 5h8v2H3zm14-2 5 3-5 3z",
    menu: "M3 6h18v2H3zm0 5h18v2H3zm0 5h18v2H3z",
    close: "M6.4 5 12 10.6 17.6 5 19 6.4 13.4 12l5.6 5.6-1.4 1.4-5.6-5.6L6.4 19 5 17.6l5.6-5.6L5 6.4z",
    more: "M12 7a2 2 0 1 1 0-4 2 2 0 0 1 0 4zm0 7a2 2 0 1 1 0-4 2 2 0 0 1 0 4zm0 7a2 2 0 1 1 0-4 2 2 0 0 1 0 4z",
    plus: "M11 5h2v6h6v2h-6v6h-2v-6H5v-2h6z",
    refresh:
      "M17.7 6.3A8 8 0 1 0 20 12h-2a6 6 0 1 1-1.76-4.24L13 11h7V4z",
    volume: "M4 9h4l5-4v14l-5-4H4zm12.5-1.5a6 6 0 0 1 0 9l-1.4-1.4a4 4 0 0 0 0-6.2zM19.3 4.7a10 10 0 0 1 0 14.6l-1.4-1.4a8 8 0 0 0 0-11.8z",
    mute: "M4 9h4l5-4v14l-5-4H4zm12.3.3L18 11l1.7-1.7 1.4 1.4-1.7 1.7 1.7 1.7-1.4 1.4-1.7-1.7-1.7 1.7-1.4-1.4 1.7-1.7-1.7-1.7z",
    drag: "M9 5h2v2H9zm4 0h2v2h-2zM9 11h2v2H9zm4 0h2v2h-2zm-4 6h2v2H9zm4 0h2v2h-2z",
    note: "M10 4h9v3h-7v9.5a3.5 3.5 0 1 1-2-3.16z",
    expand: "M4 4h7v2H7.4l4.3 4.3-1.4 1.4L6 7.4V11H4zm16 16h-7v-2h3.6l-4.3-4.3 1.4-1.4 4.3 4.3V13h2z",
    folder: "M3 6a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    person: "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zm0 2c-4.4 0-8 2.2-8 5v1h16v-1c0-2.8-3.6-5-8-5z",
    up: "M12 5.5 5 12.5l1.4 1.4 4.6-4.6V19h2V9.3l4.6 4.6 1.4-1.4z",
    down: "M12 18.5l7-7-1.4-1.4-4.6 4.6V5h-2v9.7l-4.6-4.6L5 11.5z",
    search: "M10 3a7 7 0 0 1 5.6 11.2l5.1 5.1-1.4 1.4-5.1-5.1A7 7 0 1 1 10 3zm0 2a5 5 0 1 0 0 10 5 5 0 0 0 0-10z",
    cloud: "M7 19a5 5 0 0 1-.7-9.95A6.5 6.5 0 0 1 18.8 10 4.5 4.5 0 0 1 18 19z",
    check: "M9.5 16.2 5.3 12l-1.4 1.4 5.6 5.6L20.1 8.4 18.7 7z",
  } as const;

  export type IconName = keyof typeof paths;
</script>

<script lang="ts">
  let { name, size = "1.25em" }: { name: IconName; size?: string } = $props();
</script>

<svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" fill="currentColor">
  <path d={paths[name]} />
</svg>

<style>
  svg {
    flex: none;
    display: block;
  }
</style>
