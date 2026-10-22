# TPF3-MP v1.3.2

Version 1.3.2 brings more road, rail, terrain and native-mod actions into
multiplayer rooms. The selected Windows Steam game build is **40420**. Players
and servers must update together: this release uses protocol **23** (v1.3.1
used 19), bridge **29**, and room-log format **10**. Older clients are refused
with an update message; format-8 and format-9 room starts remain readable.

## Changes since v1.3.1

- **Building and editing.** Bridge and tunnel type changes work from their
  windows, including long bridges whose unchanged junction settings no longer
  exhaust the action limit ([#143](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/143),
  [#175](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/175),
  [#181](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/181)).
  Track and street edits better preserve the intended edge, junction settings
  and nearby signals, including stacked edges, junction branches and signals
  the track tool refits near a removed node
  ([#155](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/155),
  [#161](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/161),
  [#162](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/162),
  [#168](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/168)).
  Track joins at tram-street ends and previously accepted stop/signal
  rebuilds also carry through the shared schema
  ([#158](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/158),
  [#164](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/164),
  [#165](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/165)).
  A new track crossing can take the junction window's double-slip switch
  even before that node has other settings
  ([#173](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/173)).
  The level-crossing type picked in the road or track toolbar now reaches
  every game in the room
  ([#169](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/169),
  [#178](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/178)).
  Line waypoints are enabled after a two-player check
  ([#159](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/159)),
  and Signal Distance's dotted signal setting is carried
  ([#144](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/144)).
- **Terrain, trees and responsiveness.** Room actions cover terrain paint,
  asset groups and reviewed road, rail and line edits with a single
  fail-closed action layout
  ([#160](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/160)).
  A quick second tree bulldoze in the same group is no longer refused, and
  planted or retained trees no longer level squares of ground around them
  ([#176](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/176),
  [#183](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/183)).
  Other players' long build previews redraw at a cost-aware pace; network
  lookup work during replay is reduced without dropping proposal checks
  ([#163](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/163),
  [#167](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/167),
  [#174](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/174),
  [#182](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/182)).
  Experimental street and track tool continuation handles the click's own
  reset and a room build landing under the cursor; it still requires
  `TPF3MP_TOOL_CONTINUE=1`
  ([#157](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/157),
  [#172](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/172),
  [#179](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/179)).
- **Native mods and room checks.** The launcher can manage native mods and
  match their exact packages and settings to a room. Server registries,
  consent tied to plugin content and source, and a Big Maps plugin are
  included; native plugin loading currently supports Windows x86-64 only.
  A production registry and Big Maps package are not configured or published
  by this release
  ([#147](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/147),
  [#148](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/148),
  [#151](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/151),
  [#152](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/152);
  owner-approved policy in
  [#146](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/146) and
  [#153](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/153)).
  Smaller native rolling-check batches, stronger checks of room state, and
  bounded traces help locate a divergence without guessing an entity when a
  native read fails
  ([#111](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/111),
  [#149](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/149)).
- **Setup and operation.** Windows setup handles unavailable Steam libraries
  and files held open during repair; admin tools can find rooms by invite,
  player or name. Large subsidy and balance numbers no longer stop a room's
  world check
  ([#140](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/140),
  [#141](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/141),
  [#154](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/154),
  [#142](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/142),
  [#145](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/145)).

## Limits and rollout

The existing crossing **window** can still emit a network rebuild that its
type-only action cannot reproduce; that path remains refused. Snapping a
construction to track ends retains the safe direct path, but the two-command
fallback that could remove track before a failed reconnect remains disabled
([#138](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/138),
[#171](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/171)).
The crossing-window limit was kept explicit in
[#166](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/166).
The new street-stop bulldoze and kept-stop implementation is included, but
its `bulldozeStops` and `keptStops` gates remain off; remove stops first when
bulldozing their street
([#177](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/177)).

Deploy the matching server before directing updated players to it; old
protocol-19 clients cannot join protocol-23 rooms. Keep existing game data
and follow [Upgrades](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/blob/main/docs/OPERATIONS.md#upgrades) for the format-8/9-to-10 server
transition. This draft is **not signed for automatic updates**. Signing and
the server image happen only after the owner publishes and approves the
protected signing workflow. The Windows standalone launcher has no Windows
publisher certificate.

The contributor PRs record focused 40420 two-player tests, including an
ordinary launcher-start run for the toolbar crossing type. The owner waived
new in-game testing for this release preparation; automated CI, acceptance,
static native-build verification and the fake-game upgrade check remain the
release gates. No production relay deployment is part of this draft.

## Contributor provenance

The implementation commits since v1.3.1 credit Max (tearded), Silver
(silver2127), and Julian Cooper (Juliansgith); some Max commits also record
Claude Opus 5.5 as a co-author. The linked PRs distinguish each proposal's
author and review. The repository's [contribution provenance](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/blob/main/docs/CONTRIBUTION_PROVENANCE.md)
describes how those roles are recorded. Code-owner review is shared by
Juliansgith and silver2127
([#170](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/170));
the contributor CI workflow was updated in
[#180](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/180).

To update after publication, close the game and restart the launcher, or
download **TPF3-MP.exe** for Windows. Separate player and server packages are
provided for Windows, Linux and macOS.
