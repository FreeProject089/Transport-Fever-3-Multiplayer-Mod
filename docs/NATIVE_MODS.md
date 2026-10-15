# Native mods: trusted indexes, installed by the launcher

*Owner-approved policy* ([DECISIONS.md](DECISIONS.md), D29 and its
server-index and plugin-consent amendments, approved 2026-10-15). The
launcher group, protocol-20 room terms and room-version install on joining
are built on `dev`. The open #148 work adds the server index and a full
canonical package identity with installed-byte/file-set verification.
Plugins and per-content executable consent are policy-approved but not
implemented or merged: #151 remains open, and plugin entries and hook
declarations must be added to that same identity before plugin code may
load. No server registry or first native package is configured yet, so
players cannot use a native mod yet. The implemented and open parts are
listed at the end.

A **native mod** is a mod that is more than Lua: it needs code in the game.
Big Maps ([BIGMAPS.md](BIGMAPS.md)) is the first: hook patches behind
switches, targets in the build's hook profile, a New Game page the hook
serves, settings, and a Lua mod folder. Mod Hub carries Lua mods for the
game's own loader, and a room's Mod Hub mods are handled by D28
([LOBBY.md](LOBBY.md), [MODS.md](MODS.md)); native mods come through a
channel of their own, managed the way CKAN manages Kerbal Space Program
mods. A launcher may use the project's signed index or the index of the
server hosting the room, under the approved server-index amendment; it does
not accept an arbitrary index URL.

## The index

`native-mods.json` lists every package the launcher may install.
`native-mods.json.sig` is an Ed25519 signature of its exact bytes. Both are
assets of the `native-mods` release of the repository
(`releases/download/native-mods/`), fetched over HTTPS only.

```json
{
  "format": 1,
  "serial": 12,
  "packages": [
    {
      "id": "bigmap",
      "version": "0.3.0",
      "name": "Big Maps",
      "description": "Maps up to 128 km a side.",
      "homepage": "https://github.com/<its authors>/<its repository>",
      "simulation": true,
      "builds": ["<SHA-256 of the game's executable>"],
      "features": ["bigmap.octree", "bigmap.street_raster", "bigmap.placement",
                   "bigmap.page", "bigmap.density", "bigmap.memory_gate"],
      "settings": {"octree_depth": 10, "street_raster": true},
      "depends": [{"id": "other", "version": "^1.2"}],
      "conflicts": ["another_big_map_mod"],
      "files": [
        {"path": "mod/tpf3mp_bigmap_1/mod.lua",
         "url": "https://github.com/…/releases/download/native-bigmap-v0.3.0/mod.lua",
         "size": 1234, "sha256": "<lowercase hex>"}
      ],
      "plugins": []
    }
  ]
}
```

| field | meaning |
|---|---|
| `format` | 1. Another format is refused whole: a launcher that does not know a format does not guess. |
| `serial` | Grows with every index published. The launcher remembers the highest it accepted and refuses an older one, so a withdrawn package cannot come back by replaying an old signed index. The registry also binds an accepted serial to the SHA-256 of the exact index bytes; a crash after writing a complete cache pair recovers that serial floor, and different contents cannot reuse it. |
| `id` | 1 to 40 lowercase letters, digits and `_`. |
| `version` | Semantic version, at most 20 characters (it travels in the room's terms). |
| `homepage` | Optional. The mod's own project page, such as its authors' repository: the launcher links to it, as CKAN links to a mod's. An `https` address of at most 200 characters. The files still come only as the signed index pins them, wherever they are hosted (the mod's own releases included). |
| `simulation` | Whether it changes how the world runs. A feature the hook knows to change the simulation makes the package count as changing it too, whatever this says. |
| `builds` | The game builds it runs on, by the executable's SHA-256, as hook profiles pin them ([HOOKS.md](HOOKS.md)). On any other build it is not installed, and not enabled if it was. |
| `features` | The hook features it enables (below). |
| `settings` | Its settings with their defaults: booleans, integers or text of at most 128 bytes (a room tells them to its members). A player's choice must name one of these and keep its type. |
| `depends` | Packages it needs, with a semver requirement. |
| `conflicts` | Packages that must not be installed beside it. |
| `files` | Data, Lua and settings files: where each goes in the package's folder (plain names joined by `/`), where it comes from, its size and SHA-256. |
| `plugins` | Native libraries pinned by platform, `path`, `url`, `size`, `sha256` and `abi`. Current `dev` still refuses a package that has any. The approved plugin policy requires explicit consent before a plugin payload is fetched or installed. |

The plugin-consent prompt must get the author/publisher and trusted source
from validated index or registry metadata. The current `dev` index schema
does not yet carry all consent metadata; the schema must supply it before
plugin loading is implemented. A `homepage` is a link for the player, not a
source of prompt data: the launcher must not fetch it or a file URL to build
the consent screen.

Unknown fields are refused, and one malformed entry refuses the whole index:
the trusted project or server source must fix a mistake, never have the
launcher guess around it.

### Trusted sources

The project may still publish an optional native-mods index signed with its
own key. Under the owner-approved server-index amendment, a launcher also
trusts the index supplied by the server it is playing on, reached through
that server's authenticated connection; the server builds it from its
configured registry. The launcher keeps each server's index and serial
floor separate. A server selected in Settings is used only when the player
asks, and proposed registry entries are never installed by a room without
the player's choice.

The framework on `dev` still verifies the project's signed index. The open
#148 work implements the server index and per-server cache. No private key
or signing workflow is required for a server's own index; if the project
publishes its optional index, its key remains separate from release keys.

## Installing

The launcher keeps native mods in its own data folder, never the game's:

```text
<launcher data>/native-mods/
  native-mods.json, native-mods.json.sig   the last index accepted
  registry.json                            every installed package and its files
  enabled.json                             what the next game runs
  <id>/<version>/…                         a package's files
  <id>/.partial-<version>/                 a download in progress
```

`store::Store` does every step, refusing rather than guessing:

1. **Resolve** (`resolve`): the wanted package's newest version pinned to the
   running build, then its dependencies (an installed version is kept when it
   satisfies the requirement). Current `dev` refuses packages with plugins;
   the approved policy allows them only after the consent and identity checks
   below are implemented. Other refusals include a package the index does not
   list, none for this build, a feature this hook does not have, an unsatisfied
   dependency, a dependency cycle or a conflict. There is no backtracking: an
   install that does not hold together is refused with the reason.
2. **Download** each file into `.partial-<version>/`, checking its size and
   SHA-256 on the way (`fetch`, the updater's code). More bytes than signed
   stop the download; a short or changed file fails it.
3. **Move into place** only when every file checks out: the partial folder is
   renamed to `<id>/<version>/`. A failure deletes the partial folder; the
   version in use is untouched.
4. **Register**: the version becomes the one in use, the one before it is
   kept for a rollback, and any older one is deleted. The registry is written
   through a temporary file, so it is whole or not written.

- **Upgrade** is an install of a newer version: the old one stays in use
  until the new one has verified, then stays on disk for a rollback.
- **Rollback** goes back to the version before, after checking its files are
  still whole.
- **Uninstall** deletes exactly the files the registry names, then the
  folders that leaves empty; a file someone else put there stays. A package
  another installed package needs is not uninstalled.
- An install cut short after the rename leaves a folder the registry does
  not name: the next install takes it if every file is the package's, and
  refuses (`Occupied`) otherwise.
- A registry that cannot be read is an error, never replaced by an empty
  one.
- Registry package paths are validated before use. Traversal paths and paths
  that cross a symlink, including a symlink above the store root, are refused
  before reads, writes, or deletes.
- **Installing is not enabling.** The player switches a package on, and sets
  its settings, separately.

## What the game runs

When the launcher starts the game (D11) and the player has enabled a native
mod, it hashes the game's executable, writes the enabled packages for that
build to `enabled.json` with their settings (the defaults with the player's
choices over them) and their folders, and names that file in the game's
environment as `TPF3MP_NATIVE_MODS` (`tpf3mp-agent`'s `native_mods`). A
package that changes only what one player sees and is not pinned to that
build is left out with a log message. An enabled simulation-changing
package that is not pinned to that build stops the start instead of
producing an empty list. Nothing enabled, nothing installed: no variable,
and the executable is not hashed. A registry that cannot be read stops the
start.

The hook makes its plan once, at bootstrap, after matching the build's
profile and before installing anything (`tpf3mp-hook`'s `native_mods`,
`enabled::plan`). A package's features run only when:

- the file names the build the hook runs in;
- this hook has every feature the package names
  (`tpf3mp_nativemods::features::BUILT_IN`), and the matching profiles have
  every target those features patch;
- for a package that changes the simulation, the room compares native mods
  (below).

Otherwise a package that changes only what its player sees is left off, with
the reason in `hook.log`; one that changes the simulation **keeps the game
out of rooms**: the hook installs nothing and logs
`multiplayer disabled (fail-closed): native mods that change the simulation
cannot run: …`, as for a build it has no profile for. An `enabled.json` that
cannot be read refuses the same way. No other switch turns a native feature
on: Big Maps' own branches read `TPF3MP_BIGMAP_*` variables by hand; as a
package, it would ask the plan instead (`native_mods::feature(id)`, which
gives the feature's settings and its package's folder).

### Built-in features and plugins

Built-in features remain part of D29. A package may name **features
compiled into the hook**, by id; the hook switches on those the launcher
enabled. Each feature has an id (the package's id, a dot, the feature's), an
effect (only what one player sees, or the simulation) and the profile
targets it patches. `features::BUILT_IN` is empty on `dev`: no native feature
is built in yet. Big Maps would add its own; `features::example::BIG_MAPS`
shows the shape, and the tests use it:

| feature | effect | profile targets |
|---|---|---|
| `bigmap.octree` | simulation | `bigmap::octree_root` |
| `bigmap.street_raster` | simulation | `bigmap::street_raster` |
| `bigmap.placement` | simulation | `bigmap::placement_attempts` |
| `bigmap.page` | what one player sees | `bigmap::tile_count`, `bigmap::size_list` |
| `bigmap.density` | simulation | `bigmap::density_levels` |
| `bigmap.memory_gate` | what one player sees | none |

The launcher reads the same registry to refuse a package this TPF3-MP cannot
run before downloading it.

The owner-approved plugin policy permits executable libraries from the same
trusted sources as other native packages. It does not replace built-in
features or make them executable consent items. Plugin loading and consent
remain unimplemented on `dev`; current `dev` refuses packages with plugins.
Before an implementation loads one, the plugin bytes and declarations must
be covered by the package identity used in the room terms and package store.

## In a room

A native mod that changes the simulation must run in every game of a room,
in the same version, with the same settings; one that changes only what a
player sees is that player's own, as personal mods are ([MODS.md](MODS.md),
D25).

The room's **native terms** are, for each simulation-changing package the
game runs, its id, version, effective settings and full canonical package
identity, ordered by id. Protocol 20 on `dev` carries the settings digest;
the open #148 work adds the full `identity` digest and verifies installed
file bytes against its file-set manifest. The identity covers package id and
version, simulation flag, features, settings, dependencies and sorted
regular-file paths, sizes and SHA-256s. Before plugin loading is
implemented, extend that same identity with each plugin's platform, ABI,
path, size and SHA-256 and its declared hooks, so terms, storage checks and
consent all name the same content. The full digest is not a separate
approval key.

These terms travel in the content manifest's own list,
`ContentManifest::native` ([PROTOCOL.md](PROTOCOL.md), "Native mods"):
each as a mod named `native:<id>` (no mod folder can be named so; `:` is
not allowed in a Windows file name). The pending #148 representation keeps
the full digest in the native term while ordinary mod versions stay short.
The room compares native terms as it compares mods: a member whose package
bytes or settings differ cannot start or join, and is told which native mod
is missing, which differs and which the room does not run.

What a game declares is what the launcher started it with: the terms of the
`enabled.json` it wrote for that game (`tpf3mp-agent`'s `native_mods`), and
with no game running from the launcher, what the next game will run. The
room's owner also tells the room every setting of each (`RoomConfig::native`,
`RoomMods::native`), so a member's launcher can match them.

**Joining a room** gets the room's native mods by itself:

1. The room tells its native mods on joining its lobby, or before refusing
   a join to its running game.
2. The launcher installs each in the room's exact version from the trusted
   index of that room's server (or the project's optional signed index), for
   this game's build, **never from another player**
   (`match_room`, `Store::install_exact`); sets the room's settings over the
   defaults, the player's own choices left out; switches each on; and
   switches off any other simulation-changing native mod. One that changes
   only what the player sees stays as it was. A version the index does not
   have for this build, settings that do not come out the same, or package
   identity that differs, refuse: that player cannot join, and the window
   says why. Before any plugin payload is fetched or installed, show the
   author/publisher, trusted source and full canonical package digest from
   the already validated index/registry metadata. Do not fetch a homepage,
   file URL or other arbitrary URL to assemble that prompt. Reconnecting
   with the same full identity does not prompt again. Any identity change
   requires fresh consent; a version or metadata change may prompt again even
   if plugin bytes are unchanged. Changed plugin bytes or hook declarations
   necessarily change the identity. If the player declines, do not fetch,
   install or enable that package's plugin payload, and keep the join blocked.
3. **The game starts again with them**, since the hook reads the enabled
   list once, as the game starts: when the game running from the launcher
   runs other native mods, the launcher leaves the room's lobby, closes the
   game, starts it again and joins the same room (invite and password as
   the player gave them). A refused join to a running game does the same
   before it tries again. With no game running, the player declares the
   room's terms at once. This happens only on joining a room, and only when
   native mods were installed or switched; nothing else closes the game.
   The room's owner is never asked to: their game's terms are the room's.
4. The lobby lists the room's native mods with whether this game runs each
   alike, and offers **Get the room's native mods** when an attempt failed.

A room restored after a server restart keeps its native mods and their full
settings in the start record (format 9), so a new member's launcher installs
and enables the exact room terms. Native-free format-8 logs remain
recoverable. A format-8 log that names native mods is refused and preserved
for diagnosis because its settings were never recorded; recovery does not
invent defaults.

Mods from Mod Hub are not native mods: players get those through their own
game and Mod Hub account (D28).

## In the launcher

Settings has a **Native mods** group. It lists each package of the trusted
index for the current server (or the optional project-signed index) and
each installed one, with its description, a link to
its own project page (`homepage`), whether it changes the simulation, and
**Install** or **Update**, **Switch on**/**Switch off**, **Roll back** and
**Uninstall**, then its settings (switches and numbers; text is shown).
Plugin packages are marked **Runs code**. Before downloading their
executable content, the launcher shows the author/publisher, trusted source
and full canonical package digest, and asks for consent. It remembers
consent for the same full identity on reconnect; a changed identity requires
a new choice, even if a version or metadata change leaves plugin bytes
unchanged. **Check the index** refreshes the index from its trusted source.
A change takes effect in the next game the launcher starts; with no game
running, the player declares the new terms at once.

## Framework built, player feature still open

Framework implemented (`crates/tpf3mp-nativemods`, with tests):

- the index format, signature verification primitives and validation, a
  serial floor bound to exact index bytes, and recovery from a complete
  cache written before a crash;
- resolution: builds, dependencies, conflicts, cycles, features, plugins;
- the store: checked downloads, the registry, upgrade, rollback, uninstall,
  enabling and settings, `enabled.json`;
- the hook's plan and its fail-closed refusal (`tpf3mp-hook`), and the
  launcher passing `TPF3MP_NATIVE_MODS` to the game it starts
  (`tpf3mp-agent`);
- helpers for room-term comparison and finding a missing package in a
  signed index;
- the launcher's updater uses the same signature and download code.

Built on it (protocol 20): the room's terms on the wire with their
settings, the launcher's Native mods group, `Store::install_exact` and
`resolve::resolve_exact` for a room's exact version, and getting the room's
native mods on joining, the game started again with them. The open #148
work adds a full canonical package identity to room terms and verifies
installed bytes against the file manifest. It does not yet cover plugin
entries or implement executable consent.

This is not yet a feature players can use: the production trusted-key list
is empty and `features::BUILT_IN` has no native features. The tests use
temporary test keys.

Still open:

- the server registry and first native package; the project's optional
  signed index remains a separate source;
- copying a package's Lua mod folder where the game loads mods, the way
  TPF3-MP's own mod is installed ([MODS.md](MODS.md), "TPF3-MP's own mod");
- Big Maps as the first package, with its features built into the hook;
- plugin loading and explicit per-identity consent. The open #151 code is
  not merged; its plugin entries and declared hooks must extend #148's
  canonical identity before the prompt, room terms, cache and byte checks
  can refer to the exact executable content.
