# Native mods: a signed index, installed by the launcher

*Owner-approved policy* ([DECISIONS.md](DECISIONS.md), D29, approved
2026-10-08). The launcher's Native mods group, the room's native terms on
the wire (protocol 20) and getting the room's native mods on joining are
built; there is no production trust key or index, nor a built-in native
feature yet, so players cannot use a native mod yet. The implemented and
open parts are listed at the end.

A **native mod** is a mod that is more than Lua: it needs code in the game.
Big Maps ([BIGMAPS.md](BIGMAPS.md)) is the first: hook patches behind
switches, targets in the build's hook profile, a New Game page the hook
serves, settings, and a Lua mod folder. Mod Hub carries Lua mods for the
game's own loader, and a room's Mod Hub mods are handled by D28
([LOBBY.md](LOBBY.md), [MODS.md](MODS.md)); native mods come through a
channel of their own, managed the way CKAN manages Kerbal Space Program
mods, but only from the project's signed index.

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
| `settings` | Its settings with their defaults: booleans, integers or text of at most 128 bytes (a room tells them to its members). The launcher has no controls for them: a mod runs with its defaults, or with the room's settings once it joins a room that sets them. |
| `depends` | Packages it needs, with a semver requirement. |
| `conflicts` | Packages that must not be installed beside it. |
| `files` | Data, Lua and settings files: where each goes in the package's folder (plain names joined by `/`), where it comes from, its size and SHA-256. |
| `plugins` | Its own code: native libraries the hook loads into the game (`path`, `url`, `size`, `sha256`; `abi`, the hook's plugin interface they are built against; `platform`, `windows-x86_64` or `linux-x86_64`). A package with any must say `simulation: true`. See "Plugins" below (proposed D29 amendment). |
| `hooks` | The game functions its plugins patch, each found by signature: `target` (`<id>::<name>`), `signature`, `offset`, `prologue`, and `use` (`chain` or `replace`). See "Plugins" below. |

Unknown fields are refused, and one malformed entry refuses the whole index:
the project signs it, so a mistake is fixed there, never guessed around.

### Signing

The index is signed the way releases are (D7), with the same code: the
launcher's updater and the index call `tpf3mp_nativemods::signed` (Ed25519
through `ring`, public keys as base64 strings, more than one so that a key
can be changed). The key is the **native-mods key**, a key of its own as the
dev key is (D19): the public half is built into the launcher from
`TPF3MP_NATIVE_MODS_PUBLIC_KEY`, and a launcher built without one installs no
native mod. A release key cannot sign the index, and the native-mods key
cannot sign a release.

To be built with the decision: a `sign-native-mods.yml` workflow that signs
the index in a GitHub environment whose required reviewer approves each
signing, as `sign.yml` does for releases. No key is in the repository; the
tests make a fresh test key each time.

## A server's own index

*Built on a proposed amendment to D29 ([DECISIONS.md](DECISIONS.md)), not
decided: until the owner approves it, this belongs on the branch that
proposes it.* A server can offer native mods without the project's key:
its operator names a **registry** (`--native-mods-registry`,
[OPERATIONS.md](OPERATIONS.md), "Native mods"), and the server makes an
index of it and serves it to its players (protocol 21,
[PROTOCOL.md](PROTOCOL.md)). The launcher trusts it as it trusts the
server's rooms (D4).

The registry is a git repository, as CKAN's metadata repository is for
Kerbal mods, holding one file, `mods.txt`: one address of a mod's own
repository per line, `#` for comments.

```text
# Native mods this server offers
https://github.com/<author>/tpf3mp-bigmap
https://gitlab.com/<author>/trees
```

Each mod's repository holds `tpf3mp-native.json` at its top, read from its
default branch (GitHub's and GitLab's raw files; any other host by naming
the file's own address in the list). It holds the mod's packages in this
page's format. What players see of a mod in the mod browser comes from it:
its name, its description (at most 500 bytes), its `authors` (at most 8,
each at most 48 bytes; the repository's owner if it names none), and its
`preview`, a PNG or JPEG image, either a path in the repository or an
`https` address. The repository is each package's `homepage` unless the
file names another:

```json
{
  "format": 1,
  "packages": [
    {"id": "bigmap", "version": "0.3.0", "name": "Big Maps",
     "description": "Maps up to 128 km a side.",
     "authors": ["<author>"], "preview": "images/preview.png",
     "simulation": true, "builds": ["<SHA-256 of the game's executable>"],
     "features": ["bigmap.octree"], "settings": {"octree_depth": 10},
     "files": [{"path": "mod/tpf3mp_bigmap_1/mod.lua",
                "url": "https://github.com/<author>/tpf3mp-bigmap/releases/download/v0.3.0/mod.lua",
                "size": 1234, "sha256": "<lowercase hex>"}]}
  ]
}
```

The server reads the list and every mod's file again every hour by
default. Each mod stands on its own: one whose file cannot be read, holds a
malformed package or claims an id an earlier mod in the list has is left
out, with the reason in the server's log; the others are served. Its index
is numbered by the time it was made, always past the one before, and the
launcher refuses one older than it kept from that server.

**Downloads and dates.** For a mod on GitHub the server also asks GitHub
for its releases: how often their files were downloaded, all versions
together, and when the newest was published (with no release, when the
repository was last pushed to). The index carries them as `listings`, by
mod id, and the mod browser shows them; they are never used to choose what
is installed. A mod GitHub does not answer for has none, with the reason in
the log.

**Proposed mods.** A mod joins the registry through a pull request that
adds its repository to `mods.txt`. For a registry on GitHub the server also
reads the open pull requests and serves the mods they would add apart, as
*proposals*, each with its pull request (`registry::Proposals`). In the
launcher's mod browser **Show proposed** lists them, marked
"Proposed in pull request #N: not reviewed" with a link to it; a player
installs one only by choosing it. A room never gets a proposed mod by
itself: its members install it the same way, by choosing it.

**What the launcher does with it.** On connecting, and on **Check the
index**, it reads the server's index (and with proposals shown, the
proposals), keeps them per server (`native-mods/servers/<…>/`), and
installs from the server's index first, then from the project's signed
index when it was built with the key. It gets a room's native mods by
itself only from a server the release lists (or with the project's key);
from a server the player set in Settings it does so when the player presses
**Get the room's native mods**.

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
   satisfies the requirement). Refused: a package the index does not list,
   none for this build, one whose plugin this TPF3-MP cannot load, one
   naming a feature this hook does not have, a dependency no version
   satisfies, a dependency cycle, a
   conflict with a package chosen or installed. There is no backtracking: an
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
- **Installing is not enabling.** The player switches a package on
  separately.

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
  (below);
- its features do not clash with those of a package before it (below,
  "Several mods on one target").

Otherwise a package that changes only what its player sees is left off, with
the reason in `hook.log`; one that changes the simulation **keeps the game
out of rooms**: the hook installs nothing and logs
`multiplayer disabled (fail-closed): native mods that change the simulation
cannot run: …`, as for a build it has no profile for. An `enabled.json` that
cannot be read refuses the same way. No other switch turns a native feature
on: Big Maps' own branches read `TPF3MP_BIGMAP_*` variables by hand; as a
package, it would ask the plan instead (`native_mods::feature(id)`, which
gives the feature's settings and its package's folder).

### Features

In this first version a package carries no code. It names **features
compiled into the hook**, by id; the hook switches on those the launcher
enabled. Each feature has an id (the package's id, a dot, the feature's), an
effect (only what one player sees, or the simulation) and the profile
targets it patches. `features::BUILT_IN` is empty on `dev`: no native feature
is built in yet. Big Maps would add its own; `features::example::BIG_MAPS`
shows the shape, and the tests use it:

| feature | effect | profile targets, and how it uses each |
|---|---|---|
| `bigmap.octree` | simulation | `bigmap::octree_root`: value, largest |
| `bigmap.street_raster` | simulation | `bigmap::street_raster`: takes it over |
| `bigmap.placement` | simulation | `bigmap::placement_attempts`: value, largest |
| `bigmap.page` | what one player sees | `bigmap::tile_count`: value, largest; `bigmap::size_list`: value, every list's entries |
| `bigmap.density` | simulation | `bigmap::density_levels`: value, every list's entries |
| `bigmap.memory_gate` | what one player sees | none |

The launcher reads the same registry to refuse a package this TPF3-MP cannot
run before downloading it.

### Load order

The launcher writes the enabled packages to `enabled.json` in the **load
order**, and the hook switches them on in that order: each package after
those it depends on (`depends`), the rest by id (`enabled::load_order`;
packages that depend on each other in a circle, which installing already
refuses, come last). It is worked out the same way on every player's game
and is not set by the player: in a room, an order each player chose would be
one more way for their games to differ. The mod manager shows it.

### Several mods on one target

Every patch is the hook's own code, so two packages never patch a game
function apart: each feature says how it uses each target it patches
(`features::Access`), and the hook combines them, much as Harmony combines
patches in modded Unity games:

| how a feature uses a target | example | two features on it |
|---|---|---|
| **value**, with a merge rule | a size limit, a list of map sizes | merged by the rule (`features::Merge`): the largest, the smallest, the sum, or every list's entries |
| **chain** | runs before or after the game's function, which stays | both run, in the load order |
| **replace** | takes the function over | they **clash**: never on together |

Features that share a target must agree on how they combine: both values
with the same rule, or both chains. The tests hold `BUILT_IN` to that
(`features::registry_problems`), so a feature added to the hook that would
combine a target differently from another fails CI rather than a game. A
clash is caught three times over:

- **switching on**: the launcher refuses to switch on a package whose
  features clash with one already on, and the mod manager says which
  (`Store::set_enabled`, "Can't be switched on: it clashes with …");
- **in the game**: the hook's plan leaves a clashing package off, or keeps
  the game out of rooms if it changes the simulation, as for a missing
  target (`enabled::plan`), in case a launcher and hook of different
  builds disagree;
- **in the hook's code**: a feature asks the plan which active features
  share its target, in the load order (`Plan::users`), and merges their
  values (`Merge::numbers`, `Merge::lists`) or runs their chains in turn.

`conflicts` in the index remains for packages that must not be installed
together for reasons the hook cannot see.

## Plugins

*Built on a proposed amendment to D29 ([DECISIONS.md](DECISIONS.md)), not
decided: until the owner approves it, this belongs on the branch that
proposes it.* A package may carry code of its own: **plugins**, native
libraries the hook loads into the game, which patch the game functions the
package declares. Built-in features stay for what the project maintains
itself; a plugin is how a mod's authors ship their own.

```json
{"id": "faster_trains", "version": "0.4.1", "name": "Faster Trains",
 "simulation": true, "builds": ["<SHA-256 of the game's executable>"],
 "plugins": [{"path": "plugin/faster_trains.dll", "platform": "windows-x86_64",
              "abi": 1, "url": "https://github.com/<author>/faster-trains/releases/download/v0.4.1/faster_trains.dll",
              "size": 81920, "sha256": "<lowercase hex>"}],
 "hooks": [{"target": "faster_trains::max_speed",
            "signature": "48 89 5C 24 ? 57 48 83 EC 20 8B FA",
            "offset": 0, "prologue": "48895C2408", "use": "chain"}]}
```

**How it runs.** At bootstrap, while the game is still suspended and
before the hook's own multiplayer patches (`tpf3mp-hook`'s `plugins` and
`native_mods::load_plugins`), for the enabled packages in the load order:

1. every declared hook is found in the running build by its signature, as
   the hook finds its own (one match, else refused), and the function's
   first bytes are checked against `prologue`. A mod never names an
   address: addresses change with every game update and cannot be checked;
2. the hooks are checked against each other and against every function
   the hook patches itself: a hook on one of those refuses, and so do two
   on one function of which one is `replace`;
3. each plugin file is checked again against its pinned SHA-256, loaded,
   and started: its one export, `tpf3mp_plugin_init`, gets a `Host`
   (`tpf3mp-plugin-abi`) and **attaches** a detour to each declared hook by
   name, with a slot the hook fills with what the detour calls on to;
4. each function is patched once: the first detour in the load order
   runs, each calls on to the next, the last to the game's own code. A
   `replace` hook is alone on its function.

Any step that fails stops before anything is patched and keeps the game
out of rooms (`hook.log` says why). Plugins load only into the game on
Windows x86-64 yet; elsewhere a package with one keeps the game out of
rooms.

**In a room.** A plugin's code may change anything, so a package with one
changes the simulation, whatever it says: it must say `simulation: true`,
and its term's digest covers its plugins' SHA-256s as well as its settings
(`terms::package_digest`). A room's members run the same bytes, or are
told they differ.

**In the launcher.** A mod with a plugin is marked **Runs code** in the mod
browser and manager, with "Runs its own code in the game, with your
account's rights on this PC. Install it only from a server you trust." One
whose plugin is for another platform or another plugin interface, or that
needs a hook feature this TPF3-MP lacks, says why and is not offered for
install. Two packages whose declared hooks name the same function, one
taking it over, cannot both be switched on.

**Trust.** Plugins come from the same places as every native mod: the
project's index or the server's own (proposed D29 amendment, "A server's
own index"), and the launcher trusts a server's registry for code as it
does for rooms (D4). Installing is still the player's choice (a room's
mods are got by themselves only from a server the release lists), and a
proposed mod's plugin is never installed but by choosing it. A plugin runs
inside the game with the player's rights: there is no sandbox, and a bad
one can crash the game or worse. That is the trade-off the amendment asks
the owner to accept.

**Writing one.** Build a library for the platform (`cdylib` in Rust) that
exports `tpf3mp_plugin_init`, an `Init` from `tpf3mp-plugin-abi`:

```rust
use std::ffi::c_void;
use tpf3mp_plugin_abi::{Host, OK};

static mut NEXT: *const c_void = std::ptr::null();

// The game function's own signature and calling convention.
unsafe extern "C" fn max_speed(this: *mut c_void, kind: i32) -> f32 {
    let next: unsafe extern "C" fn(*mut c_void, i32) -> f32 = unsafe { std::mem::transmute(NEXT) };
    unsafe { next(this, kind) * 2.0 }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tpf3mp_plugin_init(host: *const Host) -> i32 {
    let host = unsafe { &*host };
    let name = "faster_trains::max_speed";
    let status = unsafe {
        (host.attach)(host.ctx, name.as_ptr(), name.len(), max_speed as *const c_void, &raw mut NEXT)
    };
    if status != OK { return status; }
    OK
}
```

## In a room

A native mod that changes the simulation must run in every game of a room,
in the same version, with the same settings; one that changes only what a
player sees is that player's own, as personal mods are ([MODS.md](MODS.md),
D25).

The room's **native terms** are, for each simulation-changing package the
game runs, its id, its version and a SHA-256 of its settings (defaults
included), ordered by id. Since protocol 20 they travel in the content
manifest's own list, `ContentManifest::native`
([PROTOCOL.md](PROTOCOL.md), "Native mods"): each as a mod named
`native:<id>` (no mod folder can be named so; `:` is not allowed in a
Windows file name) whose version is `<version>+<the settings digest's first
ten hex digits>`, at most 31 characters. The room compares them as it
compares mods: a member whose terms differ cannot start or join, and is
told which native mod is missing, which differs and which the room does not
run.

What a game declares is what the launcher started it with: the terms of the
`enabled.json` it wrote for that game (`tpf3mp-agent`'s `native_mods`), and
with no game running from the launcher, what the next game will run. The
room's owner also tells the room every setting of each (`RoomConfig::native`,
`RoomMods::native`), so a member's launcher can match them.

**Joining a room** gets the room's native mods by itself:

1. The room tells its native mods on joining its lobby, or before refusing
   a join to its running game.
2. The launcher installs each in the room's exact version from the signed
   index, for this game's build, **never from another player**
   (`match_room`, `Store::install_exact`); sets the room's settings over the
   defaults, the player's own choices left out; switches each on; and
   switches off any other simulation-changing native mod. One that changes
   only what the player sees stays as it was. A version the index does not
   have for this build, or settings that do not come out the same, refuse:
   that player cannot join, and the window says why.
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

A room restored after a server restart keeps its terms in its log (format
8, unchanged: the native mods follow the mods under their `native:` ids),
but not their settings: a member whose launcher must install one then runs
it with its defaults, and matches only if those are the room's.

Mods from Mod Hub are not native mods: players get those through their own
game and Mod Hub account (D28).

## In the launcher

Two buttons beside Settings, at the top of the window, open the mods
dialog, with tabs between its two pages. Each is a table of mods, a row
each with its preview image, name and authors, beside the details of the
one picked: its preview, large, its description, its project page
(`homepage`), what it is (version, size, and for the browser downloads and
date) and what can be done with it.

- **Mod manager** (the **Installed** tab): the native mods installed, with
  their version, size, **load order** (where each enabled one comes in the
  order the game loads them, "Load order" above; it is not set by the
  player) and whether each is on, has an update or clashes with one on;
  **Update**, **Switch on**/**Switch off**, **Roll back to** the version
  before, and **Uninstall**.
- **Mod browser** (the **Browse** tab): each package the last accepted
  index has for this game's build, with its version, download size,
  downloads and when it was last updated, to search by name, author or
  description; **Install** or **Update**. With **Show proposed** it lists
  the server's proposed mods too, marked as not reviewed and linking to
  their pull request.

Preview images are fetched as they are shown, over HTTPS only, at most
4 MB, PNG or JPEG, and kept for the session (`tpf3mp-launcher`'s
`previews`); a mod without one shows its initials.

Neither has controls for a mod's settings: a mod runs with its defaults,
or with the settings of the room it was matched to. **Check the index**
fetches the index again. A change takes effect in the next game the
launcher starts; with no game running, the player declares the new terms
at once.

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
native mods on joining, the game started again with them.

This is not yet a feature players can use: the production trusted-key list
is empty and `features::BUILT_IN` has no native features. The tests use
temporary test keys.

Still open:

- the native-mods key, its signing workflow and the first index (or, with
  the proposed amendment, a server's registry: built);
- copying a package's Lua mod folder where the game loads mods, the way
  TPF3-MP's own mod is installed ([MODS.md](MODS.md), "TPF3-MP's own mod");
- Big Maps as the first package, with its features built into the hook;
- plugin libraries.
