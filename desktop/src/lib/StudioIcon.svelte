<script lang="ts">
  let { name = 'settings' }: { name?: string } = $props();
  const paths: Record<string, string> = {
    help: 'M9.1 9a3 3 0 0 1 5.8 1c0 2-3 2-3 4m0 3h.01',
    plus: 'M12 5v14M5 12h14',
    close: 'm6 6 12 12M6 18 18 6',
    undo: 'M9 5 4 10l5 5M4 10h10a6 6 0 0 1 0 12',
    redo: 'm15 5 5 5-5 5M20 10H10a6 6 0 0 0 0 12',
    up: 'm6 14 6-6 6 6M6 4h12',
    down: 'm6 10 6 6 6-6M6 20h12',
    copy: 'M9 9h12v12H9zM15 9V3H3v12h6',
    trash: 'M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7',
    text: 'M4 5h16M12 5v15M8 20h8',
    image: 'M3 3h18v18H3zM3 17l6-6 4 4 3-3 5 5M16 7h.01',
    clock: 'M12 6v6l4 2',
    sensor: 'M3 12h4l3-7 4 14 3-7h4',
    divider: 'M3 12h18',
    settings: 'M4 6h16M4 12h16M4 18h16M8 3v6M16 9v6M10 15v6',
    pause: 'M8 5v14M16 5v14',
    play: 'm7 4 14 8-14 8z',
    folder: 'M3 5h7l2 3h9v12H3z',
    edit: 'm4 16 12-12 4 4L8 20H4zM13 7l4 4',
    rotate: 'M4 10a8 8 0 1 1 1 8M4 4v6h6',
    screen: 'M3 4h18v13H3zM8 21h8M12 17v4',
    download: 'M12 3v12m-5-5 5 5 5-5M4 17v4h16v-4',
  };
</script>

<svg
  width="18"
  height="18"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.6"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === 'help' || name === 'clock'}<circle cx="12" cy="12" r="9" />{/if}<path
    d={paths[name] ?? paths.settings}
  />
</svg>
