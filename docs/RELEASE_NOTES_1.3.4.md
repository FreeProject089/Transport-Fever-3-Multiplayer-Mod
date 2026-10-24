# TPF3-MP v1.3.4

Version 1.3.4 restores multiplayer for **Transport Fever 3 Windows Steam build
40425** (Steam build ID 26129055). The v1.3.3 hook recognizes build 40423 and
refuses the updated executable. Players still on 40423 need the matching
earlier TPF3-MP package until they update the game.

## Changes since v1.3.3

- The hook selects a new 40425 native profile, pinned to the executable's
  SHA-256, size and PE timestamp. Tearded's comparison found that the game's
  code and scripts used by the hook are unchanged from 40423 apart from
  version data: all 165 native targets remain at the same addresses. The
  batch interval and line-management window copy remain available for this
  exact build ([#187](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/187)).
- The bundled Big Maps plugin code is pinned to 40425. **Big Maps cannot yet
  be installed for 40425**: the relay's reviewed catalogue still offers the
  separate 40423-only package. A 40425 package must be published and reviewed
  before the catalogue can offer it. TPF3-MP does not install the 40423 package
  into a 40425 game ([#187](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/187)).
- The Linux installer test now uses a Bash process as its stand-in for a
  running game, keeping the existing refusal check valid on newer Ubuntu
  images. The installer behavior is unchanged
  ([#187](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/187)).

The crossing-window rebuild and the two-command snapped-construction
fallback remain disabled. Replacing one stop with another through the stop
tool is still refused in rooms.

## Verification and operation

Tearded reports a two-game 40425 room run, including road and track builds,
bulldozing, a bus stop and a loan, with no divergence in the recorded checks.
The project's own release preparation does not repeat the in-game test, as
requested by the owner. The complete private Steam build 26129055 archive is
verified against the exact release commit by `tpfre verify-build`; its 165
static targets must match without review required before packaging. Static
verification does not prove every runtime path. CI and acceptance must pass
on the exact promoted commit.

The protocol remains **23**, Lua bridge **29**, and room-log format **10**.
This client-only build update needs no relay restart or room-log migration;
the existing v1.3.3 relay and its tag-pinned native catalogue stay in place.
The signed updater metadata is attached after publication by the protected
signing workflow. The Windows standalone launcher has no Windows publisher
certificate. Restart the launcher to update, or download the new
**TPF3-MP.exe**. Player and server archives are provided separately.

Max (tearded) authored [#187](https://github.com/Juliansgith/Transport-Fever-3-Multiplayer-Mod/pull/187),
with Claude Opus 5.5 recorded as co-author. Big Maps' underlying plugin work
is Silver's (silver2127); this release only repins its bundled code to the new
game identity.
