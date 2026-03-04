<!-- Header -->
<p align="center">
  <img src="docs/icon.png" width="128" height="128" alt="RustCast icon" />
</p>

<h1 align="center">RustCast</h1>

<p align="center">
  An open-source, Rust-powered productivity, blazing fast popup launcher for apps, utilities, and workflows.
</p>

<p align="center">
  <a href="https://github.com/unsecretised/rustcast/releases/latest">
    <img alt="Latest release" src="https://img.shields.io/github/v/release/unsecretised/rustcast?display_name=tag&sort=semver&style=flat-square" />
  </a>
  <a href="https://github.com/unsecretised/rustcast/releases">
    <img alt="Downloads" src="https://img.shields.io/github/downloads/unsecretised/rustcast/total?style=flat-square" />
  </a>
  <a href="https://discord.gg/bDfNYPbnC5">
    <img alt="Discord" src="https://img.shields.io/discord/1463119282459119844?label=Discord&logo=discord&logoColor=white&style=flat-square" />
  </a>
  <a href="https://github.com/unsecretised/rustcast/stargazers">
    <img alt="Stars" src="https://img.shields.io/github/stars/unsecretised/rustcast?style=flat-square" />
  </a>
  <a href="https://github.com/unsecretised/rustcast/blob/main/LICENSE">
    <img alt="License" src="https://img.shields.io/github/license/unsecretised/rustcast?style=flat-square" />
  </a>
</p>

**Config docs:** https://github.com/unsecretised/rustcast/wiki

**Community:** https://discord.gg/bDfNYPbnC5

**Plugins**:
[RustCast Library for shell scripts](https://github.com/unsecretised/rustcast-library)

![RustCast Demo](./docs/rustcast-latest-demo.png)

## Installation:

### Via Homebrew:

```
brew install --cask unsecretised/tap/rustcast
```

### Via github releases

1. Download the dmg from this link
   [https://github.com/unsecretised/rustcast/releases/latest/download/rustcast.dmg](https://github.com/unsecretised/rustcast/releases/latest/download/rustcast.dmg)

## Config:

Full config docs can be found [here](https://github.com/unsecretised/rustcast/wiki)

The config file should be located at: `~/.config/rustcast/config.toml` RustCast
creates the default configuration for you, but it does use its
[default options](docs/default.toml) Here's a full list of what all you can
configure [The list](docs/config.toml). 

## Feature list:

### Finished:

- [x] Autoload installed apps 11/11/2025
- [x] Search through apps 11/11/2025
- [x] Generate [randomvar](https://github.com/Nazeofel) (between 0 and 100) via
      the app. Simply type `randomvar` and it will generate the num for you
- [x] Image icons next to the text 13/12/2025
- [x] Scrollable options 12/12/2025
- [x] Customisable themes (13/12/2025)
  - [x] Configurable colours
- [x] Spotify control - Ability to control spotify via the app
- [x] Allow variables to be passed into custom shell scripts.
- [x] Google your query. Simply type your query, and then put a `?` at the end,
      and press enter
- [x] Calculator (27/12/2025)
- [x] Clipboard History (29/12/2025) This works by typing `cbhist` to enter the
      cliboard history page, which allows u to access your clipboard history,
- [x] Blur / transparent background (7/1/2026)
- [x] Select the options using arrow keys
- [x] Tray icons (8/1/2026)
- [x] Unit Conversions (19/1/2026) thanks to
      [Hriztam](https://github.com/hriztam)
- [x] Emoji Searching (19/1/2026) Allows people to search for emojis through
      rustcast
- [x] RustCast modes (2/3/2026)
- [x] Better documentation for the config (3/3/2026)

### Planned:

- [ ] Popup note-taking
- [ ] Settings Panel
- [ ] Plugin Support (Partially implemented on 15/12/2025)
- [ ] Hyperkey - Map CMD + OPT + CTRL + SHIFT to a physical key
- [ ] Image rendering from clipboard history
- [ ] Better hotkey picking
- [ ] Ability to pick between tabs in firefox / chromium browsers - using
      [Puppeteer](https://pptr.dev/)

### Not planned:

- [ ] Cross platform support Cancelled for now, as not within my ability to
      support and maintain it

## RustCast wouldn't be possible without these people:

- [Nazeofel](https://github.com/Nazeofel) - First sponsor + initiater of windows
  support
- [Mnem42](https://github.com/mnem42) - Helped add windows support
- [Random Scientist](https://github.com/Random-Scientist) - First ever community
  contributor to rustcast
- [Lemon](https://github.com/lemonlambda) - Sponsored me, and gave me free
  Discord Nitro
- [Julie / Zoey](https://github.com/zoey-on-github) - Gave me amazing feedback
  and has been using RustCast since almost the first version!
- [Hriztam](https://github.com/hriztam) - Added support for unit conversions to
  rustcast

All the people who starred my repo!!

Those who sponsor me also get a free easter egg inside RustCast, so if you want
a free easter egg, a dollar is all you need!

And the updated list of contributors to the macos version:

<a href="https://github.com/unsecretised/rustcast/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=unsecretised/rustcast" />
</a>

### Easter egg list:

- Nazeofel (Random Variable on discord) -> led to the _randomvar_ easter egg
- Lemon -> led to the _lemon_ easter egg that shows "lemon" on rustcast

## If you like rustcast, consider starring it on github :)

[![Star History Chart](https://api.star-history.com/svg?repos=unsecretised/rustcast&type=date&legend=top-left)](https://www.star-history.com/#unsecretised/rustcast&type=date&legend=top-left)

## Motivations:

I didn't want to pay for raycast + wanted to get better at rust. Raycast in
itself is one of the most useful productivity apps in my opinion, and it is
truly an underappreiciated marvel of computer engineering
