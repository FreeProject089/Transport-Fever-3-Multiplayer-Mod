# TPF3-MP v1.3.3

Version 1.3.3 restores multiplayer on **Transport Fever 3 Windows Steam build
40423** (Steam build ID 25962684). The 1.3.2 hook targets build 40420 and
refuses the updated executable. This package selects 40423; players still on
40420 need the matching earlier package until they update the game. The room
protocol remains **23**, Lua bridge **29**, and room-log format **10**, so this
patch does not require a relay upgrade or a room-log migration.

## Changes since v1.3.2

- **Game update compatibility.** The native hook bundle was re-derived for
  40423, including moved game UI fields and signatures. The batch interval and
  line-management window remain available on this build. The Big Maps plugin
  source was ported too, but Big Maps users need its separately published
  **40423** package; this release does not supply one
  ([#184](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/184)).
- **More room actions enabled.** Streets can be bulldozed with their stops;
  street builds can retain stops on a replacement piece. Historic Preservation
  can be set or cleared, and company Industry Greenification and marketing
  campaigns can be used in rooms. A campaign now follows the game's infinite
  money setting, where the balance is absent, while unreadable or non-finite
  balances still fail closed
  ([#185](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/185)).
- **Street and track tool chains.** The tools now continue from the segment
  the room built by default, and a right-click ends a chain. In the recorded
  run, a click immediately after a street segment joined a junction made no
  segment; choose a new start point if that happens. Set
  `TPF3MP_TOOL_CONTINUE=0` (also `off`, `false`, or `no`) to turn
  automatic continuation off
  ([#185](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/185)).

The crossing **window** rebuild and the two-command snapped-construction
fallback remain disabled because their unresolved failure paths are not made
safe by a game run. Replacing one stop with another through the stop tool is
also still refused in rooms.

## Release evidence and operation

Tearded's build-40423 PR records a private static archive check matching all
165 native targets and a two-game room run. The gate PR records ordinary
launcher-start, two-game runs of the newly enabled actions. Both PRs passed
their full contributor CI; the integrated commit must also pass the
repository's normal CI and acceptance workflows before publication. At the
owner's request, this release
does not repeat in-game tests. The repository's private runner has no 40423
archive configured, so packaging uses the documented owner-authorized
`fast_track` path rather than claiming that private verification was repeated.
The native archive check remains static evidence, not proof of every runtime
path on every platform.

The relay can remain on v1.3.2: no server, protocol, or persistence code
changed. The signed updater metadata is attached after publication through
the protected signing workflow. The Windows standalone launcher does not
have a Windows publisher certificate.

Max (tearded) authored the changes in [#184](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/184)
and [#185](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/185),
with Claude Opus 5.5 recorded as co-author on the implementation commits.
Restart the launcher to update after publication, or download the new
**TPF3-MP.exe**. Player and server archives are provided separately.
