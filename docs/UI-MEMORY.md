# GTK memory work

GPU acceleration remains enabled. The application does not set `GSK_RENDERER`,
`GDK_DISABLE` or a GPU-cache override. GTK chooses its normal renderer, and user
renderer settings remain effective. The temporary Cairo/no-GPU experiments were
not retained, following the user's instruction.

Changes retained:

- Create settings panels on first use, then keep their widgets and drafts when
  switching pages. The overview remains available immediately.
- Use standard Adwaita symbolic icons instead of indexing large custom icon themes.
- Set a glibc trim threshold for this application, before GTK/thread initialization,
  so temporary allocations can be released. Existing allocator settings are respected.
  This uses safe process re-execution, without unsafe global environment mutation.
- Add an optional renderer diagnostic and a repeatable desktop memory probe.

The earlier approximately 230 MiB RSS report came from Xvfb with Cairo selected,
which still initialized graphics drivers. Xvfb's normal GPU renderer uses software
emulation and can consume considerably more RAM; it is useful for UI regression
checks but does not represent real GPU memory use on this laptop.

Native Wayland runs confirmed `GskVulkanRenderer`, with NVIDIA driver libraries and
no application renderer overrides. Before/after native measurements varied with
warm-up and driver caching: approximately 162–183 MiB RSS before, and 138–152 MiB
RSS after. These are development measurements, not a 30–40 MiB total-RSS guarantee.
PSS and private memory are reported separately in the probe output; neither should
be presented as total RSS. GPU acceleration was explicitly prioritized by the user.

Run on the actual desktop, with an isolated D-Bus session:

```sh
cargo build --workspace --release --locked
dbus-run-session -- python3 tests/ui_memory.py
```

This briefly opens a window connected to mock IPC, samples `/proc`, closes it,
and writes `artifacts/ui-memory.json` and its renderer log. It never writes hardware
settings. The accessibility bridge is disabled only in this isolated benchmark to
avoid unrelated session-service failures; production accessibility is unchanged.
Use `--seconds 20` for a longer sample or `--binary PATH` to compare another build.

The normal Xvfb UI test still checks all five pages and applying a fan draft after
status polling and a page round-trip. It no longer forces a Cairo renderer.
